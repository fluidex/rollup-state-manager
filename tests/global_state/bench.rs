#![allow(dead_code)]
use anyhow::Result;
use rollup_state_manager::account::{self, Account};
use rollup_state_manager::state::{GlobalState, WitnessGenerator};
use rollup_state_manager::test_utils::{
    self,
    messages::{parse_msg, WrappedMessage}
};
use rollup_state_manager::types::l2;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::Instant;

use pprof::protos::Message;
use std::io::Write;

mod types;
use types::{Accounts, Orders};

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
        20, //test_utils::params::BALANCELEVELS,
        20, //test_utils::params::ORDERLEVELS,
        20, //test_utils::params::ACCOUNTLEVELS,
        verbose,
    );

    //amplify the records: in each iter we run records on a group of new accounts
    let mut orders = Orders::default();
    let mut accounts = Accounts::default();

    // TODO: max(user id)
    let account_num = 10;
    // we are generating more txs from the given test cases
    // by clone accounts with same trades
    let loop_num = 50;
    let cache_order_sig = false;
    if cache_order_sig {
        for j in 0..account_num {
            let seed = account::rand_seed();
            for i in 0..loop_num {
                let account_id = i * account_num + j;
                // since we cache order_sig by Map<(order_hash, bjj_key), Signature>
                // we can make cache meet 100% by reusing seed
                //let seed = if cache_order_sig { seed.clone() } else { account::rand_seed() };
                let seed = seed.clone();
                let acc = Account::from_seed(account_id, &seed).unwrap();
                accounts.set_account(account_id, acc);
            }
        }
        for msg in messages.iter() {
            if let WrappedMessage::TRADE(trade) = msg {
                orders.sign_orders(&accounts, trade.clone());
            }
        }
    }

    let mut witgen = WitnessGenerator::new(state, *test_utils::params::NTXS, verbose);

    let timing = Instant::now();
    let mut inner_timing = Instant::now();

    for i in 0..loop_num {
        let account_offset = i * account_num;
        for msg in messages.iter() {
            match msg {
                WrappedMessage::BALANCE(balance) => {
                    let mut balance = balance.clone();
                    balance.user_id += account_offset;
                    accounts.handle_deposit(&mut witgen, balance);
                }
                WrappedMessage::TRADE(trade) => {
                    let mut trade = trade.clone();
                    trade.ask_user_id += account_offset;
                    trade.bid_user_id += account_offset;
                    orders.handle_trade(&mut witgen, &accounts, trade);
                }
                _ => unreachable!(),
            }
        }

        if i % 10 == 0 {
            let total = inner_timing.elapsed().as_secs_f32();
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
            inner_timing = Instant::now();
        }
    }
    let blocks = witgen.take_blocks();
    println!(
        "bench for {} blocks (TPS: {})",
        blocks.len(),
        (*test_utils::params::NTXS * blocks.len()) as f32 / timing.elapsed().as_secs_f32()
    );
    Ok(blocks)
}

fn run_bench() -> Result<()> {
    let circuit_repo = fs::canonicalize(PathBuf::from("../circuits")).expect("invalid circuits repo path");
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
