#![allow(clippy::let_and_return)]
use crate::types::primitives::{self, hash, shl, u32_to_fr, Fr};

use crate::account::{Account, Signature};

use ff::Field;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrderInput {
    // TODO: or Fr?
    pub account_id: u32,
    pub side: OrderSide,
    pub order_id: u32,
    pub tokenbuy: Fr,
    pub tokensell: Fr,
    pub total_sell: Fr,
    pub total_buy: Fr,
    pub sig: Signature,
}
impl OrderInput {
    pub fn hash(&self) -> Fr {
        // copy from https://github.com/Fluidex/circuits/blob/d6e06e964b9d492f1fa5513bcc2295e7081c540d/helper.ts/state-utils.ts#L38
        // TxType::PlaceOrder
        let magic_head = primitives::u32_to_fr(4);
        let data = hash(&[
            magic_head,
            primitives::u32_to_fr(self.order_id),
            self.tokensell,
            self.tokenbuy,
            self.total_sell,
            self.total_buy,
        ]);
        //data = hash([data, accountID, nonce]);
        // nonce and orderID seems redundant?

        // account_id is not needed if the hash is signed later?
        //data = hash(&[data, primitives::u32_to_fr(self.account_id)]);
        data
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Order {
    // TODO: shoule we split these into a OrderInput instance?
    pub account_id: u32,
    pub order_id: u32,
    pub side: OrderSide,
    pub tokenbuy: Fr,
    pub tokensell: Fr,
    pub total_sell: Fr,
    pub total_buy: Fr,
    pub sig: Signature,
    //
    pub filled_sell: Fr,
    pub filled_buy: Fr,
}

impl Default for Order {
    fn default() -> Self {
        Self {
            order_id: 0,
            tokenbuy: Fr::zero(),
            tokensell: Fr::zero(),
            filled_sell: Fr::zero(),
            filled_buy: Fr::zero(),
            total_sell: Fr::zero(),
            total_buy: Fr::zero(),
            sig: Signature::default(),
            account_id: 0,
            side: OrderSide::Buy,
        }
    }
}

impl Order {
    pub fn from_order_input(order_input: &OrderInput) -> Self {
        Self {
            order_id: order_input.order_id,
            tokenbuy: order_input.tokenbuy,
            tokensell: order_input.tokensell,
            total_sell: order_input.total_sell,
            total_buy: order_input.total_buy,
            sig: order_input.sig,
            account_id: order_input.account_id,
            side: order_input.side,
            filled_sell: Fr::zero(),
            filled_buy: Fr::zero(),
        }
    }
    pub fn hash(&self) -> Fr {
        let mut data = Fr::zero();
        data.add_assign(&u32_to_fr(self.order_id));
        data.add_assign(&shl(&self.tokenbuy, 32));
        data.add_assign(&shl(&self.tokensell, 64));
        hash(&[data, self.filled_sell, self.filled_buy, self.total_sell, self.total_buy])
    }
    pub fn is_filled(&self) -> bool {
        //debug_assert!(self.filled_buy <= self.total_buy, "too much filled buy");
        //debug_assert!(self.filled_sell <= self.total_sell, "too much filled sell");
        // TODO: one side fill is enough
        // https://github.com/Fluidex/circuits/blob/4f952f63aa411529c466de2f6e9f8ceeac9ceb00/src/spot_trade.circom#L42
        //self.filled_buy >= self.total_buy || self.filled_sell >= self.total_sell
        (self.side == OrderSide::Buy && self.filled_buy >= self.total_buy)
            || (self.side == OrderSide::Sell && self.filled_sell >= self.total_sell)
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

#[cfg(test)]
#[test]
fn bench_order_sign() {
    use std::time::Instant;
    let mut order = Order::default();
    let t1 = Instant::now();
    for _ in 0..99 {
        order.hash();
    }
    // order hash takes 7.18ms, debug mode
    println!("order hash takes {}ms", t1.elapsed().as_millis() as f64 / 100.0);
    let acc = Account::new(0);
    let t2 = Instant::now();
    for _ in 0..99 {
        order.sign_with(&acc).unwrap();
    }
    // order sign takes 53.45ms, debug mode
    println!("order sign takes {}ms", t2.elapsed().as_millis() as f64 / 100.0);
}
