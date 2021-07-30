use crate::grpc::rpc::*;
use crate::state::global::GlobalState;
use crate::test_utils::types::{get_token_id_by_name, prec_token_id};
use fluidex_common::types::FrExt;
use std::sync::{Arc, RwLock};
use tonic::Status;

pub struct Controller {
    state: Arc<RwLock<GlobalState>>,
}

impl Controller {
    pub fn new(state: Arc<RwLock<GlobalState>>) -> Self {
        Self { state }
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
