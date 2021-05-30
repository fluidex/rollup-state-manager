use crate::types::primitives::{hash, shl, Fr};

use crate::account::{Account, Signature};

use ff::Field;

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
