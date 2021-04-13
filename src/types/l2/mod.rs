// from https://github1s.com/Fluidex/circuits/blob/HEAD/test/common.ts
pub use crate::types::merkle_tree::MerklePath;
use crate::types::primitives::{hash, shl, Fr};
use ff::Field;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Order {
    pub order_id: Fr,
    pub tokenbuy: Fr,
    pub tokensell: Fr,
    pub filled_sell: Fr,
    pub filled_buy: Fr,
    pub total_sell: Fr,
    pub total_buy: Fr,
}

impl Default for Order {
    fn default() -> Self {
        Self {
            order_id: Fr::zero(),
            tokenbuy: Fr::zero(),
            tokensell: Fr::zero(),
            filled_sell: Fr::zero(),
            filled_buy: Fr::zero(),
            total_sell: Fr::zero(),
            total_buy: Fr::zero(),
        }
    }
}

impl Order {
    pub fn hash(&self) -> Fr {
        let mut data = Fr::zero();
        data.add_assign(&self.order_id);
        data.add_assign(&shl(&self.tokenbuy, 32));
        data.add_assign(&shl(&self.tokensell, 64));
        hash(&[data, self.filled_sell, self.filled_buy, self.total_sell, self.total_buy])
    }
    pub fn is_filled(&self) -> bool {
        //debug_assert!(self.filled_buy <= self.total_buy, "too much filled buy");
        //debug_assert!(self.filled_sell <= self.total_sell, "too much filled sell");
        // TODO: one side fill is enough
        // https://github.com/Fluidex/circuits/blob/4f952f63aa411529c466de2f6e9f8ceeac9ceb00/src/spot_trade.circom#L42
        self.filled_buy >= self.total_buy || self.filled_sell >= self.total_sell
    }
}

#[derive(Copy, Clone)]
pub enum TxType {
    DepositToNew,
    DepositToOld,
    Transfer,
    Withdraw,
    PlaceOrder,
    SpotTrade,
    Nop,
}

pub const TX_LENGTH: usize = 34;
pub mod tx_detail_idx {
    pub const TOKEN_ID: usize = 0;
    pub const AMOUNT: usize = 1;
    pub const ACCOUNT_ID1: usize = 2;
    pub const ACCOUNT_ID2: usize = 3;
    pub const ETH_ADDR1: usize = 4;
    pub const ETH_ADDR2: usize = 5;
    pub const SIGN1: usize = 6;
    pub const SIGN2: usize = 7;
    pub const AY1: usize = 8;
    pub const AY2: usize = 9;
    pub const NONCE1: usize = 10;
    pub const NONCE2: usize = 11;
    pub const BALANCE1: usize = 12;
    pub const BALANCE2: usize = 13;
    pub const BALANCE3: usize = 14;
    pub const BALANCE4: usize = 15;
    pub const SIG_L2_HASH: usize = 16;
    pub const S: usize = 17;
    pub const R8X: usize = 18;
    pub const R8Y: usize = 19;

    // only used in spot_trade
    pub const TOKEN_ID2: usize = 20;
    pub const AMOUNT2: usize = 21;
    pub const ORDER1_ID: usize = 22;
    pub const ORDER1_AMOUNT_SELL: usize = 23;
    pub const ORDER1_AMOUNT_BUY: usize = 24;
    pub const ORDER1_FILLED_SELL: usize = 25;
    pub const ORDER1_FILLED_BUY: usize = 26;
    pub const ORDER2_ID: usize = 27;
    pub const ORDER2_AMOUNT_SELL: usize = 28;
    pub const ORDER2_AMOUNT_BUY: usize = 29;
    pub const ORDER2_FILLED_SELL: usize = 30;
    pub const ORDER2_FILLED_BUY: usize = 31;

    // only used in place_order
    pub const TOKEN_ID3: usize = 32;
    pub const TOKEN_ID4: usize = 33;
}

pub struct RawTx {
    pub tx_type: TxType,
    pub payload: Vec<Fr>,
    pub balance_path0: MerklePath,
    pub balance_path1: MerklePath,
    pub balance_path2: MerklePath,
    pub balance_path3: MerklePath,
    pub order_path0: MerklePath,
    pub order_path1: MerklePath,
    pub order_root0: Fr,
    pub order_root1: Fr,
    pub account_path0: MerklePath,
    pub account_path1: MerklePath,
    pub root_before: Fr,
    pub root_after: Fr,
    // debug info
    // extra: any;
}
#[derive(Clone)]
pub struct L2Block {
    pub txs_type: Vec<TxType>,
    pub encoded_txs: Vec<Vec<Fr>>,
    pub balance_path_elements: Vec<[MerklePath; 4]>,
    pub order_path_elements: Vec<[MerklePath; 2]>,
    pub account_path_elements: Vec<[MerklePath; 2]>,
    pub order_roots: Vec<[Fr; 2]>,
    pub old_account_roots: Vec<Fr>,
    pub new_account_roots: Vec<Fr>,
}

// TODO: remove previous_...
#[derive(Debug)]
pub struct PlaceOrderTx {
    pub order_id: u32,
    pub account_id: u32,
    //pub previous_token_id_sell: u32,
    //pub previous_token_id_buy: u32,
    //pub previous_amount_sell: Fr,
    //pub previous_amount_buy: Fr,
    //pub previous_filled_sell: Fr,
    //pub previous_filled_buy: Fr,
    pub token_id_sell: u32,
    pub token_id_buy: u32,
    pub amount_sell: Fr,
    pub amount_buy: Fr,
}

#[derive(Debug)]
pub struct DepositToOldTx {
    pub account_id: u32,
    pub token_id: u32,
    pub amount: Fr,
}

#[derive(Debug)]
pub struct SpotTradeTx {
    pub order1_account_id: u32,
    pub order2_account_id: u32,
    pub token_id_1to2: u32,
    pub token_id_2to1: u32,
    pub amount_1to2: Fr,
    pub amount_2to1: Fr,
    pub order1_id: u32,
    pub order2_id: u32,
}
