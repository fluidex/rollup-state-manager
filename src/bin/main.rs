#![allow(clippy::unnecessary_wraps)]
#![allow(dead_code)]

use crossbeam_channel::RecvTimeoutError;
use rollup_state_manager::config::Settings;
use rollup_state_manager::db::MIGRATOR;
use rollup_state_manager::grpc::run_grpc_server;
use rollup_state_manager::msg::{msg_loader, msg_processor};
use rollup_state_manager::params;
use rollup_state_manager::r#const::sled_db::*;
use rollup_state_manager::state::{GlobalState, WitnessGenerator};
use rollup_state_manager::test_utils::messages::WrappedMessage;
use rollup_state_manager::types::l2::{L2Block, L2BlockSerde};
use sqlx::postgres::PgPool;
use sqlx::Row;
use std::option::Option::None;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::{fs, io};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();
    log::info!("state_keeper started");

    Settings::init_default();
    log::debug!("{:?}", Settings::get());

    let last_dump = get_latest_dump()?;
    if let Some(id) = last_dump {
        log::info!("found dump #{}", id);
        let db = sled::open(Settings::persist_dir().join(format!("{}.db", id)))?;
        let last_offset = db.get(KAFKA_OFFSET_KEY)?.unwrap();
        let last_offset: i64 = bincode::deserialize(&last_offset)?;
        run(Some(last_offset), Some(db)).await;
    } else {
        run(None, None).await;
    }

    Ok(())
}

fn grpc_run(state: Arc<RwLock<GlobalState>>) -> Option<std::thread::JoinHandle<anyhow::Result<()>>> {
    Some(std::thread::spawn(move || {
        let addr = Settings::grpc_addr().parse()?;
        run_grpc_server(addr, state)
    }))
}

fn replay_msgs(
    msg_receiver: crossbeam_channel::Receiver<WrappedMessage>,
    block_sender: crossbeam_channel::Sender<L2Block>,
    state: Arc<RwLock<GlobalState>>,
    db: Option<sled::Db>,
) -> Option<std::thread::JoinHandle<anyhow::Result<()>>> {
    Some(std::thread::spawn(move || {
        let block_offset: Option<usize> = if let Some(db) = db {
            state.write().unwrap().load_persist(&db).unwrap();
            db.get(BLOCK_OFFSET_KEY).ok().flatten().and_then(|v| bincode::deserialize(&v).ok())
        } else {
            None
        };
        let mut witgen = WitnessGenerator::new(state, *params::NTXS, block_offset, *params::VERBOSE);

        println!("genesis root {}", witgen.root().to_string());

        let mut processor = msg_processor::Processor::default();

        let timing = Instant::now();
        loop {
            // In the worst case we wait for about 119 seconds timeout until we try to
            // generate a block, if there's any tx.
            // TODO: dynamic timeout
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

async fn run(offset: Option<i64>, db: Option<sled::Db>) {
    let state = Arc::new(RwLock::new(GlobalState::new(
        *params::BALANCELEVELS,
        *params::ORDERLEVELS,
        *params::ACCOUNTLEVELS,
        *params::VERBOSE,
    )));

    let (msg_sender, msg_receiver) = crossbeam_channel::unbounded();
    let (blk_sender, blk_receiver) = crossbeam_channel::unbounded();

    let loader_thread = msg_loader::load_msgs_from_mq(Settings::brokers(), offset, msg_sender);
    let replay_thread = replay_msgs(msg_receiver, blk_sender, Arc::clone(&state), db);
    let server_thread = grpc_run(state);

    // TODO: It should be better to integrate prover-cluster DB migrations with CI.
    let prover_cluster_db_pool = if dotenv::var("CI").map_or_else(|_| false, |v| v.parse().unwrap()) {
        None
    } else {
        Some(PgPool::connect(Settings::prover_cluster_db()).await.unwrap())
    };

    let rollup_state_manager_db_pool = PgPool::connect(Settings::rollup_state_manager_db()).await.unwrap();
    MIGRATOR.run(&rollup_state_manager_db_pool).await.ok();

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
        if let Some(db_pool) = &prover_cluster_db_pool {
            save_task_to_prover_cluster_db(db_pool, block).await.unwrap();
        }
    }

    loader_thread.map(|h| h.join().expect("loader thread failed"));
    replay_thread.map(|h| h.join().expect("loader thread failed"));
    server_thread.map(|h| h.join().expect("loader thread failed"));
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
            let old_root: String = block.witness.new_root.to_string();
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
        Err(error) => Err(error.into()),
    }
}

async fn save_block_to_rollup_state_manager_db(pool: &PgPool, block: &L2Block) -> anyhow::Result<()> {
    let new_root = block.witness.new_root.to_string();
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

    sqlx::query("insert into task (task_id, circuit, block_id, input, status) values ($1, $2, $3, $4, $5)")
        .bind(task_id)
        .bind(CircuitType::Block)
        .bind(block.block_id as i64) // TODO: will it overflow?
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

fn get_latest_dump() -> anyhow::Result<Option<usize>> {
    let mut dumps = std::fs::read_dir(Settings::persist_dir())?
        .map(|entry| entry.and_then(|e| e.metadata().map(|meta| (meta, e.file_name().into_string().unwrap()))))
        .collect::<io::Result<Vec<(fs::Metadata, String)>>>()?
        .into_iter()
        .filter_map(|(meta, name)| if meta.is_dir() && name.ends_with(".db") { Some(name) } else { None })
        .map(|path| path.strip_suffix(".db").unwrap().parse::<usize>())
        .collect::<Result<Vec<usize>, _>>()?;
    dumps.sort_unstable();
    Ok(dumps.last().copied())
}
