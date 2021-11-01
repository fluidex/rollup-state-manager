#![allow(clippy::let_and_return)]
use crate::account::Account;
use fluidex_common::ff::Field;
use fluidex_common::l2::account::{Signature, SignatureBJJ};
#[cfg(not(feature = "fr_string_repr"))]
use fluidex_common::serde::FrBytes as FrSerde;
#[cfg(feature = "fr_string_repr")]
use fluidex_common::serde::FrStr as FrSerde;
use fluidex_common::{types::FrExt, Fr};
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
pub struct OrderInput {
    // TODO: or Fr?
    pub account_id: u32,
    pub side: OrderSide,
    pub order_id: u32,
    pub token_buy: Fr,
    pub token_sell: Fr,
    pub total_sell: Fr,
    pub total_buy: Fr,
    pub sig: Option<SignatureBJJ>,
}
impl OrderInput {
    pub fn hash(&self) -> Fr {
        // copy from https://github.com/fluidex/circuits/blob/d6e06e964b9d492f1fa5513bcc2295e7081c540d/helper.ts/state-utils.ts#L38
        // TxType::PlaceOrder
        let magic_head = Fr::from_u32(4);
        let data = Fr::hash(&[
            magic_head,
            // TODO: sign nonce or order_id
            //Fr::from_u32(self.order_id),
            self.token_sell,
            self.token_buy,
            self.total_sell,
            self.total_buy,
        ]);
        //data = hash([data, accountID, nonce]);
        // nonce and orderID seems redundant?

        // account_id is not needed if the hash is signed later?
        //data = hash(&[data, Fr::from_u32(self.account_id)]);
        data
    }
    pub fn sign_with(&mut self, account: &Account) -> Result<(), String> {
        let hash = self.hash();
        self.sig = Some(account.sign_hash_raw(hash)?);
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Order {
    // TODO: shoule we split these into a OrderInput instance?
    pub account_id: u32,
    pub order_id: u32,
    pub side: OrderSide,
    #[serde(with = "FrSerde")]
    pub token_buy: Fr,
    #[serde(with = "FrSerde")]
    pub token_sell: Fr,
    #[serde(with = "FrSerde")]
    pub total_sell: Fr,
    #[serde(with = "FrSerde")]
    pub total_buy: Fr,
    pub sig: Signature,
    #[serde(with = "FrSerde")]
    pub filled_sell: Fr,
    #[serde(with = "FrSerde")]
    pub filled_buy: Fr,
    pub is_active: bool,
}

impl Default for Order {
    fn default() -> Self {
        Self {
            order_id: 0,
            token_buy: Fr::zero(),
            token_sell: Fr::zero(),
            filled_sell: Fr::zero(),
            filled_buy: Fr::zero(),
            total_sell: Fr::zero(),
            total_buy: Fr::zero(),
            sig: Signature::default(),
            account_id: 0,
            side: OrderSide::Buy,
            is_active: true,
        }
    }
}

impl From<OrderInput> for Order {
    fn from(order_input: OrderInput) -> Order {
        Self {
            order_id: order_input.order_id,
            token_buy: order_input.token_buy,
            token_sell: order_input.token_sell,
            total_sell: order_input.total_sell,
            total_buy: order_input.total_buy,
            sig: Signature::from_raw(order_input.hash(), &order_input.sig.clone().unwrap()),
            account_id: order_input.account_id,
            side: order_input.side,
            filled_sell: Fr::zero(),
            filled_buy: Fr::zero(),
            is_active: true,
        }
    }
}

impl Order {
    pub fn hash(&self) -> Fr {
        let mut data = Fr::zero();
        data.add_assign(&Fr::from_u32(self.order_id));
        data.add_assign(&self.token_buy.shl(32));
        data.add_assign(&self.token_sell.shl(64));
        Fr::hash(&[data, self.filled_sell, self.filled_buy, self.total_sell, self.total_buy])
    }
    pub fn is_filled(&self) -> bool {
        //debug_assert!(self.filled_buy <= self.total_buy, "too much filled buy");
        //debug_assert!(self.filled_sell <= self.total_sell, "too much filled sell");
        // TODO: one side fill is enough
        // https://github.com/fluidex/circuits/blob/4f952f63aa411529c466de2f6e9f8ceeac9ceb00/src/spot_trade.circom#L42
        //self.filled_buy >= self.total_buy || self.filled_sell >= self.total_sell
        (self.side == OrderSide::Buy && self.filled_buy >= self.total_buy)
            || (self.side == OrderSide::Sell && self.filled_sell >= self.total_sell)
    }
    pub fn is_default(&self) -> bool {
        self.total_sell.is_zero()
    }
    pub fn trade_with(&mut self, sell: &Fr, buy: &Fr) {
        // TODO: check overflow?
        self.filled_buy.add_assign(buy);
        self.filled_sell.add_assign(sell);
    }
}

#[cfg(test)]
#[test]
fn bench_order_sign() {
    use std::time::Instant;
    let mut order = Order::default();
    let t1 = Instant::now();
    for _ in 0..99 {
        order.hash();
    }
    // safe version:
    //   order hash takes 7.18ms, debug mode
    //   order hash takes 0.43ms, release mode
    // unsafe version:
    //   order hash takes 7.18ms, debug mode
    //   order hash takes 0.43ms, release mode
    println!("order hash takes {}ms", t1.elapsed().as_millis() as f64 / 100.0);
    let acc = Account::new(0);
    let t2 = Instant::now();
    let hash = order.hash();
    for _ in 0..99 {
        //order.sign_with(&acc).unwrap();
        order.sig = acc.sign_hash(hash).unwrap();
    }
    // safe version:
    //   order sign takes 53.45ms, debug mode
    //   order sign takes 2.42ms, release mode
    // unsafe version:
    //   order sign takes 12.59ms, debug mode
    //   order sign takes 0.4ms, release mode
    println!("order sign takes {}ms", t2.elapsed().as_millis() as f64 / 100.0);
    let t3 = Instant::now();
    for _ in 0..99 {
        assert_eq!(true, acc.l2_account.verify(order.sig));
    }
    // safe version:
    //   order sig verify takes 2.17ms, release mode
    // unsafe version:
    //   order sig verify takes 12.59ms, debug mode
    //   order sig verify takes 0.36ms, release mode
    println!("order sig verify takes {}ms", t3.elapsed().as_millis() as f64 / 100.0);
}
