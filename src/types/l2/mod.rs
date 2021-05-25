pub mod mod_tx_data;

pub use mod_tx_data::*;

// from https://github1s.com/Fluidex/circuits/blob/HEAD/test/common.ts
use super::fixnum::Float864;
use crate::account::{Account, Signature};
pub use crate::types::merkle_tree::MerklePath;
use crate::types::primitives::{hash, shl, Fr};
use anyhow::bail;
use anyhow::Result;
use ff::Field;
use std::convert::TryInto;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Order {
    pub order_id: Fr,
    pub tokenbuy: Fr,
    pub tokensell: Fr,
    pub filled_sell: Fr,
    pub filled_buy: Fr,
    pub total_sell: Fr,
    pub total_buy: Fr,
    pub sig: Signature,
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
            sig: Signature::default(),
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
    pub fn sign_with(&mut self, account: &Account) -> Result<(), String> {
        self.sig = account.sign_hash(self.hash())?;
        Ok(())
    }
    pub fn trade_with(&mut self, sell: &Fr, buy: &Fr) {
        // TODO: check overflow?
        self.filled_buy.add_assign(buy);
        self.filled_sell.add_assign(sell);
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

pub type AmountType = Float864;

#[derive(Debug)]
pub struct PlaceOrderTx {
    pub order_id: u32,
    pub account_id: u32,
    pub token_id_sell: u32,
    pub token_id_buy: u32,
    pub amount_sell: Fr,
    pub amount_buy: Fr,
}

#[derive(Debug)]
pub struct DepositToOldTx {
    pub account_id: u32,
    pub token_id: u32,
    pub amount: AmountType,
}

#[derive(Debug)]
pub struct SpotTradeTx {
    pub order1_account_id: u32,
    pub order2_account_id: u32,
    pub token_id_1to2: u32,
    pub token_id_2to1: u32,
    pub amount_1to2: AmountType,
    pub amount_2to1: AmountType,
    pub order1_id: u32,
    pub order2_id: u32,
}

pub const PUBDATA_LEN: usize = 60;
pub const ACCOUNT_ID_LEN: usize = 4;
pub const TOKEN_ID_LEN: usize = 2;
pub const AMOUNT_LEN: usize = 9;
//pub type PUBDATA = [u8; PUBDATA_LEN];

// https://github.com/Fluidex/circuits/issues/144
impl DepositToOldTx {
    pub fn to_pubdata(&self) -> Vec<u8> {
        let mut result = vec![TxType::DepositToOld as u8];
        result.append(&mut self.account_id.to_be_bytes().to_vec());
        result.append(&mut (self.token_id as u16).to_be_bytes().to_vec());
        result.append(&mut self.amount.encode());
        assert!(result.len() <= PUBDATA_LEN);
        result.append(&mut vec![0; PUBDATA_LEN - result.len()]);
        result
    }
    pub fn from_pubdata(data: &[u8]) -> Result<Self> {
        if data.len() != PUBDATA_LEN {
            bail!("invalid len for DepositToOldTx");
        }
        let mut idx: usize = 0;

        if data[0] != TxType::DepositToOld as u8 {
            bail!("invalid type for DepositToOldTx");
        }
        idx += 1;

        let account_id = u32::from_be_bytes(data[idx..(idx + ACCOUNT_ID_LEN)].try_into()?);
        idx += ACCOUNT_ID_LEN;

        let token_id = (u16::from_be_bytes(data[idx..(idx + TOKEN_ID_LEN)].try_into()?)) as u32;
        idx += TOKEN_ID_LEN;

        let amount = AmountType::decode(&data[idx..(idx + AMOUNT_LEN)])?;
        Ok(Self {
            account_id,
            token_id,
            amount,
        })
    }
}

#[cfg(test)]
#[test]
fn test_deposit_to_old_pubdata() {
    let tx = DepositToOldTx {
        account_id: 1323,
        token_id: 232,
        amount: AmountType {
            significand: 756,
            exponent: 11,
        },
    };
    let pubdata1 = tx.to_pubdata();
    println!("pubdata {:?}", pubdata1);
    let tx2 = DepositToOldTx::from_pubdata(&pubdata1).unwrap();
    assert_eq!(tx.account_id, tx2.account_id);
    assert_eq!(tx.token_id, tx2.token_id);
    assert_eq!(tx.amount.to_bigint(), tx2.amount.to_bigint());
}
