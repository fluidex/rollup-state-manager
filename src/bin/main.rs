#![allow(clippy::unnecessary_wraps)]
#![allow(dead_code)]

use crossbeam_channel::RecvTimeoutError;
use fluidex_common::db::models::tablenames;
use fluidex_common::db::models::task::TaskStatus;
use fluidex_common::db::MIGRATOR;
use fluidex_common::non_blocking_tracing;
use fluidex_common::types::FrExt;
use rollup_state_manager::config::Settings;
use rollup_state_manager::grpc::run_grpc_server;
use rollup_state_manager::msg::{msg_loader, msg_processor};
use rollup_state_manager::params;
#[cfg(feature = "persist_sled")]
use rollup_state_manager::r#const::sled_db::*;
use rollup_state_manager::state::{GlobalState, ManagerWrapper};
use rollup_state_manager::test_utils::messages::WrappedMessage;
use rollup_state_manager::types::l2::{L2Block, L2BlockSerde};
use sqlx::postgres::PgPool;
use sqlx::Row;
use std::option::Option::None;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::{fs, io};

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let _guard = non_blocking_tracing::setup();
    log::info!("state_keeper started");

    Settings::init_default();
    log::debug!("{:?}", Settings::get());

    run().await;
}

fn grpc_run(state: Arc<RwLock<GlobalState>>) -> Option<std::thread::JoinHandle<anyhow::Result<()>>> {
    Some(std::thread::spawn(move || {
        let addr = Settings::grpc_addr().parse()?;
        run_grpc_server(addr, state)
    }))
}

fn process_msgs(
    msg_receiver: crossbeam_channel::Receiver<WrappedMessage>,
    block_sender: crossbeam_channel::Sender<L2Block>,
    state: Arc<RwLock<GlobalState>>,
    block_offset: Option<usize>,
) -> Option<std::thread::JoinHandle<anyhow::Result<()>>> {
    Some(std::thread::spawn(move || {
        let manager = ManagerWrapper::new(state, *params::NTXS, block_offset, *params::VERBOSE);
        // TODO: change to to_hex_string, remove 'Fr(' and ')'
        log::info!("genesis root {}", manager.root().to_string());

        run_msg_processor(msg_receiver, block_sender, manager)
    }))
}

async fn run() {
    let state = Arc::new(RwLock::new(GlobalState::new(
        *params::BALANCELEVELS,
        *params::ORDERLEVELS,
        *params::ACCOUNTLEVELS,
        *params::VERBOSE,
    )));

    let (block_offset, kafka_offset) = get_persistent_offsets(Arc::clone(&state));

    let (msg_sender, msg_receiver) = crossbeam_channel::unbounded();
    let (blk_sender, blk_receiver) = crossbeam_channel::unbounded();

    let loader_thread = msg_loader::load_msgs_from_mq(Settings::brokers(), kafka_offset, msg_sender);
    let replay_thread = process_msgs(msg_receiver, blk_sender, Arc::clone(&state), block_offset);
    let server_thread = grpc_run(state);

    let db_pool = PgPool::connect(Settings::db()).await.unwrap();
    MIGRATOR.run(&db_pool).await.ok();

    for block in blk_receiver.iter() {
        save_block_to_db(&db_pool, &block).await.unwrap();
        save_task_to_db(&db_pool, block).await.unwrap();
    }

    loader_thread.map(|h| h.join().expect("loader thread failed"));
    replay_thread.map(|h| h.join().expect("loader thread failed"));
    server_thread.map(|h| h.join().expect("loader thread failed"));
}

fn run_msg_processor(
    msg_receiver: crossbeam_channel::Receiver<WrappedMessage>,
    block_sender: crossbeam_channel::Sender<L2Block>,
    mut manager: ManagerWrapper,
) -> anyhow::Result<()> {
    let rt: tokio::runtime::Runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Build runtime");

    rt.block_on(async {
        let mut processor = msg_processor::Processor::default();

        let timing = Instant::now();
        let db_pool = PgPool::connect(Settings::db()).await.unwrap();
        let mut old_block_check = true;
        let mut old_block_num = 0;
        loop {
            // In the worst case we wait for about 119 seconds timeout until we try to
            // generate a block, if there's any tx.
            // TODO: dynamic timeout
            match msg_receiver.recv_timeout(Duration::from_secs(120)) {
                Ok(msg) => {
                    log::debug!("recv new msg {:?}", msg);
                    match msg {
                        WrappedMessage::DEPOSIT(deposit) => {
                            processor.handle_deposit_msg(&mut manager, deposit);
                        }
                        WrappedMessage::ORDER(order) => {
                            processor.handle_order_msg(&mut manager, order);
                        }
                        WrappedMessage::TRADE(trade) => {
                            processor.handle_trade_msg(&mut manager, trade);
                        }
                        WrappedMessage::TRANSFER(transfer) => {
                            processor.handle_transfer_msg(&mut manager, transfer);
                        }
                        WrappedMessage::USER(user) => {
                            processor.handle_user_msg(&mut manager, user);
                        }
                        WrappedMessage::WITHDRAW(withdraw) => {
                            processor.handle_withdraw_msg(&mut manager, withdraw);
                        }
                    }
                }
                Err(err) => match err {
                    RecvTimeoutError::Timeout => {
                        if manager.has_raw_tx() {
                            manager.flush_with_nop();
                        }
                    }
                    RecvTimeoutError::Disconnected => break,
                },
            };

            for block in manager.pop_all_blocks() {
                if old_block_check && is_present_block(&db_pool, &block).await.unwrap() {
                    // Skips this old block.
                    old_block_num += 1;
                    continue;
                }

                // Once the block is a new one, no need to check if old.
                old_block_check = false;

                block_sender.try_send(block).unwrap();
            }

            let block_num = manager.get_block_generate_num() - old_block_num;
            let secs = timing.elapsed().as_secs_f32();
            log::info!(
                "generate {} blocks with block_size {} in {}s: average TPS: {}",
                block_num,
                *params::NTXS,
                secs,
                (*params::NTXS * block_num) as f32 / secs
            );
        }

        Ok(())
    })
}

// Returns true if already present in DB, otherwise false.
async fn is_present_block(pool: &PgPool, block: &L2Block) -> anyhow::Result<bool> {
    match sqlx::query(&format!("select new_root from {} where block_id = $1", tablenames::L2_BLOCK))
        .bind(block.block_id as u32)
        .fetch_one(pool)
        .await
    {
        Ok(row) => {
            let new_root: String = row.get(0);
            let old_root: String = block.detail.new_root.to_hex_string();
            if new_root == old_root {
                log::debug!("skip same l2 block {} {}", block.block_id, new_root);
            } else {
                log::error!(
                    "new block {}",
                    serde_json::to_string_pretty(&L2BlockSerde::from(block.detail.clone())).unwrap()
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

async fn save_block_to_db(pool: &PgPool, block: &L2Block) -> anyhow::Result<()> {
    let new_root = block.detail.new_root.to_hex_string();
    let detail = L2BlockSerde::from(block.detail.clone());
    sqlx::query(&format!(
        "insert into {} (block_id, new_root, detail, raw_public_data) values ($1, $2, $3, $4)",
        tablenames::L2_BLOCK
    ))
    .bind(block.block_id as u32)
    .bind(new_root)
    .bind(sqlx::types::Json(detail))
    .bind(&block.public_data)
    .execute(pool)
    .await?;

    Ok(())
}

async fn save_task_to_db(pool: &PgPool, block: L2Block) -> anyhow::Result<()> {
    let tx_num = block.detail.encoded_txs.len();
    let input = L2BlockSerde::from(block.detail);
    let task_id = unique_task_id();

    sqlx::query(&format!(
        "insert into {} (task_id, circuit, block_id, input, status) values ($1, $2, $3, $4, $5)",
        tablenames::TASK
    ))
    .bind(task_id)
    .bind(format!("block_{}", tx_num))
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

#[cfg(feature = "persist_sled")]
fn get_block_offset(db: &Option<sled::Db>, state: Arc<RwLock<GlobalState>>) -> Option<usize> {
    db.as_ref().and_then(|db| {
        state.write().unwrap().load_persist(db).unwrap();
        db.get(BLOCK_OFFSET_KEY).ok().flatten().and_then(|v| bincode::deserialize(&v).ok())
    })
}

#[cfg(feature = "persist_sled")]
fn get_kafka_offset(db: &Option<sled::Db>) -> Option<i64> {
    db.as_ref()
        .and_then(|db| db.get(KAFKA_OFFSET_KEY).ok().flatten().and_then(|v| bincode::deserialize(&v).ok()))
}

cfg_if::cfg_if! {
    if #[cfg(feature = "persist_sled")] {
        fn get_persistent_offsets(state: Arc<RwLock<GlobalState>>) -> (Option<usize>, Option<i64>) {
            get_latest_dump().unwrap().map_or_else(
                || (None, None),
                |id| {
                log::info!("found dump #{}", id);
                let db = sled::open(Settings::persist_dir().join(format!("{}.db", id))).ok();
                (get_block_offset(&db, state), get_kafka_offset(&db))
                })
        }
    } else {
        fn get_persistent_offsets(_state: Arc<RwLock<GlobalState>>) -> (Option<usize>, Option<i64>) {
            (None, None)
        }
    }
}
