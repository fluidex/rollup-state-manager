use crate::config::Settings;
use crate::state::global::GlobalState;
use crate::test_utils::types::{get_token_id_by_name, prec_token_id};
use crate::types::l2::L2BlockSerde;
use core::cmp::min;
use fluidex_common::db::models::{l2_block, tablenames};
use fluidex_common::db::DbType;
use fluidex_common::types::FrExt;
use fluidex_common::utils::timeutil::FTimestamp;
use orchestra::rpc::rollup::*;
use std::sync::{Arc, RwLock};
use tonic::{Code, Status};

pub struct Controller {
    db_pool: sqlx::Pool<DbType>,
    state: Arc<RwLock<GlobalState>>,
}

impl Controller {
    pub async fn new(state: Arc<RwLock<GlobalState>>) -> Self {
        let db_pool = sqlx::postgres::PgPool::connect(Settings::db()).await.unwrap();
        Self { db_pool, state }
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
                    block_time: FTimestamp::from(&b.created_time).0,
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
        let txs = detail
            .encoded_txs
            .iter()
            .map(|tx| l2_block_query_response::Tx {
                detail: tx.iter().map(|fr_str| fr_str.0.to_decimal_string()).collect(),
            })
            .collect();

        Ok(L2BlockQueryResponse {
            new_root: l2_block.new_root,
            created_time: FTimestamp::from(&l2_block.created_time).0,
            tx_num,
            real_tx_num: tx_num, // TODO: Needs to decode and filter out NOP txs.
            status: status as i32,
            txs,
        })
    }

    pub fn token_balance_query(&self, request: TokenBalanceQueryRequest) -> Result<TokenBalanceQueryResponse, Status> {
        let token_id = if !request.token_address.is_empty() {
            unimplemented!()
        } else if !request.token_name.is_empty() {
            get_token_id_by_name(&request.token_name)
        } else {
            request.token_id
        };

        let balance = self.state.read().unwrap().get_token_balance(request.account_id, token_id);
        let precision = prec_token_id(token_id);

        Ok(TokenBalanceQueryResponse {
            balance: balance.to_decimal(precision).to_string(),
            balance_raw: balance.to_decimal_string(),
            precision,
        })
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
        "select block_id, new_root, status, l1_tx_hash, detail, created_time
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
        "select block_id, new_root, status, l1_tx_hash, detail, created_time
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
