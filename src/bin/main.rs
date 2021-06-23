#![allow(clippy::unnecessary_wraps)]
#![allow(dead_code)]

use rollup_state_manager::config;
use rollup_state_manager::msg::{msg_loader, msg_processor};
use rollup_state_manager::params;
use rollup_state_manager::state::{GlobalState, WitnessGenerator};
use rollup_state_manager::test_utils::messages::WrappedMessage;
use rollup_state_manager::types::l2::{L2Block, L2BlockSerde};
use sqlx::postgres::PgPool;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    env_logger::init();
    log::info!("state_keeper started");

    let mut conf = config_rs::Config::new();
    let config_file = dotenv::var("CONFIG").unwrap();
    conf.merge(config_rs::File::with_name(&config_file)).unwrap();
    let settings: config::Settings = conf.try_into().unwrap();
    log::debug!("{:?}", settings);

    run(&settings).await;
}

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

        let mut current_block_num = 0;
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
                WrappedMessage::USER(user) => {
                    processor.handle_user_msg(&mut witgen, user);
                }
            }

            let new_block_num = witgen.get_block_generate_num();
            if new_block_num > current_block_num {
                current_block_num = new_block_num;
                let secs = timing.elapsed().as_secs_f32();
                println!(
                    "generate {} blocks with block_size {} in {}s: average TPS: {}",
                    current_block_num,
                    *params::NTXS,
                    secs,
                    (*params::NTXS * current_block_num) as f32 / secs
                );
            }
        }

        Ok(())
    }))
}

async fn run(settings: &config::Settings) {
    let (msg_sender, msg_receiver) = crossbeam_channel::unbounded();
    let (blk_sender, blk_receiver) = crossbeam_channel::unbounded();

    let loader_thread = msg_loader::load_msgs_from_mq(&settings.brokers, msg_sender);
    let replay_thread = replay_msgs(msg_receiver, blk_sender);

    let db_pool = PgPool::connect(&settings.prover_cluster_db).await.unwrap();
    for block in blk_receiver.iter() {
        save_block_to_db(&db_pool, block).await.unwrap();
    }

    loader_thread.map(|h| h.join().expect("loader thread failed"));
    replay_thread.map(|h| h.join().expect("loader thread failed"));
}

#[derive(sqlx::Type)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
pub enum CircuitType {
    Block,
}

#[derive(sqlx::Type)]
#[sqlx(type_name = "task_status", rename_all = "snake_case")]
enum TaskStatus {
    Inited,
    Witgening,
    Ready,
    Assigned,
    Proved,
}

async fn save_block_to_db(pool: &PgPool, block: L2Block) -> anyhow::Result<()> {
    let input = L2BlockSerde::from(block);
    let task_id = unique_task_id();

    sqlx::query("insert into task (task_id, circuit, input, status) values ($1, $2, $3, $4)")
        .bind(task_id)
        .bind(CircuitType::Block)
        .bind(sqlx::types::Json(input))
        .bind(TaskStatus::Ready)
        .execute(pool)
        .await?;

    Ok(())
}

fn unique_task_id() -> String {
    let current_millis = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
    format!("task_{}", current_millis)
}
