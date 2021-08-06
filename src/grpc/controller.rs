use crate::config::Settings;
use crate::grpc::rpc::*;
use crate::state::global::GlobalState;
use crate::test_utils::types::{get_token_id_by_name, prec_token_id};
use crate::types::l2::L2BlockSerde;
use fluidex_common::db::models::{l2_block, tablenames, task};
use fluidex_common::db::DbType;
use fluidex_common::types::FrExt;
use fluidex_common::utils::timeutil::FTimestamp;
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

    // TODO: offset can be optional?
    // default 0? what about genesis block?
    // default -1?
    pub async fn l2_blocks_query(&self, request: L2BlocksQueryRequest) -> Result<L2BlocksQueryResponse, Status> {
        // db begin tx
        // query sum

        let stmt = format!(
            "select block_id, new_root, witness, created_time
            from {}
            where block_id = $1
            order by created_time desc limit {}",
            tablenames::L2_BLOCK,
            request.limit,
        );

        let blocks: Vec<l2_block::L2Block> = sqlx::query_as::(&stmt)
            .bind(request.offset)
            .fetch_all(db_pool)
            .await;

        unimplemented!();
    }

    pub async fn l2_block_query(&self, request: L2BlockQueryRequest) -> Result<L2BlockQueryResponse, Status> {
        let block_id = request.block_id;
        let l2_block = get_l2_block_by_id(&self.db_pool, block_id).await?;

        let status = match get_task_status_by_block_id(&self.db_pool, block_id).await? {
            task::TaskStatus::Inited => TaskStatus::Inited,
            task::TaskStatus::Witgening => TaskStatus::Witgening,
            task::TaskStatus::Ready => TaskStatus::Ready,
            task::TaskStatus::Assigned => TaskStatus::Assigned,
            task::TaskStatus::Proved => TaskStatus::Proved,
        };

        let witness: L2BlockSerde = serde_json::from_value(l2_block.witness).unwrap();
        let tx_num = witness.encoded_txs.len() as u64;
        let txs = witness
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

async fn get_l2_block_by_id(db_pool: &sqlx::Pool<DbType>, block_id: i64) -> Result<l2_block::L2Block, Status> {
    let stmt = format!(
        "select block_id, new_root, witness, created_time
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

async fn get_task_status_by_block_id(db_pool: &sqlx::Pool<DbType>, block_id: i64) -> Result<task::TaskStatus, Status> {
    let stmt = format!(
        "select status
        from {}
        where block_id = $1
        order by created_time desc limit 1",
        tablenames::TASK,
    );
    match sqlx::query_as(&stmt).bind(block_id).fetch_one(db_pool).await {
        Ok((task_status,)) => Ok(task_status),
        Err(sqlx::Error::RowNotFound) => Err(Status::new(Code::NotFound, "db task record not found")),
        Err(_) => Err(Status::new(Code::Internal, "db table task fetch error")),
    }
}
