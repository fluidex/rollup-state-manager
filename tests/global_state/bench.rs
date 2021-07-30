#![allow(dead_code)]
#![allow(unreachable_patterns)]
use anyhow::Result;
use pprof::protos::Message;
use rollup_state_manager::params;
use rollup_state_manager::state::{GlobalState, WitnessGenerator};
use rollup_state_manager::test_utils::messages::{parse_msg, WrappedMessage};
use rollup_state_manager::types::l2;
use std::fs::{self, File};
use std::io::Write;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Instant;

use rollup_state_manager::msg::msg_processor;
use std::option::Option::None;

//if we use nightly build, we are able to use bench test ...
fn bench_global_state(_circuit_repo: &Path) -> Result<Vec<l2::L2Block>> {
    let filepath = "tests/global_state/testdata/data.txt";
    let file = File::open(filepath)?;
    let messages: Vec<WrappedMessage> = BufReader::new(file)
        .lines()
        .map(Result::unwrap)
        .map(parse_msg)
        .map(Result::unwrap)
        //.filter(|msg| matches!(msg, WrappedMessage::BALANCE(_) | WrappedMessage::ORDER(_)))
        .collect();

    println!("prepare bench: {} records", messages.len());

    GlobalState::print_config();
    let state = Arc::new(RwLock::new(GlobalState::new(
        *params::BALANCELEVELS,
        *params::ORDERLEVELS,
        *params::ACCOUNTLEVELS,
        *params::VERBOSE,
    )));

    //amplify the records: in each iter we run records on a group of new accounts
    let mut processor = msg_processor::Processor {
        enable_check_order_sig: false,
        ..Default::default()
    };

    // TODO: max(user id)
    let account_num = 10;
    // we are generating more txs from the given test cases
    // by clone accounts with same trades
    let loop_num = 50;

    let mut witgen = WitnessGenerator::new(state, *params::NTXS, None, *params::VERBOSE);
    let timing = Instant::now();
    let mut inner_timing = Instant::now();

    for i in 0..loop_num {
        let account_offset = i * account_num;
        for msg in messages.iter() {
            match msg {
                WrappedMessage::USER(user) => {
                    let mut user = user.clone();
                    user.user_id += account_offset;
                    processor.handle_user_msg(&mut witgen, user);
                }
                WrappedMessage::BALANCE(balance) => {
                    let mut balance = balance.clone();
                    balance.user_id += account_offset;
                    processor.handle_balance_msg(&mut witgen, balance);
                }
                WrappedMessage::TRADE(trade) => {
                    let mut trade = trade.clone();
                    trade.ask_user_id += account_offset;
                    trade.bid_user_id += account_offset;
                    trade.state_after = None;
                    trade.state_before = None;
                    trade.ask_order.as_mut().map(|mut o| {
                        o.user += account_offset;
                        o
                    });
                    trade.bid_order.as_mut().map(|mut o| {
                        o.user += account_offset;
                        o
                    });
                    processor.handle_trade_msg(&mut witgen, trade);
                }
                WrappedMessage::ORDER(order) => {
                    let mut order = order.clone();
                    order.order.user += account_offset;
                    processor.handle_order_msg(&mut witgen, order);
                }
                _ => unreachable!(),
            }
        }

        if i % 10 == 0 {
            let total = inner_timing.elapsed().as_secs_f32();
            let (balance_t, trade_t) = processor.take_bench();
            println!(
                "{}th 10 iters in {:.5}s: balance {:.3}%, trade {:.3}%",
                i / 10,
                total,
                balance_t * 100.0 / total,
                trade_t * 100.0 / total
            );
            inner_timing = Instant::now();
        }
        //println!("\nepoch {} done", i);
    }

    let blocks: Vec<_> = witgen.pop_all_blocks();
    println!(
        "bench for {} blocks (TPS: {})",
        blocks.len(),
        (*params::NTXS * blocks.len()) as f32 / timing.elapsed().as_secs_f32()
    );
    Ok(blocks)
}

fn run_bench() -> Result<()> {
    let circuit_repo = fs::canonicalize(PathBuf::from("circuits")).expect("invalid circuits repo path");
    let _ = bench_global_state(&circuit_repo)?;
    Ok(())
}

fn profile_bench() {
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

fn main() {
    run_bench().unwrap();
}
