use crate::grpc::controller::Controller;
use crate::state::global::GlobalState;
use orchestra::rpc::rollup::*;
use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::{Request, Response, Status};

pub struct Handler {
    controller: Arc<RwLock<Controller>>,
}

impl Handler {
    pub async fn new(state: Arc<std::sync::RwLock<GlobalState>>) -> Self {
        Self {
            controller: Arc::new(RwLock::new(Controller::new(state).await)),
        }
    }
}

#[tonic::async_trait]
impl rollup_state_server::RollupState for Handler {
    async fn l2_blocks_query(&self, request: Request<L2BlocksQueryRequest>) -> Result<Response<L2BlocksQueryResponse>, Status> {
        let controller = self.controller.read().await;
        Ok(Response::new(controller.l2_blocks_query(request.into_inner()).await?))
    }

    async fn l2_block_query(&self, request: Request<L2BlockQueryRequest>) -> Result<Response<L2BlockQueryResponse>, Status> {
        let controller = self.controller.read().await;
        Ok(Response::new(controller.l2_block_query(request.into_inner()).await?))
    }

    async fn token_balance_query(&self, request: Request<TokenBalanceQueryRequest>) -> Result<Response<TokenBalanceQueryResponse>, Status> {
        let controller = self.controller.read().await;
        Ok(Response::new(controller.token_balance_query(request.into_inner())?))
    }

    async fn user_info_query(&self, request: Request<UserInfoQueryRequest>) -> Result<Response<UserInfoQueryResponse>, Status> {
        let controller = self.controller.read().await;
        Ok(Response::new(controller.user_info_query(request.into_inner()).await?))
    }

    async fn register_user(&self, request: Request<RegisterUserRequest>) -> Result<Response<RegisterUserResponse>, Status> {
        // TODO: Dispatch to another thread.
        let mut controller = self.controller.write().await;
        Ok(Response::new(controller.register_user(true, request.into_inner()).await?))
    }
}
