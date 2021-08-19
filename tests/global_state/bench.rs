#![allow(dead_code)]
#![allow(unreachable_patterns)]
use anyhow::Result;
use fluidex_common::rust_decimal_macros::dec;
use fluidex_common::types::{Decimal, Float864};
use rollup_state_manager::account::Account;
use rollup_state_manager::msg::msg_processor;
use rollup_state_manager::params;
use rollup_state_manager::state::{GlobalState, ManagerWrapper};
use rollup_state_manager::test_utils::messages::{parse_msg, WrappedMessage};
use rollup_state_manager::test_utils::types::{get_mnemonic_by_account_id, prec_token_id};
use rollup_state_manager::types::l2::{self, TransferTx};
use rollup_state_manager::types::matchengine::messages::{DepositMessage, UserMessage};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::option::Option::None;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Instant;
#[cfg(test)]
use {pprof::protos::Message, std::io::Write};

fn bench_with_dummy_transfers() -> Result<()> {
    GlobalState::print_config();
    let state = Arc::new(RwLock::new(GlobalState::new(
        *params::BALANCELEVELS,
        *params::ORDERLEVELS,
        *params::ACCOUNTLEVELS,
        *params::VERBOSE,
    )));

    let mut processor = msg_processor::Processor {
        enable_check_sig: false,
        ..Default::default()
    };

    let mut manager = ManagerWrapper::new(state, *params::NTXS, None, *params::VERBOSE);

    // step1: create users
    let user1 = Account::from_mnemonic(1, &get_mnemonic_by_account_id(1)).unwrap();
    let user2 = Account::from_mnemonic(2, &get_mnemonic_by_account_id(2)).unwrap();
    let user1_msg = UserMessage {
        user_id: user1.uid,
        l1_address: user1.eth_addr_str(),
        l2_pubkey: user1.bjj_pub_key(),
    };
    let user2_msg = UserMessage {
        user_id: user2.uid,
        l1_address: user2.eth_addr_str(),
        l2_pubkey: user2.bjj_pub_key(),
    };
    println!("user1 {:?} user2 {:?}", user1_msg, user2_msg);
    processor.handle_user_msg(&mut manager, user1_msg.into());
    processor.handle_user_msg(&mut manager, user2_msg.into());

    // step2: deposit assets

    let deposit = DepositMessage {
        timestamp: 0.0, // FIXME?
        user_id: 1,
        asset: "ETH".to_string(),
        business: "useless".to_string(),
        change: dec!(10000),
        balance: dec!(10000),
        balance_available: dec!(0),
        balance_frozen: dec!(0),
        detail: "none".to_string(),
    };

    processor.handle_deposit_msg(&mut manager, deposit.into());

    // step3: bench transfer
    let amount = Float864::from_decimal(&dec!(1), prec_token_id(0)).unwrap();
    let mut transfer = TransferTx::new(1, 2, 0 /*ETH*/, amount);
    let transfer_hash = transfer.hash();
    let sig = user1.sign_hash(transfer_hash).unwrap();
    transfer.sig = sig;

    let timing = Instant::now();
    for i in 0..10000 {
        manager.transfer(transfer.clone(), None);
        if i % 100 == 0 {
            println!("{}%...", i / 100);
        }
    }
    let elapsed = timing.elapsed();
    println!("10000 transfer takes {:?}", elapsed);
    println!("avg tps {}", 10000.0 / elapsed.as_secs_f64());
    Ok(())
}

//if we use nightly build, we are able to use bench test ...
fn bench_with_real_trades(_circuit_repo: &Path) -> Result<Vec<l2::L2Block>> {
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
        enable_check_sig: false,
        ..Default::default()
    };

    // TODO: max(user id)
    let account_num = 10;
    // we are generating more txs from the given test cases
    // by clone accounts with same trades
    let loop_num = 50;

    let mut manager = ManagerWrapper::new(state, *params::NTXS, None, *params::VERBOSE);
    let timing = Instant::now();
    let mut inner_timing = Instant::now();

    for i in 0..loop_num {
        let account_offset = i * account_num;
        for msg in messages.iter() {
            match msg {
                WrappedMessage::DEPOSIT(deposit) => {
                    let mut deposit = deposit.clone();
                    deposit.user_id += account_offset;
                    processor.handle_deposit_msg(&mut manager, deposit);
                }
                WrappedMessage::ORDER(order) => {
                    let mut order = order.clone();
                    order.order.user += account_offset;
                    processor.handle_order_msg(&mut manager, order);
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
                    processor.handle_trade_msg(&mut manager, trade);
                }
                WrappedMessage::TRANSFER(transfer) => {
                    let mut transfer = transfer.clone();
                    transfer.user_from += account_offset;
                    transfer.user_to += account_offset;
                    processor.handle_transfer_msg(&mut manager, transfer);
                }
                WrappedMessage::USER(user) => {
                    let mut user = user.clone();
                    user.user_id += account_offset;
                    processor.handle_user_msg(&mut manager, user);
                }
                WrappedMessage::WITHDRAW(withdraw) => {
                    let mut withdraw = withdraw.clone();
                    withdraw.user_id += account_offset;
                    processor.handle_withdraw_msg(&mut manager, withdraw);
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

    let blocks: Vec<_> = manager.pop_all_blocks();
    println!(
        "bench for {} blocks (TPS: {})",
        blocks.len(),
        (*params::NTXS * blocks.len()) as f32 / timing.elapsed().as_secs_f32()
    );
    Ok(blocks)
}

fn run_bench() -> Result<()> {
    //let bench_type = "real_trades";
    let bench_type = "dummy_transfers";
    match bench_type {
        "real_trades" => {
            let circuit_repo = fs::canonicalize(PathBuf::from("circuits")).expect("invalid circuits repo path");
            let _ = bench_with_real_trades(&circuit_repo)?;
        }
        "dummy_transfers" => {
            bench_with_dummy_transfers()?;
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
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
