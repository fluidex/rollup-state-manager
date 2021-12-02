use super::sequencer::Sequencer;
use super::user_cache::UserCache;
use crate::config::Settings;
use crate::message::{FullOrderMessageManager, SimpleMessageManager};
use crate::persist::history::DatabaseHistoryWriter;
use crate::persist::persistor::{CompositePersistor, DBBasedPersistor, FileBasedPersistor, MessengerBasedPersistor, PersistExector};
use crate::state::global::GlobalState;
use crate::storage::database::{DatabaseWriterConfig, OperationLogSender};
use crate::test_utils::types::{get_token_id_by_name, prec_token_id};
use crate::types::l2::{tx_detail_idx, AmountType, L2BlockSerde, TxType};
use core::cmp::min;
use fluidex_common::db::models::{account, l2_block, operation_log, tablenames};
use fluidex_common::db::DbType;
use fluidex_common::num_traits::ToPrimitive;
use fluidex_common::rust_decimal::Decimal;
use fluidex_common::types::FrExt;
use fluidex_common::utils::timeutil::{current_timestamp, FTimestamp};
use orchestra::rpc::rollup::*;
use serde::Serialize;
use std::sync::{Arc, RwLock};
use tonic::{Code, Status};

const OPERATION_REGISTER_USER: &str = "register_user";

pub struct Controller {
    db_pool: sqlx::Pool<DbType>,
    log_handler: Box<dyn OperationLogConsumer + Send + Sync>,
    persistor: Box<dyn PersistExector>,
    sequencer: Sequencer,
    state: Arc<RwLock<GlobalState>>,
    user_cache: Arc<RwLock<UserCache>>,
}

impl Controller {
    pub async fn new(state: Arc<RwLock<GlobalState>>) -> Self {
        let db_pool = sqlx::postgres::PgPool::connect(Settings::db()).await.unwrap();
        let log_handler = Box::new(
            OperationLogSender::new(&DatabaseWriterConfig {
                spawn_limit: 4,
                apply_benchmark: true,
                capability_limit: 8192,
            })
            .start_schedule(&db_pool)
            .unwrap(),
        );
        let persistor = create_persistor();
        let sequencer = Sequencer::default();
        let user_cache = Arc::new(RwLock::new(UserCache::new()));

        Self {
            db_pool,
            log_handler,
            persistor,
            sequencer,
            state,
            user_cache,
        }
    }

    // TODO: cache
    pub async fn l2_blocks_query(&self, request: L2BlocksQueryRequest) -> Result<L2BlocksQueryResponse, Status> {
        let (total, blocks) = get_l2_blocks(&self.db_pool, request).await.map_err(|e| {
            log::error!("{:?}", e);
            Status::new(Code::Internal, "db l2_blocks query error")
        })?;

        Ok(L2BlocksQueryResponse {
            total,
            blocks: blocks
                .iter()
                .map(|b| l2_blocks_query_response::BlockSummary {
                    block_height: b.block_id,
                    merkle_root: b.new_root.clone(),
                    block_time: FTimestamp::from(&b.created_time).as_milliseconds(),
                })
                .collect(),
        })
    }

    // TODO: cache
    pub async fn l2_block_query(&self, request: L2BlockQueryRequest) -> Result<L2BlockQueryResponse, Status> {
        let block_id = request.block_id;
        let l2_block = get_l2_block_by_id(&self.db_pool, block_id).await?;

        let status = match get_status_by_block_id(&self.db_pool, block_id).await? {
            l2_block::BlockStatus::Uncommited => BlockStatus::Uncommited,
            l2_block::BlockStatus::Commited => BlockStatus::Commited,
            l2_block::BlockStatus::Verified => BlockStatus::Verified,
        };

        let detail: L2BlockSerde = serde_json::from_value(l2_block.detail).unwrap();
        let tx_num = detail.encoded_txs.len() as u64;
        let real_tx_num = detail.txs_type.clone().into_iter().filter(|t| *t != TxType::Nop).count();

        let mut txs = vec![];
        let mut decoded_txs = vec![];
        let mut txs_type = vec![];
        for (tx, tx_type) in detail.encoded_txs.iter().zip(detail.txs_type.into_iter()) {
            txs.push(l2_block_query_response::EncodedTx {
                detail: tx.iter().map(|fr_str| fr_str.0.to_decimal_string()).collect(),
            });

            let mut decoded_tx = l2_block_query_response::DecodedTx::default();
            match tx_type {
                TxType::Deposit => {
                    let account_id = tx[tx_detail_idx::ACCOUNT_ID1].0.to_u32();
                    debug_assert!(account_id == tx[tx_detail_idx::ACCOUNT_ID2].0.to_u32());

                    let token_id = tx[tx_detail_idx::TOKEN_ID1].0.to_u32();
                    debug_assert!(token_id == tx[tx_detail_idx::TOKEN_ID2].0.to_u32());

                    let precision = prec_token_id(token_id);
                    let amount = tx[tx_detail_idx::AMOUNT].0.to_decimal(precision).to_string();

                    let old_balance = tx[tx_detail_idx::BALANCE1].0.to_decimal(precision).to_string();
                    let new_balance = tx[tx_detail_idx::BALANCE2].0.to_decimal(precision).to_string();

                    decoded_tx.deposit_tx = Some(DepositTx {
                        account_id,
                        token_id,
                        amount,
                        old_balance,
                        new_balance,
                    })
                }
                TxType::Withdraw => {
                    let account_id = tx[tx_detail_idx::ACCOUNT_ID1].0.to_u32();

                    let token_id = tx[tx_detail_idx::TOKEN_ID1].0.to_u32();
                    debug_assert!(token_id == tx[tx_detail_idx::TOKEN_ID2].0.to_u32());

                    let precision = prec_token_id(token_id);
                    let amount = tx[tx_detail_idx::AMOUNT].0.to_decimal(precision).to_string();

                    let old_balance = tx[tx_detail_idx::BALANCE1].0.to_decimal(precision).to_string();
                    let new_balance = tx[tx_detail_idx::BALANCE2].0.to_decimal(precision).to_string();

                    decoded_tx.withdraw_tx = Some(WithdrawTx {
                        account_id,
                        token_id,
                        amount,
                        old_balance,
                        new_balance,
                    })
                }
                TxType::Transfer => {
                    let from = tx[tx_detail_idx::ACCOUNT_ID1].0.to_u32();
                    let to = tx[tx_detail_idx::ACCOUNT_ID2].0.to_u32();

                    let token_id = tx[tx_detail_idx::TOKEN_ID1].0.to_u32();
                    debug_assert!(token_id == tx[tx_detail_idx::TOKEN_ID2].0.to_u32());

                    let precision = prec_token_id(token_id);
                    let amount = tx[tx_detail_idx::AMOUNT].0;

                    let from_old_balance = tx[tx_detail_idx::BALANCE1].0;
                    let from_new_balance = from_old_balance.sub(&amount).to_decimal(precision).to_string();
                    let from_old_balance = from_old_balance.to_decimal(precision).to_string();

                    let to_new_balance = tx[tx_detail_idx::BALANCE1].0;
                    let to_old_balance = to_new_balance.sub(&amount).to_decimal(precision).to_string();
                    let to_new_balance = to_new_balance.to_decimal(precision).to_string();

                    let amount = tx[tx_detail_idx::AMOUNT].0.to_decimal(precision).to_string();

                    decoded_tx.transfer_tx = Some(TransferTx {
                        from,
                        to,
                        token_id,
                        amount,
                        from_old_balance,
                        from_new_balance,
                        to_old_balance,
                        to_new_balance,
                    })
                }
                TxType::SpotTrade => {
                    let order1_account_id = tx[tx_detail_idx::ACCOUNT_ID1].0.to_u32();
                    let order2_account_id = tx[tx_detail_idx::ACCOUNT_ID2].0.to_u32();

                    let token_id_1to2 = tx[tx_detail_idx::NEW_ORDER1_TOKEN_SELL].0.to_u32();
                    let token_id_2to1 = tx[tx_detail_idx::NEW_ORDER2_TOKEN_SELL].0.to_u32();

                    let amount1 = tx[tx_detail_idx::AMOUNT1].0;
                    let amount2 = tx[tx_detail_idx::AMOUNT2].0;

                    let balance1 = tx[tx_detail_idx::BALANCE1].0;
                    let balance2 = tx[tx_detail_idx::BALANCE2].0;
                    let balance3 = tx[tx_detail_idx::BALANCE3].0;
                    let balance4 = tx[tx_detail_idx::BALANCE4].0;

                    let precision_1to2 = prec_token_id(token_id_1to2);
                    let precision_2to1 = prec_token_id(token_id_2to1);

                    let amount_1to2 = amount1.to_decimal(prec_token_id(token_id_1to2)).to_string();
                    let amount_2to1 = amount2.to_decimal(prec_token_id(token_id_2to1)).to_string();

                    let account1_token_sell_old_balance = balance1.to_decimal(precision_1to2).to_string();
                    let account1_token_sell_new_balance = balance1.sub(&amount1).to_decimal(precision_1to2).to_string();
                    let account1_token_buy_old_balance = balance4.sub(&amount2).to_decimal(precision_2to1).to_string();
                    let account1_token_buy_new_balance = balance4.to_decimal(precision_2to1).to_string();
                    let account2_token_sell_old_balance = balance3.to_decimal(precision_2to1).to_string();
                    let account2_token_sell_new_balance = balance3.sub(&amount2).to_decimal(precision_2to1).to_string();
                    let account2_token_buy_old_balance = balance2.sub(&amount1).to_decimal(precision_1to2).to_string();
                    let account2_token_buy_new_balance = balance2.to_decimal(precision_1to2).to_string();

                    decoded_tx.spot_trade_tx = Some(SpotTradeTx {
                        order1_account_id,
                        order2_account_id,
                        token_id_1to2,
                        token_id_2to1,
                        amount_1to2,
                        amount_2to1,
                        account1_token_buy_new_balance,
                        account1_token_buy_old_balance,
                        account1_token_sell_new_balance,
                        account1_token_sell_old_balance,
                        account2_token_buy_new_balance,
                        account2_token_buy_old_balance,
                        account2_token_sell_new_balance,
                        account2_token_sell_old_balance,
                    })
                }
                _ => (),
            };
            decoded_txs.push(decoded_tx);

            txs_type.push(tx_type as i32);
        }

        Ok(L2BlockQueryResponse {
            tx_num,
            real_tx_num: real_tx_num as u64,
            created_time: FTimestamp::from(&l2_block.created_time).as_milliseconds(),
            status: status as i32,
            new_root: l2_block.new_root,
            l1_tx_hash: l2_block.l1_tx_hash.unwrap_or_else(|| "".to_owned()),
            txs,
            decoded_txs,
            txs_type,
        })
    }

    pub fn token_balance_query(&self, request: TokenBalanceQueryRequest) -> Result<TokenBalanceQueryResponse, Status> {
        let token_id = if let Some(token_id) = request.token_id {
            token_id
        } else if let Some(_token_address) = request.token_address {
            unimplemented!()
        } else if let Some(token_name) = request.token_name {
            get_token_id_by_name(&token_name)
        } else {
            return Err(Status::new(
                Code::InvalidArgument,
                "Must specify one of token_id, token_address or token_name",
            ));
        };

        let balance = self.state.read().unwrap().get_token_balance(request.account_id, token_id);
        let precision = prec_token_id(token_id);

        Ok(TokenBalanceQueryResponse {
            balance: balance.to_decimal(precision).to_string(),
            balance_raw: balance.to_decimal_string(),
            precision,
        })
    }

    pub async fn user_info_query(&self, request: UserInfoQueryRequest) -> Result<UserInfoQueryResponse, Status> {
        if let Some(user_info) = self
            .user_cache
            .read()
            .unwrap()
            .get_user_info(request.user_id, &request.l1_address, &request.l2_pubkey)
        {
            let user_id = user_info.id as u32;
            let user_info = Some(UserInfo {
                user_id,
                l1_address: user_info.l1_address.clone(),
                l2_pubkey: user_info.l2_pubkey.clone(),
                nonce: self.state.read().unwrap().get_account_nonce(user_id).to_u32(),
            });
            return Ok(UserInfoQueryResponse { user_info });
        }

        let user_info = get_user_info_from_db(&self.db_pool, request).await?;
        self.user_cache.write().unwrap().set_user_info(user_info.clone());

        let user_id = user_info.id as u32;
        let user_info = Some(UserInfo {
            user_id,
            l1_address: user_info.l1_address,
            l2_pubkey: user_info.l2_pubkey,
            nonce: self.state.read().unwrap().get_account_nonce(user_id).to_u32(),
        });
        Ok(UserInfoQueryResponse { user_info })
    }

    pub async fn register_user(&mut self, real: bool, request: RegisterUserRequest) -> Result<RegisterUserResponse, Status> {
        let user_id = request.user_id;
        let user = account::AccountDesc {
            id: user_id as i32,
            l1_address: request.l1_address.to_lowercase(),
            l2_pubkey: request.l2_pubkey.to_lowercase(),
        };

        self.user_cache.write().unwrap().set_user_info(user.clone());

        if real {
            self.persistor.register_user(user.clone());
            self.append_operation_log(OPERATION_REGISTER_USER, &request);
        }

        Ok(RegisterUserResponse {
            user_info: Some(UserInfo {
                user_id,
                l1_address: user.l1_address,
                l2_pubkey: user.l2_pubkey,
                nonce: self.state.read().unwrap().get_account_nonce(user_id).to_u32(),
            }),
        })
    }

    fn append_operation_log<Operation>(&mut self, method: &str, req: &Operation)
    where
        Operation: Serialize,
    {
        let params = serde_json::to_string(req).unwrap();
        let operation_log = operation_log::OperationLog {
            id: self.sequencer.next_operation_log_id() as i64,
            time: FTimestamp(current_timestamp()).into(),
            method: method.to_owned(),
            params,
        };
        (*self.log_handler).append_operation_log(operation_log).ok();
    }
}

trait OperationLogConsumer {
    fn is_block(&self) -> bool;
    fn append_operation_log(&mut self, item: operation_log::OperationLog) -> anyhow::Result<(), operation_log::OperationLog>;
}

impl OperationLogConsumer for OperationLogSender {
    fn is_block(&self) -> bool {
        self.is_block()
    }
    fn append_operation_log(&mut self, item: operation_log::OperationLog) -> anyhow::Result<(), operation_log::OperationLog> {
        self.append(item)
    }
}

async fn get_l2_blocks(
    db_pool: &sqlx::Pool<DbType>,
    request: L2BlocksQueryRequest,
) -> Result<(i64, Vec<l2_block::L2Block>), anyhow::Error> {
    let mut tx = db_pool.begin().await?;

    let count_query = format!("select block_id from {} order by block_id desc limit 1", tablenames::L2_BLOCK);
    // "total"'s type needs to be consistent with block_id
    let total: i64 = match sqlx::query_scalar::<_, i64>(&count_query).fetch_one(&mut tx).await {
        Ok(max_block_id) => max_block_id + 1,
        Err(sqlx::Error::RowNotFound) => return Ok((0, vec![])),
        Err(error) => return Err(error.into()),
    };

    let limit = if request.limit.is_positive() { request.limit } else { 10 };

    let limit = min(100, limit);
    let blocks_query = format!(
        "select block_id, new_root, raw_public_data, status, l1_tx_hash, detail, created_time
            from {}
            where block_id <= $1
            order by block_id desc limit {}",
        tablenames::L2_BLOCK,
        limit,
    );
    let blocks: Vec<l2_block::L2Block> = sqlx::query_as::<_, l2_block::L2Block>(&blocks_query)
        .bind(total - request.offset)
        .fetch_all(&mut tx)
        .await?;

    tx.commit().await?;

    Ok((total, blocks))
}

async fn get_l2_block_by_id(db_pool: &sqlx::Pool<DbType>, block_id: i64) -> Result<l2_block::L2Block, Status> {
    let stmt = format!(
        "select block_id, new_root, raw_public_data, status, l1_tx_hash, detail, created_time
        from {}
        where block_id = $1
        order by created_time desc limit 1",
        tablenames::L2_BLOCK,
    );
    match sqlx::query_as::<_, l2_block::L2Block>(&stmt)
        .bind(block_id)
        .fetch_one(db_pool)
        .await
    {
        Ok(l2_block) => Ok(l2_block),
        Err(sqlx::Error::RowNotFound) => Err(Status::new(Code::NotFound, "db l2_block record not found")),
        Err(err) => {
            log::error!("{:?}", err);
            Err(Status::new(Code::Internal, "db table l2_block fetch error"))
        }
    }
}

async fn get_status_by_block_id(db_pool: &sqlx::Pool<DbType>, block_id: i64) -> Result<l2_block::BlockStatus, Status> {
    let stmt = format!(
        "select status
        from {}
        where block_id = $1
        order by created_time desc limit 1",
        tablenames::L2_BLOCK,
    );
    match sqlx::query_as(&stmt).bind(block_id).fetch_one(db_pool).await {
        Ok((status,)) => Ok(status),
        Err(sqlx::Error::RowNotFound) => Err(Status::new(Code::NotFound, "db l2_block record not found")),
        Err(_) => Err(Status::new(Code::Internal, "db table l2_block fetch error")),
    }
}

async fn get_user_info_from_db(db_pool: &sqlx::Pool<DbType>, request: UserInfoQueryRequest) -> Result<account::AccountDesc, Status> {
    let query = format!(
        "select * from {} where id = $1 OR l1_address = $2 OR l2_pubkey = $2",
        tablenames::ACCOUNT
    );
    sqlx::query_as(&query)
        .bind(request.user_id.unwrap_or(0))
        .bind(request.l1_address.unwrap_or_else(|| "".to_owned()))
        .bind(request.l2_pubkey.unwrap_or_else(|| "".to_owned()))
        .fetch_one(db_pool)
        .await
        .map_err(|_| Status::not_found("no specified user info"))
}

// TODO: reuse pool of two dbs when they are same?
fn create_persistor() -> Box<dyn PersistExector> {
    let persist_to_mq = true;
    let persist_to_mq_full_order = true;
    let persist_to_db = false;
    let persist_to_file = false;
    let mut persistor = Box::new(CompositePersistor::default());
    if !Settings::brokers().is_empty() && persist_to_mq {
        persistor.add_persistor(Box::new(MessengerBasedPersistor::new(Box::new(
            SimpleMessageManager::new_and_run(Settings::brokers()).unwrap(),
        ))));
    }
    if !Settings::brokers().is_empty() && persist_to_mq_full_order {
        persistor.add_persistor(Box::new(MessengerBasedPersistor::new(Box::new(
            FullOrderMessageManager::new_and_run(Settings::brokers()).unwrap(),
        ))));
    }
    if persist_to_db {
        // persisting to db is disabled now
        let pool = sqlx::Pool::<DbType>::connect_lazy(Settings::db()).unwrap();
        persistor.add_persistor(Box::new(DBBasedPersistor::new(Box::new(
            DatabaseHistoryWriter::new(
                &DatabaseWriterConfig {
                    spawn_limit: 4,
                    apply_benchmark: true,
                    capability_limit: 8192,
                },
                &pool,
            )
            .unwrap(),
        ))));
    }
    if Settings::brokers().is_empty() || persist_to_file {
        persistor.add_persistor(Box::new(FileBasedPersistor::new("persistor_output.txt")));
    }
    persistor
}
