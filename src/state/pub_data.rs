use super::global::{AccountUpdates, GlobalState};
use crate::account::Signature;
use crate::types::l2::tx::{AmountType, DepositTx, FullSpotTradeTx, L2Tx, TransferTx, WithdrawTx, AMOUNT_LEN};
use crate::types::l2::Order;
use fluidex_common::num_bigint::BigInt;
use fluidex_common::num_traits::{
    identities::{One, Zero},
    int::PrimInt,
    sign::Unsigned,
    FromPrimitive,
};
use fluidex_common::{ff::Field, types::FrExt, Fr};
use std::io::Read;

struct BitDecodeContext<'c> {
    ref_buf: &'c [u8],
    considering_bit: u8,
}

impl<'c> BitDecodeContext<'c> {
    pub fn new(r: &'c [u8]) -> Self {
        BitDecodeContext {
            ref_buf: r,
            considering_bit: 0u8,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PubDataDecodeError {
    #[error("reference slice has out of range")]
    OutOfRange,
    #[error("integer has overflowed")]
    Overflowed,
    #[error("encoded integer is malformed: {0}")]
    InvalidInteger(u64),
}

impl BitDecodeContext<'_> {
    fn next_byte(&mut self) -> Result<(), PubDataDecodeError> {
        self.ref_buf = self.ref_buf.get(1..).ok_or(PubDataDecodeError::OutOfRange)?;
        self.considering_bit = 0;

        Ok(())
    }

    fn next_bit(&mut self) -> Result<bool, PubDataDecodeError> {
        if self.ref_buf.is_empty() {
            return Err(PubDataDecodeError::OutOfRange);
        }

        let mask: [u8; 8] = [128, 64, 32, 16, 8, 4, 2, 1];

        let ret = mask[self.considering_bit as usize] & self.ref_buf[0] != 0;

        self.considering_bit += 1;
        if self.considering_bit == 8 {
            self.next_byte()?;
        }

        Ok(ret)
    }

    pub fn read_int<T: PrimInt + Unsigned + FromPrimitive + One + Zero>(&mut self, bits: usize) -> Result<T, PubDataDecodeError> {
        let mut start = T::zero();

        for i in 0..bits {
            let nextbit = if self.next_bit()? { T::one() << i } else { T::zero() };
            start = start.checked_add(&nextbit).ok_or(PubDataDecodeError::Overflowed)?;
        }

        Ok(start)
    }
}

impl Read for BitDecodeContext<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut rsz: usize = 0;

        for elm in buf.iter_mut() {
            //consider EOF when there is less than 8bits avaliable
            if (self.ref_buf.len() * 8 - self.considering_bit as usize) < 8 {
                return Ok(rsz);
            }

            *elm = self.read_int(8).unwrap();
            rsz += 1;
        }

        Ok(rsz)
    }
}

struct PubRawTx {
    account1: u32,
    account2: u32,
    token: u32,
    amount: AmountType,
}

impl PubRawTx {
    //we are able to restore typed tx from rawtx's field
    fn to_tx(self) -> L2Tx {
        if self.account1 == 0 && self.account2 == 0 && self.amount.significand.is_zero() {
            return L2Tx::Nop;
        }

        //TODO: not completed yet, we can not tell deposit / withdraw tx apart and spotTrade can not be handled
        if self.account1 == self.account2 {
            //deposit or withdraw
            L2Tx::Deposit(DepositTx {
                account_id: self.account1,
                token_id: self.token,
                amount: self.amount,
                l2key: None,
            })
        } else {
            //transfer
            L2Tx::Transfer(TransferTx {
                from: self.account1,
                to: self.account2,
                token_id: self.token,
                amount: self.amount,
                from_nonce: Fr::zero(),
                sig: Signature::default(),
                l2key: None,
            })
        }
    }
}

//re-exporting
pub use crate::types::l2::tx::TxDataEncoder;

pub struct TxDataDecoder {
    pub n_txs: u32,
    pub account_bits: u32,
    pub token_bits: u32,
}

impl TxDataDecoder {
    pub fn new(n_txs: u32, balance_levels: u32, account_levels: u32) -> Self {
        TxDataDecoder {
            n_txs,
            account_bits: account_levels,
            token_bits: balance_levels,
        }
    }

    fn decode_to_rawtx(&self, decoder: &mut BitDecodeContext<'_>) -> Result<PubRawTx, PubDataDecodeError> {
        let account1 = decoder.read_int(self.account_bits as usize)?;
        let account2 = decoder.read_int(self.account_bits as usize)?;
        let token = decoder.read_int(self.token_bits as usize)?;
        let amount_u: u64 = decoder.read_int(AMOUNT_LEN as usize * 8)?;

        Ok(PubRawTx {
            account1,
            account2,
            token,
            amount: AmountType::from_encoded_bigint(BigInt::from(amount_u)).map_err(|_| PubDataDecodeError::InvalidInteger(amount_u))?,
        })
    }

    pub fn decode_pub_data(&self, data: &[u8]) -> Result<Vec<L2Tx>, PubDataDecodeError> {
        let mut decoder = BitDecodeContext::new(data);
        let mut ret = Vec::new();

        for _ in 0..self.n_txs {
            ret.push(self.decode_to_rawtx(&mut decoder).map(PubRawTx::to_tx)?);
        }

        Ok(ret)
    }
}

#[cfg(test)]
#[test]
fn test_decode_pubdata() {
    let encoded_sample = [
        0x4a, 0xf0, 0xb5, 0xa4, 0x0, 0x0, 0x25, 0x47, 0x74, 0x00, 0x0, 0x0, 0x13, 0xa1, 0xdd, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
        0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
    ];

    let tx_decoder = TxDataDecoder::new(5, 3, 3);

    let txs = tx_decoder.decode_pub_data(&encoded_sample).unwrap();

    let tx = match &txs[0] {
        L2Tx::Deposit(tx) => tx,
        _ => panic!("not here"),
    };

    assert_eq!(tx.account_id, 2);
    assert_eq!(tx.token_id, 5);
    assert_eq!(tx.amount.to_bigint(), BigInt::from(1234567));

    let tx = match &txs[1] {
        L2Tx::Deposit(tx) => tx,
        _ => panic!("not here"),
    };

    assert_eq!(tx.account_id, 2);
    assert_eq!(tx.token_id, 5);
    assert_eq!(tx.amount.to_bigint(), BigInt::from(3000));

    let tx = match &txs[2] {
        L2Tx::Transfer(tx) => tx,
        _ => panic!("not here"),
    };

    assert_eq!(tx.from, 2);
    assert_eq!(tx.to, 6);
    assert_eq!(tx.token_id, 5);
    assert_eq!(tx.amount.to_bigint(), BigInt::from(6000));

    match &txs[3] {
        L2Tx::Nop => (),
        _ => panic!("not here"),
    };

    match &txs[4] {
        L2Tx::Nop => (),
        _ => panic!("not here"),
    };
}

pub struct StateRecoveror {
    state: GlobalState,
    n_tx: usize,
    tx_data_decoder: TxDataDecoder,
    block_recover_num: usize,
    verbose: bool,
}

impl StateRecoveror {
    pub fn new(n_tx: usize, balance_levels: usize, order_levels: usize, account_levels: usize, verbose: bool) -> Self {
        StateRecoveror {
            n_tx,
            state: GlobalState::new(balance_levels, order_levels, account_levels, verbose),
            tx_data_decoder: TxDataDecoder::new(n_tx as u32, balance_levels as u32, account_levels as u32),
            block_recover_num: 0,
            verbose,
        }
    }

    pub fn from_state(init_state: GlobalState, n_tx: usize, block_offset: usize, verbose: bool) -> Self {
        let tx_data_decoder = TxDataDecoder::new(n_tx as u32, init_state.balance_bits() as u32, init_state.account_bits() as u32);
        StateRecoveror {
            n_tx,
            state: init_state,
            tx_data_decoder,
            block_recover_num: block_offset,
            verbose,
        }
    }

    pub fn recover_from_l1tx(&mut self, tx: DepositTx) -> anyhow::Result<()> {
        let maybe_l2key = tx.l2key.clone();
        let account_id = tx.account_id;
        self.deposit(tx)?;
        if let Some(l2key) = maybe_l2key {
            self.state.set_account_l2_addr(account_id, l2key.sign, l2key.ay, l2key.eth_addr);
        }

        let new_root = self.state.root();
        log::debug!("recover L1 deposit new root {}", new_root);

        Ok(())
    }

    pub fn to_next_block(&mut self) {
        self.block_recover_num += 1;
    }

    pub fn recover_from_pubdata(&mut self, data: &[u8]) -> Result<(), PubDataDecodeError> {
        let txs = self.tx_data_decoder.decode_pub_data(data)?;

        for tx in txs.into_iter() {
            match tx {
                L2Tx::Deposit(tx) => self.deposit(tx).unwrap(),
                L2Tx::Transfer(tx) => self.transfer(tx),
                L2Tx::Withdraw(tx) => self.withdraw(tx),
                L2Tx::FullSpotTrade(tx) => self.spot_trade(tx),
                _ => (),
            };
        }

        self.block_recover_num += 1;
        Ok(())
    }

    pub fn cur_state(&self) -> (usize, Fr) {
        (self.block_recover_num, self.state.root())
    }

    fn deposit(&mut self, tx: DepositTx) -> anyhow::Result<()> {
        let state = &mut self.state;

        let old_balance = if !state.has_account(tx.account_id) {
            Fr::zero()
        } else {
            state.get_token_balance(tx.account_id, tx.token_id)
        };

        let mut balance = old_balance;
        balance.add_assign(&tx.amount.to_fr());
        state.set_token_balance(tx.account_id, tx.token_id, balance);

        Ok(())
    }

    fn transfer(&mut self, tx: TransferTx) {
        let state = &mut self.state;

        assert!(state.has_account(tx.from), "invalid account {:?}", tx);

        let from_old_balance = state.get_token_balance(tx.from, tx.token_id);
        let to_old_balance = state.get_token_balance(tx.to, tx.token_id);

        assert!(
            from_old_balance >= tx.amount.to_fr(),
            "Transfer balance not enough {} < {}",
            from_old_balance,
            tx.amount.to_fr()
        );

        let from_new_balance = from_old_balance.sub(&tx.amount.to_fr());
        let to_new_balance = to_old_balance.add(&tx.amount.to_fr());

        let acc1_updates = AccountUpdates {
            account_id: tx.from,
            balance_updates: vec![(tx.token_id, from_new_balance)],
            new_nonce: Some(state.get_account_nonce(tx.from).add(&Fr::one())),
            ..Default::default()
        };
        let acc2_updates = AccountUpdates {
            account_id: tx.to,
            balance_updates: vec![(tx.token_id, to_new_balance)],
            ..Default::default()
        };
        state.batch_update(vec![acc1_updates, acc2_updates], false);
    }

    fn withdraw(&mut self, tx: WithdrawTx) {
        let state = &mut self.state;

        assert!(state.has_account(tx.account_id), "invalid account {:?}", tx);
        let amount = tx.amount.to_fr();

        let old_balance = state.get_token_balance(tx.account_id, tx.token_id);
        let new_balance = old_balance.sub(&amount);

        state.set_token_balance(tx.account_id, tx.token_id, new_balance);
        state.increase_nonce(tx.account_id);
    }

    fn handle_trade_order(&mut self, account_id: u32, order_id: u32, maybe_neworder: Option<Order>) -> (u32, Order) {
        if let Some(order) = maybe_neworder {
            // new order
            assert!(!self.state.has_order(order.account_id, order.order_id));
            assert_eq!(order.filled_buy, Fr::zero());
            assert_eq!(order.filled_sell, Fr::zero());
            // state.update_order_state(maker_order.account_id, maker_order);
            (self.state.find_or_insert_order(account_id, &order).0, order)
        } else {
            // order1 means maker, order2 means taker
            assert!(self.state.has_order(account_id, order_id), "unknown order {}", order_id);
            let order_pos = self.state.get_order_pos_by_id(account_id, order_id).unwrap();
            (order_pos, self.state.get_account_order_by_id(account_id, order_id))
        }
    }

    fn spot_trade(&mut self, full_tx: FullSpotTradeTx) {
        let trade = full_tx.trade;
        let acc_id1 = trade.order1_account_id;
        let acc_id2 = trade.order2_account_id;

        assert!(acc_id1 != acc_id2, "self trade no allowed");

        // handle new order
        let (order1_pos, order1) = self.handle_trade_order(acc_id1, trade.order1_id, full_tx.maker_order);
        let (order2_pos, order2) = self.handle_trade_order(acc_id2, trade.order2_id, full_tx.taker_order);

        let state = &mut self.state;
        assert!(state.has_account(acc_id1), "invalid account {:?}", trade);
        assert!(state.has_account(acc_id2), "invalid account {:?}", trade);

        let acc1_balance_sell = state.get_token_balance(acc_id1, trade.token_id_1to2);
        assert!(acc1_balance_sell > trade.amount_1to2.to_fr(), "balance_1to2");
        let acc1_balance_sell_new = acc1_balance_sell.sub(&trade.amount_1to2.to_fr());
        let acc1_balance_buy = state.get_token_balance(acc_id1, trade.token_id_2to1);
        let acc1_balance_buy_new = acc1_balance_buy.add(&trade.amount_2to1.to_fr());

        let acc2_balance_sell = state.get_token_balance(acc_id2, trade.token_id_2to1);
        assert!(acc2_balance_sell > trade.amount_2to1.to_fr(), "balance_2to1");
        let acc2_balance_sell_new = acc2_balance_sell.sub(&trade.amount_2to1.to_fr());
        let acc2_balance_buy = state.get_token_balance(acc_id2, trade.token_id_1to2);
        let acc2_balance_buy_new = acc2_balance_buy.add(&trade.amount_1to2.to_fr());

        let acc1_updates = AccountUpdates {
            account_id: acc_id1,
            balance_updates: vec![
                (trade.token_id_1to2, acc1_balance_sell_new),
                (trade.token_id_2to1, acc1_balance_buy_new),
            ],
            order_updates: vec![(order1_pos, order1.hash())],
            ..Default::default()
        };
        let acc2_updates = AccountUpdates {
            account_id: acc_id2,
            balance_updates: vec![
                (trade.token_id_1to2, acc2_balance_buy_new),
                (trade.token_id_2to1, acc2_balance_sell_new),
            ],
            order_updates: vec![(order2_pos, order2.hash())],
            ..Default::default()
        };
        state.batch_update(vec![acc1_updates, acc2_updates], false);
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::config::Settings;
    use crate::state::ManagerWrapper;
    use fluidex_common::rust_decimal::Decimal;
    use std::sync::{Arc, RwLock};

    use crate::types::l2::L2Key;

    fn dummy_l2key() -> L2Key {
        L2Key {
            eth_addr: Fr::zero(),
            sign: Fr::zero(),
            ay: Fr::one(),
        }
    }

    #[test]
    fn test_state_recover() {
        let mut s = Settings::new();
        //don't persist
        s.persist_every_n_block = 1000;
        Settings::set_safe(s);

        let mut recover = StateRecoveror::new(2, 2, 3, 2, false);
        let mut wrapper = ManagerWrapper::new(Arc::new(RwLock::new(GlobalState::new(2, 3, 2, false))), 2, None, false);

        wrapper
            .deposit(
                DepositTx {
                    account_id: 0,
                    token_id: 1,
                    amount: AmountType::from_decimal(&Decimal::new(1000000i64, 0), 6).unwrap(),
                    l2key: Some(dummy_l2key()),
                },
                None,
            )
            .unwrap();
        wrapper
            .deposit(
                DepositTx {
                    account_id: 0,
                    token_id: 0,
                    amount: AmountType::from_decimal(&Decimal::new(1000000i64, 0), 6).unwrap(),
                    l2key: None,
                },
                None,
            )
            .unwrap();

        //block 2
        wrapper
            .deposit(
                DepositTx {
                    account_id: 1,
                    token_id: 1,
                    amount: AmountType::from_decimal(&Decimal::new(1000000i64, 0), 6).unwrap(),
                    l2key: Some(dummy_l2key()),
                },
                None,
            )
            .unwrap();
        wrapper
            .deposit(
                DepositTx {
                    account_id: 1,
                    token_id: 0,
                    amount: AmountType::from_decimal(&Decimal::new(1000000i64, 0), 6).unwrap(),
                    l2key: None,
                },
                None,
            )
            .unwrap();

        //block 3
        wrapper.transfer(
            TransferTx::new(1, 0, 1, AmountType::from_decimal(&Decimal::new(12345i64, 4), 6).unwrap()),
            None,
        );

        wrapper.flush_with_nop();

        //block 4
        wrapper.transfer(
            TransferTx::new(0, 1, 0, AmountType::from_decimal(&Decimal::new(21345i64, 4), 6).unwrap()),
            None,
        );

        wrapper.transfer(
            TransferTx::new(1, 0, 0, AmountType::from_decimal(&Decimal::new(31245i64, 4), 6).unwrap()),
            None,
        );

        let blks = wrapper.pop_all_blocks();
        assert_eq!(blks.len(), 4);

        //block 0 and 1 can not be recovered from l2 pubdata ...
        recover
            .recover_from_l1tx(DepositTx {
                account_id: 0,
                token_id: 1,
                amount: AmountType::from_decimal(&Decimal::new(1000000i64, 0), 6).unwrap(),
                l2key: Some(dummy_l2key()),
            })
            .unwrap();

        recover
            .recover_from_l1tx(DepositTx {
                account_id: 0,
                token_id: 0,
                amount: AmountType::from_decimal(&Decimal::new(1000000i64, 0), 6).unwrap(),
                l2key: None,
            })
            .unwrap();

        recover.to_next_block();

        let (blkn, root) = recover.cur_state();
        assert_eq!(blkn, 1);
        assert_eq!(root, blks[0].detail.new_root);

        recover
            .recover_from_l1tx(DepositTx {
                account_id: 1,
                token_id: 1,
                amount: AmountType::from_decimal(&Decimal::new(1000000i64, 0), 6).unwrap(),
                l2key: Some(dummy_l2key()),
            })
            .unwrap();

        recover
            .recover_from_l1tx(DepositTx {
                account_id: 1,
                token_id: 0,
                amount: AmountType::from_decimal(&Decimal::new(1000000i64, 0), 6).unwrap(),
                l2key: None,
            })
            .unwrap();

        recover.to_next_block();

        let (blkn, root) = recover.cur_state();
        assert_eq!(blkn, 2);
        assert_eq!(root, blks[1].detail.new_root);

        //only test transfer ...
        recover.recover_from_pubdata(&blks[2].detail.txdata).unwrap();

        let (blkn, root) = recover.cur_state();
        assert_eq!(blkn, 3);
        assert_eq!(root, blks[2].detail.new_root);

        recover.recover_from_pubdata(&blks[3].detail.txdata).unwrap();

        let (blkn, root) = recover.cur_state();
        assert_eq!(blkn, 4);
        assert_eq!(root, blks[3].detail.new_root);
    }
}
