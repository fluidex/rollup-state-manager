use crate::state::global::GlobalState;
use crate::state::witness_generator::WitnessGenerator;
use crate::test_utils::{CircuitTestData, L2BlockSerde};
use serde_json::json;

pub struct Block {
    n_txs: usize,
    account_levels: usize,
    balance_levels: usize,
    order_levels: usize,
    verbose: bool,
}

impl Block {
    pub fn new(n_txs: usize, account_levels: usize, balance_levels: usize, order_levels: usize, verbose: bool) -> Self {
        Self {
            n_txs,
            account_levels,
            balance_levels,
            order_levels,
            verbose,
        }
    }

    pub fn test_data(&self) -> Vec<CircuitTestData> {
        vec![self.empty_block_case()]
    }

    fn block_cases(&self) -> Vec<CircuitTestData> {
        todo!()
    }

    fn empty_block_case(&self) -> CircuitTestData {
        let state = GlobalState::new(self.balance_levels, self.order_levels, self.account_levels, self.verbose);
        let mut witgen = WitnessGenerator::new(state, self.n_txs, self.verbose);
        // we need to have at least 1 account
        witgen.create_new_account(1).unwrap();
        for _ in 0..self.n_txs {
            witgen.nop();
        }
        let block = witgen.forge_all_l2_blocks()[0].clone();
        CircuitTestData {
            name: "empty_block".to_owned(),
            input: json!(L2BlockSerde::from(block)),
            output: json!({}),
        }
    }
}
