use crate::account::{Account, Signature};
use crate::state::global::GlobalState;
use crate::state::witness_generator::WitnessGenerator;
use crate::test_utils::types::prec_token_id;
use crate::test_utils::{CircuitTestData, L2BlockSerde};
use crate::types::fixnum::decimal_to_amount;
use crate::types::l2::{self, DepositTx, L2Key, Order, SpotTradeTx, TransferTx, WithdrawTx};
use crate::types::primitives::{u32_to_fr, Fr};
use ff::Field;
use rust_decimal::Decimal;
use serde_json::json;

pub struct Block {
    n_txs: usize,
    account_levels: usize,
    balance_levels: usize,
    order_levels: usize,
    verbose: bool,
}

impl Block {
    pub fn new(n_txs: usize, balance_levels: usize, order_levels: usize, account_levels: usize, verbose: bool) -> Self {
        Self {
            n_txs,
            account_levels,
            balance_levels,
            order_levels,
            verbose,
        }
    }

    pub fn test_data(&self) -> Vec<CircuitTestData> {
        let mut cases = self.block_cases();
        let empty_case = self.empty_block_case();
        cases.push(empty_case);
        cases
    }

    fn block_cases(&self) -> Vec<CircuitTestData> {
        /*
        the l2 blocks contains following txs:
        1. deposit token0 to account0
        2. transfer token0 from account0 to account1, and create account1 keypair
        3. transfer token0 from account1 to account0
        4. withdraw token0 from account0
        5. desposit token0 to account1
        6. desposit token1 to account2, and create account2 keypair
        7. spot trade

        keypair of account0 is preset
        keypair of account1 is created by transfer_to_new
        keypair of account2 is created by deposit_to_new
        */
        let state = GlobalState::new(self.balance_levels, self.order_levels, self.account_levels, self.verbose);
        let (sender, receiver) = crossbeam_channel::bounded(100);
        let mut witgen = WitnessGenerator::new(state, self.n_txs, sender, self.verbose);

        let token_id0 = 0;
        let token_id1 = 1;

        let account_id0 = witgen.create_new_account(1).unwrap();
        let account_id1 = witgen.create_new_account(1).unwrap();
        let account_id2 = witgen.create_new_account(1).unwrap();

        let account0 = Account::new(account_id0);
        let account1 = Account::new(account_id1);
        let account2 = Account::new(account_id2);

        // mock existing account0 data
        witgen.set_account_l2_addr(account_id0, account0.sign(), account0.ay(), account0.eth_addr());
        for i in 0..2u32.pow(self.balance_levels as u32) {
            witgen.set_token_balance(account_id0, i, u32_to_fr(20 + i));
        }
        witgen.set_account_nonce(account_id0, u32_to_fr(29));

        // start txs

        // assert(witgen.accounts.get(account_id0).eth_addr() == 0, 'account0 should be empty');
        witgen
            .deposit(DepositTx {
                token_id: token_id0,
                account_id: account_id0,
                amount: decimal_to_amount(&Decimal::new(300, 0), prec_token_id(token_id0)),
                l2key: None,
            })
            .unwrap();

        let mut transfer_tx0 = TransferTx::new(
            account_id0,
            account_id1,
            token_id0,
            decimal_to_amount(&Decimal::new(100, 0), prec_token_id(token_id0)),
        );
        transfer_tx0.l2key = Some(L2Key {
            eth_addr: account1.eth_addr(),
            sign: account1.sign(),
            ay: account1.ay(),
        });
        transfer_tx0.from_nonce = witgen.get_account_nonce(account_id0);
        let hash = transfer_tx0.hash();
        transfer_tx0.sig = account0.sign_hash(hash).unwrap();
        witgen.transfer(transfer_tx0);

        let mut transfer_tx1 = TransferTx::new(
            account_id1,
            account_id0,
            token_id0,
            decimal_to_amount(&Decimal::new(50, 0), prec_token_id(token_id0)),
        );
        transfer_tx1.from_nonce = witgen.get_account_nonce(account_id1);
        let hash = transfer_tx1.hash();
        transfer_tx1.sig = account1.sign_hash(hash).unwrap();
        witgen.transfer(transfer_tx1);

        let mut withdraw_tx = WithdrawTx::new(
            account_id0,
            token_id0,
            decimal_to_amount(&Decimal::new(150, 0), prec_token_id(token_id0)),
        );
        witgen.fill_withdraw_tx(&mut withdraw_tx);
        let hash = withdraw_tx.hash();
        // hash = common.hashWithdraw(fullWithdrawTx);
        withdraw_tx.sig = account0.sign_hash(hash).unwrap();
        witgen.withdraw(withdraw_tx);

        // trade amount
        let amount_1to2 = 120;
        let amount_2to1 = 1200;
        // ensure balance to trade
        witgen
            .deposit(DepositTx {
                account_id: account_id1,
                token_id: token_id0,
                amount: decimal_to_amount(&Decimal::new(199, 0), prec_token_id(token_id0)),
                l2key: None,
            })
            .unwrap();
        witgen
            .deposit(DepositTx {
                account_id: account_id2,
                token_id: token_id1,
                amount: decimal_to_amount(&Decimal::new(1990, 0), prec_token_id(token_id1)),
                l2key: Some(L2Key {
                    eth_addr: account2.eth_addr(),
                    sign: account2.sign(),
                    ay: account2.ay(),
                }),
            })
            .unwrap();

        // order1
        let order_id1 = 1;
        let mut order1 = Order {
            order_id: order_id1,
            tokenbuy: u32_to_fr(token_id1),
            tokensell: u32_to_fr(token_id0),
            total_buy: decimal_to_amount(&Decimal::new(10000, 0), prec_token_id(token_id1)).to_fr(),
            total_sell: decimal_to_amount(&Decimal::new(1000, 0), prec_token_id(token_id0)).to_fr(),
            filled_buy: Fr::zero(),
            filled_sell: Fr::zero(),
            sig: Signature::default(),
            account_id: 1,
            side: l2::order::OrderSide::Buy,
        };
        order1.sign_with(&account1).unwrap();
        // order_id is known to the user, user should sign this order_id
        // while order_idx(or order_pos) is maintained by the global state keeper. User dont need to know anything about order_pos
        // const order1_pos = state.nextOrderIds.get(accountID1);
        // assert(order1_pos === 1n, 'unexpected order pos');
        //witgen.set_account_order(account_id1, order_id1, order1);
        // order2
        let order_id2 = 1;
        let mut order2 = Order {
            order_id: order_id2,
            tokenbuy: u32_to_fr(token_id0),
            tokensell: u32_to_fr(token_id1),
            total_buy: decimal_to_amount(&Decimal::new(1000, 0), prec_token_id(token_id0)).to_fr(),
            total_sell: decimal_to_amount(&Decimal::new(10000, 0), prec_token_id(token_id1)).to_fr(),
            filled_buy: Fr::zero(),
            filled_sell: Fr::zero(),
            sig: Signature::default(),
            account_id: 2,
            side: l2::order::OrderSide::Buy,
        };
        order2.sign_with(&account2).unwrap();
        //witgen.set_account_order(account_id2, order_id2, order2);
        let trade = SpotTradeTx {
            order1_account_id: account_id1,
            order2_account_id: account_id2,
            token_id_1to2: token_id0,
            token_id_2to1: token_id1,
            amount_1to2: decimal_to_amount(&Decimal::new(amount_1to2, 0), prec_token_id(token_id0)),
            amount_2to1: decimal_to_amount(&Decimal::new(amount_2to1, 0), prec_token_id(token_id1)),
            order1_id: order_id1,
            order2_id: order_id2,
        };
        //witgen.spot_trade(trade);

        let full_trade = l2::FullSpotTradeTx {
            trade,
            maker_order: order1,
            taker_order: order2,
        };
        witgen.full_spot_trade(full_trade);

        witgen.flush_with_nop();
        receiver
            .try_iter()
            .enumerate()
            .map(|(i, block)| CircuitTestData {
                name: format!("nonempty_block_{}", i),
                input: json!(L2BlockSerde::from(block)),
                output: json!({}),
            })
            .collect()
    }

    fn empty_block_case(&self) -> CircuitTestData {
        let state = GlobalState::new(self.balance_levels, self.order_levels, self.account_levels, self.verbose);
        let (sender, receiver) = crossbeam_channel::bounded(100);
        let mut witgen = WitnessGenerator::new(state, self.n_txs, sender, self.verbose);
        // we need to have at least 1 account
        witgen.create_new_account(1).unwrap();
        witgen.nop();
        witgen.flush_with_nop();
        let block = receiver.recv().unwrap();
        CircuitTestData {
            name: "empty_block".to_owned(),
            input: json!(L2BlockSerde::from(block)),
            output: json!({}),
        }
    }
}
