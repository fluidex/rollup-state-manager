#![allow(dead_code)]
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::large_enum_variant)]

use anyhow::Result;
use rollup_state_manager::state::{GlobalState, WitnessGenerator};
use rollup_state_manager::test_utils;
use rollup_state_manager::test_utils::messages::{parse_msg, WrappedMessage};
use rollup_state_manager::test_utils::L2BlockSerde;
use rollup_state_manager::types::l2;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Lines, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

mod types;
use types::{test_params, Accounts, Orders};

fn replay_msgs(circuit_repo: &Path) -> Result<(Vec<l2::L2Block>, test_utils::circuit::CircuitSource)> {
    let test_dir = circuit_repo.join("test").join("testdata");
    let file = File::open(test_dir.join("msgs_float.jsonl"))?;

    let lns: Lines<BufReader<File>> = BufReader::new(file).lines();

    let state = GlobalState::new(
        test_params::BALANCELEVELS,
        test_params::ORDERLEVELS,
        test_params::ACCOUNTLEVELS,
        test_params::VERBOSE,
    );
    let mut witgen = WitnessGenerator::new(state, test_params::NTXS, test_params::VERBOSE);

    println!("genesis root {}", witgen.root());

    let mut orders = Orders::default();
    let mut accounts = Accounts::default();

    let messages: Vec<WrappedMessage> = lns.map(|l| parse_msg(l.unwrap()).unwrap()).collect();

    let timing = Instant::now();
    for msg in messages {
        match msg {
            WrappedMessage::BALANCE(balance) => {
                accounts.handle_deposit(&mut witgen, balance);
            }
            WrappedMessage::TRADE(trade) => {
                let trade_id = trade.id;
                orders.handle_trade(&mut witgen, &accounts, trade);
                println!("trade {} test done", trade_id);
            }
            _ => {
                //other msg is omitted
            }
        }
    }

    witgen.flush_with_nop();
    let blocks = witgen.take_blocks();
    println!(
        "genesis {} blocks (TPS: {})",
        blocks.len(),
        (test_params::NTXS * blocks.len()) as f32 / timing.elapsed().as_secs_f32()
    );

    let component = test_utils::circuit::CircuitSource {
        src: String::from("src/block.circom"),
        main: format!(
            "Block({}, {}, {}, {})",
            test_params::NTXS,
            test_params::BALANCELEVELS,
            test_params::ORDERLEVELS,
            test_params::ACCOUNTLEVELS
        ),
    };

    Ok((blocks, component))
}

//just grap from export_circuit_test.rs ...
fn write_circuit(circuit_repo: &Path, test_dir: &Path, source: &test_utils::CircuitSource) -> Result<PathBuf> {
    let circuit_name = test_utils::format_circuit_name(source.main.as_str());
    let circuit_dir = test_dir.join(circuit_name);

    fs::create_dir_all(circuit_dir.clone())?;

    let circuit_file = circuit_dir.join("circuit.circom");

    // on other OS than UNIX the slash in source wolud not be considerred as separator
    //so we need to convert them explicity
    let src_path: PathBuf = source.src.split('/').collect();

    let file_content = format!(
        "include \"{}\";\ncomponent main = {}",
        circuit_repo.join(src_path).to_str().unwrap(),
        source.main
    );
    let mut f = File::create(circuit_file)?;
    f.write_all(&file_content.as_bytes())?;
    Ok(circuit_dir)
}

fn write_input_output(dir: &Path, block: l2::L2Block) -> Result<()> {
    fs::create_dir_all(dir)?;

    let input_f = File::create(dir.join("input.json"))?;
    serde_json::to_writer_pretty(input_f, &L2BlockSerde::from(block))?;

    let output_f = File::create(dir.join("output.json"))?;
    //TODO: no output?
    serde_json::to_writer_pretty(output_f, &serde_json::Value::Object(Default::default()))?;

    Ok(())
}

fn export_circuit_and_testdata(circuit_repo: &Path, blocks: Vec<l2::L2Block>, source: test_utils::CircuitSource) -> Result<PathBuf> {
    let test_dir = circuit_repo.join("testdata");
    let circuit_dir = write_circuit(circuit_repo, &test_dir, &source)?;

    for (blki, blk) in blocks.into_iter().enumerate() {
        let dir = circuit_dir.join(format!("{:04}", blki));
        write_input_output(&dir, blk)?;
        //println!("{}", serde_json::to_string_pretty(&types::L2BlockSerde::from(blk)).unwrap());
    }

    Ok(circuit_dir)
}

pub fn run() -> Result<()> {
    let circuit_repo = fs::canonicalize(PathBuf::from("circuits")).expect("invalid circuits repo path");

    let (blocks, components) = replay_msgs(&circuit_repo)?;

    let circuit_dir = export_circuit_and_testdata(&circuit_repo, blocks, components)?;

    println!("test circuit dir {}", circuit_dir.to_str().unwrap());

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
