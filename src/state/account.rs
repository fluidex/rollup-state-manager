// from https://github1s.com/Fluidex/circuits/blob/HEAD/test/common.ts
pub use crate::types::merkle_tree::MerklePath;
use crate::types::primitives::{hash, shl, Fr};
use crate::types::primitives::fr_bytes;
use ff::Field;
use serde::{Serialize, Deserialize};

#[derive(Copy, Clone, Default, Serialize, Deserialize)]
pub struct AccountState {
    #[serde(with = "fr_bytes")]
    pub nonce: Fr,
    #[serde(with = "fr_bytes")]
    pub sign: Fr,
    #[serde(with = "fr_bytes")]
    pub balance_root: Fr,
    #[serde(with = "fr_bytes")]
    pub ay: Fr,
    #[serde(with = "fr_bytes")]
    pub eth_addr: Fr,
    #[serde(with = "fr_bytes")]
    pub order_root: Fr,
}

impl AccountState {
    pub fn empty(balance_root: Fr, order_root: Fr) -> Self {
        Self {
            nonce: Fr::zero(),
            sign: Fr::zero(),
            balance_root,
            ay: Fr::zero(),
            eth_addr: Fr::zero(),
            order_root,
        }
    }
    // TODO: combine with emptyAccount
    /*
    pub fn new() -> Self {
        Self {
            nonce: Fr::zero(),
            sign: Fr::zero(),
            balance_root: Fr::zero(),
            ay: Fr::zero(),
            eth_addr: Fr::zero(),
            order_root: Fr::zero(),
        }
    }
    */
    pub fn hash(&self) -> Fr {
        let mut data = Fr::zero();

        data.add_assign(&self.nonce);
        data.add_assign(&shl(&self.sign, 40));
        let inputs = &[data, self.balance_root, self.ay, self.eth_addr, self.order_root];
        hash(inputs)
    }
    // TODO: remove eth_addr
    pub fn update_l2_addr(&mut self, sign: Fr, ay: Fr, eth_addr: Fr) {
        self.sign = sign;
        self.ay = ay;
        self.eth_addr = eth_addr;
    }
    pub fn update_nonce(&mut self, nonce: Fr) {
        self.nonce = nonce;
    }
    pub fn update_order_root(&mut self, order_root: Fr) {
        self.order_root = order_root;
    }
}
/*
impl Default for AccountState {
    fn default() -> Self {
        Self::new()
    }
}
*/
