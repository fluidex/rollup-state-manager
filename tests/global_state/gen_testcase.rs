#![allow(dead_code)]
#![allow(unreachable_patterns)]
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::unnecessary_wraps)]

use anyhow::Result;
use rollup_state_manager::state::{GlobalState, WitnessGenerator};
use rollup_state_manager::test_utils;
use rollup_state_manager::params;
use rollup_state_manager::test_utils::l2::L2Block;
use rollup_state_manager::test_utils::messages::WrappedMessage;
use std::fs::{self};
use std::path::PathBuf;
use std::time::Instant;

mod export_circuit;
mod msg_consumer;
mod msg_loader;
mod msg_processor;
mod types;

fn replay_msgs(
    msg_receiver: crossbeam_channel::Receiver<WrappedMessage>,
    block_sender: crossbeam_channel::Sender<L2Block>,
) -> Option<std::thread::JoinHandle<anyhow::Result<()>>> {
    Some(std::thread::spawn(move || {
        let state = GlobalState::new(
            *params::BALANCELEVELS,
            *params::ORDERLEVELS,
            *params::ACCOUNTLEVELS,
            *params::VERBOSE,
        );
        let mut witgen = WitnessGenerator::new(state, *params::NTXS, block_sender, *params::VERBOSE);

        println!("genesis root {}", witgen.root());

        let mut processor = msg_processor::Processor::default();

        let timing = Instant::now();
        for msg in msg_receiver.iter() {
            match msg {
                WrappedMessage::BALANCE(balance) => {
                    processor.handle_balance_msg(&mut witgen, balance);
                }
                WrappedMessage::TRADE(trade) => {
                    let trade_id = trade.id;
                    processor.handle_trade_msg(&mut witgen, trade);
                    println!("trade {} test done", trade_id);
                }
                WrappedMessage::ORDER(order) => {
                    processor.handle_order_msg(&mut witgen, order);
                }
                _ => {
                    //other msg is omitted
                }
            }
        }
        witgen.flush_with_nop();
        let block_num = witgen.get_block_generate_num();
        if let Ok(path) = std::env::var("SLED_DB_PATH") {
            let db = sled::open(&path).unwrap();
            witgen.dump_to_sled(&db);
        }
        println!(
            "genesis {} blocks (TPS: {})",
            block_num,
            (*params::NTXS * block_num) as f32 / timing.elapsed().as_secs_f32()
        );
        Ok(())
    }))
}

pub fn run(src: &str) -> Result<()> {
    let circuit_repo = fs::canonicalize(PathBuf::from("circuits")).expect("invalid circuits repo path");
    let filepath = PathBuf::from(src);
    let (msg_sender, msg_receiver) = crossbeam_channel::unbounded();
    let (blk_sender, blk_receiver) = crossbeam_channel::unbounded();

    let loader_thread = msg_loader::load_msgs_from_file(&filepath.to_str().unwrap(), msg_sender);

    let replay_thread = replay_msgs(msg_receiver, blk_sender);

    let blocks: Vec<_> = blk_receiver.iter().collect();

    loader_thread.map(|h| h.join().expect("loader thread failed"));
    replay_thread.map(|h| h.join().expect("replay thread failed"));

    let component = test_utils::circuit::CircuitSource {
        src: String::from("src/block.circom"),
        main: format!(
            "Block({}, {}, {}, {})",
            *params::NTXS,
            *params::BALANCELEVELS,
            *params::ORDERLEVELS,
            *params::ACCOUNTLEVELS
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
    let default_test_file = "circuits/test/testdata/msgs_float.jsonl";
    //let default_test_file = "tests/global_state/testdata/data001.txt";
    let test_file = std::env::args().nth(1).unwrap_or_else(|| default_test_file.into());
    match run(&test_file) {
        Ok(_) => println!("global_state test_case generated"),
        Err(e) => panic!("{:#?}", e),
    }
}
