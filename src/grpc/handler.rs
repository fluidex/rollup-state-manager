use crate::grpc::controller::Controller;
use crate::grpc::rpc::*;
use crate::state::global::GlobalState;
use std::sync::{Arc, RwLock};
use tonic::{Request, Response, Status};

pub struct Handler {
    controller: Controller,
}

impl Handler {
    pub fn new(state: Arc<RwLock<GlobalState>>) -> Self {
        Self {
            controller: Controller::new(state),
        }
    }
}

#[tonic::async_trait]
impl rollup_state_server::RollupState for Handler {
    async fn token_balance_query(&self, request: Request<TokenBalanceQueryRequest>) -> Result<Response<TokenBalanceQueryResponse>, Status> {
        Ok(Response::new(self.controller.token_balance_query(request.into_inner())?))
    }
}
