use crate::grpc::controller::Controller;
use crate::state::global::GlobalState;
use orchestra::rpc::rollup::*;
use std::sync::{Arc, RwLock};
use tonic::{Request, Response, Status};

pub struct Handler {
    controller: Controller,
}

impl Handler {
    pub async fn new(state: Arc<RwLock<GlobalState>>) -> Self {
        Self {
            controller: Controller::new(state).await,
        }
    }
}

#[tonic::async_trait]
impl rollup_state_server::RollupState for Handler {
    async fn l2_blocks_query(&self, request: Request<L2BlocksQueryRequest>) -> Result<Response<L2BlocksQueryResponse>, Status> {
        Ok(Response::new(self.controller.l2_blocks_query(request.into_inner()).await?))
    }

    async fn l2_block_query(&self, request: Request<L2BlockQueryRequest>) -> Result<Response<L2BlockQueryResponse>, Status> {
        Ok(Response::new(self.controller.l2_block_query(request.into_inner()).await?))
    }

    async fn token_balance_query(&self, request: Request<TokenBalanceQueryRequest>) -> Result<Response<TokenBalanceQueryResponse>, Status> {
        Ok(Response::new(self.controller.token_balance_query(request.into_inner())?))
    }
}
