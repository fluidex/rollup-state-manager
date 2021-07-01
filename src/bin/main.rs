#![allow(clippy::unnecessary_wraps)]
#![allow(dead_code)]

use crossbeam_channel::RecvTimeoutError;
use rollup_state_manager::config;
use rollup_state_manager::msg::{msg_loader, msg_processor};
use rollup_state_manager::params;
use rollup_state_manager::state::{GlobalState, WitnessGenerator};
use rollup_state_manager::test_utils::messages::WrappedMessage;
use rollup_state_manager::types::l2::{L2Block, L2BlockSerde};
use rollup_state_manager::types::primitives::fr_to_string;
use sqlx::postgres::PgPool;
use sqlx::Row;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    env_logger::init();
    log::info!("state_keeper started");

    let mut conf = config_rs::Config::new();
    let config_file = dotenv::var("CONFIG").unwrap();
    conf.merge(config_rs::File::with_name(&config_file)).unwrap();
    let settings = config::Settings::set(conf.try_into().unwrap());

    log::debug!("{:?}", settings);

    run(settings).await;
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
        let mut witgen = WitnessGenerator::new(state, *params::NTXS, *params::VERBOSE);

        println!("genesis root {}", witgen.root());

        let mut processor = msg_processor::Processor::default();

        let timing = Instant::now();
        loop {
            // TODO: It is worst to delay for about 119 seconds to send a message since timeout.
            match msg_receiver.recv_timeout(Duration::from_secs(120)) {
                Ok(msg) => {
                    log::debug!("recv new msg {:?}", msg);
                    match msg {
                        WrappedMessage::BALANCE(balance) => {
                            processor.handle_balance_msg(&mut witgen, balance);
                        }
                        WrappedMessage::TRADE(trade) => {
                            processor.handle_trade_msg(&mut witgen, trade);
                        }
                        WrappedMessage::ORDER(order) => {
                            processor.handle_order_msg(&mut witgen, order);
                        }
                        WrappedMessage::USER(user) => {
                            processor.handle_user_msg(&mut witgen, user);
                        }
                    }
                }
                Err(err) => match err {
                    RecvTimeoutError::Timeout => {
                        if witgen.has_raw_tx() {
                            witgen.flush_with_nop();
                        }
                    }
                    RecvTimeoutError::Disconnected => break,
                },
            };

            for block in witgen.pop_all_blocks() {
                block_sender.try_send(block).unwrap();
            }

            let block_num = witgen.get_block_generate_num();
            let secs = timing.elapsed().as_secs_f32();
            println!(
                "generate {} blocks with block_size {} in {}s: average TPS: {}",
                block_num,
                *params::NTXS,
                secs,
                (*params::NTXS * block_num) as f32 / secs
            );
        }

        Ok(())
    }))
}

async fn run(settings: &config::Settings) {
    let (msg_sender, msg_receiver) = crossbeam_channel::unbounded();
    let (blk_sender, blk_receiver) = crossbeam_channel::unbounded();

    let loader_thread = msg_loader::load_msgs_from_mq(&settings.brokers, msg_sender);
    let replay_thread = replay_msgs(msg_receiver, blk_sender);

    let prover_cluster_db_pool = PgPool::connect(&settings.prover_cluster_db).await.unwrap();
    let rollup_state_manager_db_pool = PgPool::connect(&settings.rollup_state_manager_db).await.unwrap();
    let mut check_old_block = true;
    for block in blk_receiver.iter() {
        if check_old_block {
            let is_present = is_present_block(&rollup_state_manager_db_pool, &block).await.unwrap();
            if is_present {
                // skip saving to db
                continue;
            } else {
                // once the old block is not in db, we don't need checking any longer
                check_old_block = false;
            }
        }
        save_block_to_rollup_state_manager_db(&rollup_state_manager_db_pool, &block)
            .await
            .unwrap();
        save_task_to_prover_cluster_db(&prover_cluster_db_pool, block).await.unwrap();
    }

    loader_thread.map(|h| h.join().expect("loader thread failed"));
    replay_thread.map(|h| h.join().expect("loader thread failed"));
}

// Returns true if already present in DB, otherwise false.
async fn is_present_block(pool: &PgPool, block: &L2Block) -> anyhow::Result<bool> {
    match sqlx::query("select new_root from l2block where block_id = $1")
        .bind(block.block_id as u32)
        .fetch_one(pool)
        .await
    {
        Ok(row) => {
            let new_root: String = row.get(0);
            let old_root: String = fr_to_string(&block.witness.new_root);
            if new_root == old_root {
                log::debug!("skip same l2 block {} {}", block.block_id, new_root);
            } else {
                log::error!(
                    "new block {}",
                    serde_json::to_string_pretty(&L2BlockSerde::from(block.witness.clone())).unwrap()
                );
                assert_eq!(
                    new_root, old_root,
                    "l2 block generation must be deterministic! Error for block {}",
                    block.block_id
                );
            }

            Ok(true)
        }
        Err(sqlx::Error::RowNotFound) => Ok(false),
        Err(error) => return Err(error.into()),
    }
}

async fn save_block_to_rollup_state_manager_db(pool: &PgPool, block: &L2Block) -> anyhow::Result<()> {
    let new_root = fr_to_string(&block.witness.new_root);
    let witness = L2BlockSerde::from(block.witness.clone());
    sqlx::query("insert into l2block (block_id, new_root, witness) values ($1, $2, $3)")
        .bind(block.block_id as u32)
        .bind(new_root)
        .bind(sqlx::types::Json(witness))
        .execute(pool)
        .await?;

    Ok(())
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

async fn save_task_to_prover_cluster_db(pool: &PgPool, block: L2Block) -> anyhow::Result<()> {
    let input = L2BlockSerde::from(block.witness);
    let task_id = unique_task_id();

    sqlx::query("insert into task (task_id, circuit, input, status) values ($1, $2, $3, $4)")
        .bind(task_id)
        .bind(CircuitType::Block)
        .bind(sqlx::types::Json(input))
        .bind(TaskStatus::Inited)
        .execute(pool)
        .await?;

    Ok(())
}

fn unique_task_id() -> String {
    let current_millis = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
    format!("task_{}", current_millis)
}
