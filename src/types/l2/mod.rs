pub mod codec;
pub mod mod_tx_data;

pub use mod_tx_data::*;

// from https://github1s.com/Fluidex/circuits/blob/HEAD/test/common.ts
pub use crate::types::merkle_tree::MerklePath;
use crate::types::primitives::{hash, shl, Fr};
use rust_decimal::Decimal;
use std::convert::TryInto;
//use num_traits::FromPrimitive;
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

// https://github.com/Fluidex/circuits/issues/144

pub struct Float832 {
    pub exponent: u8,
    pub significand: u32,
}

impl Float832 {
    pub fn encode(&self) -> Vec<u8> {
        let mut result = self.exponent.to_be_bytes().to_vec();
        result.append(&mut self.significand.to_be_bytes().to_vec());
        result
    }
    pub fn decode(data: &Vec<u8>) -> Self {
        let exponent = u8::from_be_bytes(data[0..1].try_into().unwrap());
        let significand = u32::from_be_bytes(data[1..5].try_into().unwrap());
        Self { exponent, significand }
    }
    pub fn to_decimal(&self, prec: u32) -> Decimal {
        // for example, (significand:1, exponent:17) means 10**17, when prec is 18,
        // it is 0.1 (ETH)
        Decimal::new(self.significand as i64, 0) * Decimal::new(10, 0).powi(self.exponent as u64) / Decimal::new(10, 0).powi(prec as u64)
    }
    pub fn from_decimal(d: &Decimal, prec: u32) -> Self {
        // if d is "0.1" and prec is 18, result is (significand:1, exponent:17)
        let ten = Decimal::new(10, 0);
        let exp = ten.powi(prec as u64);
        println!("mul {} {}", d, exp);
        let mut n = d * exp;
        assert!(n == n.floor(), "decimal precision error");
        let mut exponent = 0;
        loop {
            let next = n / ten;
            if next == next.floor() {
                exponent += 1;
                n = next;
            } else {
                break;
            }
        }
        if n > Decimal::new(std::u32::MAX as i64, 0) {
            panic!("invalid precision {} {}", d, prec);
        }
        // TODO: a better way...
        println!("n is {}", n.to_string());
        let significand: u32 = n.floor().to_string().parse::<u32>().unwrap();
        Float832 { exponent, significand }
    }
}

#[cfg(test)]
#[test]
fn test_float832_from_decimal() {
    use std::str::FromStr;
    // 1.23456 * 10**18
    let d0 = Decimal::new(123456, 5);
    let f = Float832::from_decimal(&d0, 18);
    assert_eq!(f.exponent, 13);
    assert_eq!(f.significand, 123456);
    let d = f.to_decimal(18);
    assert_eq!(d, Decimal::from_str("1.23456").unwrap());
    let f2 = Float832::decode(&f.encode());
    assert_eq!(f2.exponent, 13);
    assert_eq!(f2.significand, 123456);
}

pub type BalanceType = u32;
pub type AmountType = u32;
/*
impl DepositToOldTx {
    pub fn to_pubdata(&self) -> Vec<u8> {

    }
}
*/
