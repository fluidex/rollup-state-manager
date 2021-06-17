#![allow(dead_code)]

use rollup_state_manager::config;
use rollup_state_manager::state::{GlobalState, WitnessGenerator};
use rollup_state_manager::test_utils;
use rollup_state_manager::test_utils::l2::L2Block;
use rollup_state_manager::test_utils::messages::WrappedMessage;
use std::time::Instant;

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
            *test_utils::params::BALANCELEVELS,
            *test_utils::params::ORDERLEVELS,
            *test_utils::params::ACCOUNTLEVELS,
            *test_utils::params::VERBOSE,
        );
        let mut witgen = WitnessGenerator::new(state, *test_utils::params::NTXS, block_sender, *test_utils::params::VERBOSE);

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
            }
        }
        witgen.flush_with_nop();
        let block_num = witgen.get_block_generate_num();
        println!(
            "genesis {} blocks (TPS: {})",
            block_num,
            (*test_utils::params::NTXS * block_num) as f32 / timing.elapsed().as_secs_f32()
        );
        Ok(())
    }))
}

fn run(settings: &config::Settings) {
    let (msg_sender, msg_receiver) = crossbeam_channel::unbounded();
    let (blk_sender, blk_receiver) = crossbeam_channel::unbounded();

    let loader_thread = msg_loader::load_msgs_from_mq(&settings.brokers, msg_sender);
    let replay_thread = replay_msgs(msg_receiver, blk_sender);

    let _blocks: Vec<_> = blk_receiver.iter().collect();

    loader_thread.map(|h| h.join().expect("loader thread failed"));
    replay_thread.map(|h| h.join().expect("replay thread failed"));

    // Saves the blocks to DB.
    todo!();
}

fn main() {
    dotenv::dotenv().ok();
    env_logger::init();
    log::info!("state_keeper started");

    let mut conf = config_rs::Config::new();
    let config_file = dotenv::var("CONFIG").unwrap();
    conf.merge(config_rs::File::with_name(&config_file)).unwrap();
    let settings: config::Settings = conf.try_into().unwrap();
    log::debug!("{:?}", settings);

    run(&settings);
}
