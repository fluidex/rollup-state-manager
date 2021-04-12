#![allow(non_snake_case)]
#![allow(clippy::assertions_on_constants)]
use crate::state::common::TX_LENGTH;
use crate::types::primitives::Fr;
use ff::Field;
#[derive(Default)]
pub struct PlaceOrderTxData {
    pub order_pos: Fr,
    pub old_order_id: Fr,
    pub new_order_id: Fr,
    pub old_order_tokensell: Fr,
    pub old_order_filledsell: Fr,
    pub old_order_amountsell: Fr,
    pub old_order_tokenbuy: Fr,
    pub old_order_filledbuy: Fr,
    pub old_order_amountbuy: Fr,
    pub new_order_tokensell: Fr,
    pub new_order_amountsell: Fr,
    pub new_order_tokenbuy: Fr,
    pub new_order_amountbuy: Fr,
    pub accountID: Fr,
    pub balance: Fr,
    pub nonce: Fr,
    pub sign: Fr,
    pub ay: Fr,
    pub ethAddr: Fr,
}

impl PlaceOrderTxData {
    pub fn encode(self) -> Vec<Fr> {
        // double check template config is consistent
        assert!(TX_LENGTH == 34, "invalid length, check your template config");
        let mut results = vec![
            self.order_pos,
            self.old_order_id,
            self.new_order_id,
            self.old_order_tokensell,
            self.old_order_filledsell,
            self.old_order_amountsell,
            self.old_order_tokenbuy,
            self.old_order_filledbuy,
            self.old_order_amountbuy,
            self.new_order_tokensell,
            self.new_order_amountsell,
            self.new_order_tokenbuy,
            self.new_order_amountbuy,
            self.accountID,
            self.balance,
            self.nonce,
            self.sign,
            self.ay,
            self.ethAddr,
        ];
        while results.len() < TX_LENGTH {
            results.push(Fr::zero());
        }
        results
    }
}
