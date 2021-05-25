use anyhow::Result;
use rollup_state_manager::state::{GlobalState, WitnessGenerator};
use rollup_state_manager::test_utils::messages::{parse_msg, WrappedMessage};
use rollup_state_manager::types::l2;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::Instant;

use pprof::protos::Message;
use std::io::Write;

mod types;
use types::{test_params, Accounts, Orders};

//if we use nightly build, we are able to use bench test ...
fn bench_global_state(circuit_repo: &Path) -> Result<Vec<l2::L2Block>> {
    let test_dir = circuit_repo.join("test").join("testdata");
    let file = File::open(test_dir.join("msgs_float.jsonl"))?;

    let messages: Vec<WrappedMessage> = BufReader::new(file)
        .lines()
        .map(Result::unwrap)
        .map(parse_msg)
        .map(Result::unwrap)
        .filter(|msg| matches!(msg, WrappedMessage::BALANCE(_) | WrappedMessage::TRADE(_)))
        .collect();

    println!("prepare bench: {} records", messages.len());

    GlobalState::print_config();
    // TODO: use ENV
    //use custom states
    let verbose = false;
    let state = GlobalState::new(
        20, //test_params::BALANCELEVELS,
        20, //test_params::ORDERLEVELS,
        20, //test_params::ACCOUNTLEVELS,
        verbose,
    );
    let mut witgen = WitnessGenerator::new(state, test_params::NTXS, verbose);

    //amplify the records: in each iter we run records on a group of new accounts
    let mut timing = Instant::now();
    let mut orders = Orders::default();
    let mut accounts = Accounts::default();
    for i in 1..51 {
        for msg in messages.iter() {
            match msg {
                WrappedMessage::BALANCE(balance) => {
                    accounts.handle_deposit(&mut witgen, balance.clone());
                }
                WrappedMessage::TRADE(trade) => {
                    let trade = accounts.transform_trade(&mut witgen, trade.clone());
                    orders.handle_trade(&mut witgen, &accounts, trade);
                }
                _ => unreachable!(),
            }
        }

        accounts.clear();

        if i % 10 == 0 {
            let total = timing.elapsed().as_secs_f32();
            let (balance_t, _) = accounts.take_bench();
            let (plact_t, spot_t) = orders.take_bench();
            println!(
                "{}th 10 iters in {:.5}s: balance {:.3}%, place {:.3}%, spot {:.3}%",
                i / 10,
                total,
                balance_t * 100.0 / total,
                plact_t * 100.0 / total,
                spot_t * 100.0 / total
            );
            timing = Instant::now();
        }
    }

    Ok(witgen.take_blocks())
}

fn run_bench() -> Result<()> {
    let circuit_repo = fs::canonicalize(PathBuf::from("../circuits")).expect("invalid circuits repo path");

    let timing = Instant::now();
    let blocks = bench_global_state(&circuit_repo)?;
    println!(
        "bench for {} blocks (TPS: {})",
        blocks.len(),
        (test_params::NTXS * blocks.len()) as f32 / timing.elapsed().as_secs_f32()
    );

    Ok(())
}

fn main() {
    let guard = pprof::ProfilerGuard::new(100).unwrap();

    run_bench().unwrap();

    if let Ok(report) = guard.report().build() {
        let file = File::create("flamegraph.svg").unwrap();
        report.flamegraph(file).unwrap();

        let mut file = File::create("profile.pb").unwrap();
        let profile = report.pprof().unwrap();

        let mut content = Vec::new();
        profile.encode(&mut content).unwrap();
        file.write_all(&content).unwrap();

        println!("report: {:?}", &report);
    };
}
