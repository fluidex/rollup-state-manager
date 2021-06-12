#![allow(dead_code)]
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::unnecessary_wraps)]

use anyhow::Result;
use rollup_state_manager::state::{GlobalState, WitnessGenerator};
use rollup_state_manager::test_utils;
use rollup_state_manager::test_utils::l2::L2Block;
use rollup_state_manager::test_utils::messages::WrappedMessage;
use std::fs::{self};
use std::path::PathBuf;
use std::time::Instant;

mod export_circuit;
mod msg_loader;
mod msg_preprocessor;
mod types;

fn replay_msgs(
    msg_receiver: crossbeam_channel::Receiver<WrappedMessage>,
    block_sender: crossbeam_channel::Sender<L2Block>,
) -> Option<std::thread::JoinHandle<()>> {
    Some(std::thread::spawn(move || {
        let state = GlobalState::new(
            *test_utils::params::BALANCELEVELS,
            *test_utils::params::ORDERLEVELS,
            *test_utils::params::ACCOUNTLEVELS,
            *test_utils::params::VERBOSE,
        );
        let mut witgen = WitnessGenerator::new(state, *test_utils::params::NTXS, block_sender, *test_utils::params::VERBOSE);

        println!("genesis root {}", witgen.root());

        let mut processor = msg_preprocessor::Preprocessor::default();

        let timing = Instant::now();
        for msg in msg_receiver.iter() {
            match msg {
                WrappedMessage::BALANCE(balance) => {
                    processor.handle_deposit(&mut witgen, balance);
                }
                WrappedMessage::TRADE(trade) => {
                    let trade_id = trade.id;
                    processor.handle_trade(&mut witgen, trade);
                    println!("trade {} test done", trade_id);
                }
                _ => {
                    //other msg is omitted
                }
            }
        }
        witgen.flush_with_nop();
        let block_num = witgen.get_block_generate_num();
        println!(
            "genesis {} blocks (TPS: {})",
            block_num,
            (*test_utils::params::NTXS * block_num) as f32 / timing.elapsed().as_secs_f32()
        );
    }))
}

pub fn run() -> Result<()> {
    let circuit_repo = fs::canonicalize(PathBuf::from("circuits")).expect("invalid circuits repo path");

    let test_dir = circuit_repo.join("test").join("testdata");
    let filepath = test_dir.join("msgs_float.jsonl");
    let (msg_sender, msg_receiver) = crossbeam_channel::unbounded();
    let (blk_sender, blk_receiver) = crossbeam_channel::unbounded();

    let loader_thread = msg_loader::load_msgs_from_file(&filepath.to_str().unwrap(), msg_sender);

    let replay_thread = replay_msgs(msg_receiver, blk_sender);

    let blocks: Vec<_> = blk_receiver.try_iter().collect();

    loader_thread.map(|h| h.join());
    replay_thread.map(|h| h.join());

    let component = test_utils::circuit::CircuitSource {
        src: String::from("src/block.circom"),
        main: format!(
            "Block({}, {}, {}, {})",
            *test_utils::params::NTXS,
            *test_utils::params::BALANCELEVELS,
            *test_utils::params::ORDERLEVELS,
            *test_utils::params::ACCOUNTLEVELS
        ),
    };

    let circuit_dir = export_circuit::export_circuit_and_testdata(&circuit_repo, blocks, component)?;

    println!("export test cases to {}", circuit_dir.to_str().unwrap());

    Ok(())
}

/*
 * have a look at scripts/global_state_test.sh
 */

fn main() {
    match run() {
        Ok(_) => println!("global_state test_case generated"),
        Err(e) => panic!("{:#?}", e),
    }
}
