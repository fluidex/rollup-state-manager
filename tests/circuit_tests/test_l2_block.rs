use fluidex_common::rust_decimal::Decimal;
use fluidex_common::types::{DecimalExt, FrExt};
use fluidex_common::Fr;
use rollup_state_manager::account::Account;
use rollup_state_manager::state::{GlobalState, ManagerWrapper};
use rollup_state_manager::test_utils::circuit::{CircuitSource, CircuitTestCase, CircuitTestData};
use rollup_state_manager::test_utils::types::prec_token_id;
use rollup_state_manager::types::l2::{self, AmountType, DepositTx, UpdateKeyTx, L2BlockSerde, L2Key, OrderInput, SpotTradeTx, TransferTx, WithdrawTx};
use serde_json::json;

use rollup_state_manager::params;
use std::option::Option::None;
use std::sync::{Arc, RwLock};

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
        let state = Arc::new(RwLock::new(GlobalState::new(
            self.balance_levels,
            self.order_levels,
            self.account_levels,
            self.verbose,
        )));
        let (sender, receiver) = crossbeam_channel::bounded(100);
        let mut manager = ManagerWrapper::new(state, self.n_txs, None, self.verbose);

        let token_id0 = 0;
        let token_id1 = 1;

        let account_id0 = manager.create_new_account(1).unwrap();
        let account_id1 = manager.create_new_account(1).unwrap();
        let account_id2 = manager.create_new_account(1).unwrap();

        let account0 = Account::new(account_id0);
        let account1 = Account::new(account_id1);
        let account2 = Account::new(account_id2);

        // mock existing account0 data
        manager.set_account_l2_addr(account_id0, account0.sign(), account0.ay());
        for i in 0..2u32.pow(self.balance_levels as u32) {
            manager.set_token_balance(account_id0, i, Fr::from_u32(20 + i));
        }
        manager.set_account_nonce(account_id0, Fr::from_u32(29));

        // start txs

        // assert(manager.accounts.get(account_id0).eth_addr() == 0, 'account0 should be empty');
        manager
            .deposit(
                DepositTx {
                    token_id: token_id0,
                    account_id: account_id0,
                    amount: AmountType::from_decimal(&Decimal::new(300, 0), prec_token_id(token_id0)).unwrap(),
                    l2key: None,
                },
                None,
            )
            .unwrap();

        manager
            .key_update(
                UpdateKeyTx {
                    account_id: account_id1,
                    l2key: L2Key {
                        eth_addr: account1.eth_addr(),
                        sign: account1.sign(),
                        ay: account1.ay(),
                    },
                },
                None,
            )
            .unwrap();  
            
        let mut transfer_tx0 = TransferTx::new(
            account_id0,
            account_id1,
            token_id0,
            AmountType::from_decimal(&Decimal::new(100, 0), prec_token_id(token_id0)).unwrap(),
        );
        transfer_tx0.from_nonce = manager.get_account_nonce(account_id0);
        let hash = transfer_tx0.hash();
        transfer_tx0.sig = account0.sign_hash(hash).unwrap();
        manager.transfer(transfer_tx0, None);

        let mut transfer_tx1 = TransferTx::new(
            account_id1,
            account_id0,
            token_id0,
            AmountType::from_decimal(&Decimal::new(50, 0), prec_token_id(token_id0)).unwrap(),
        );
        transfer_tx1.from_nonce = manager.get_account_nonce(account_id1);
        let hash = transfer_tx1.hash();
        transfer_tx1.sig = account1.sign_hash(hash).unwrap();
        manager.transfer(transfer_tx1, None);

        let mut withdraw_tx = WithdrawTx::new(
            account_id0,
            token_id0,
            AmountType::from_decimal(&Decimal::new(150, 0), prec_token_id(token_id0)).unwrap(),
            manager.get_token_balance(account_id0, token_id0),
        );
        manager.fill_withdraw_tx(&mut withdraw_tx);
        let hash = withdraw_tx.hash();
        withdraw_tx.sig = account0.sign_hash(hash).unwrap();
        manager.withdraw(withdraw_tx, None);

        // trade amount
        let amount_1to2 = 120;
        let amount_2to1 = 1200;
        // ensure balance to trade
        manager
            .deposit(
                DepositTx {
                    account_id: account_id1,
                    token_id: token_id0,
                    amount: AmountType::from_decimal(&Decimal::new(199, 0), prec_token_id(token_id0)).unwrap(),
                    l2key: None,
                },
                None,
            )
            .unwrap();
        manager
            .key_update(
                UpdateKeyTx {
                    account_id: account_id2,
                    l2key: L2Key {
                        eth_addr: account2.eth_addr(),
                        sign: account2.sign(),
                        ay: account2.ay(),
                    },
                },
                None,
            )
            .unwrap();            
        manager
            .deposit(
                DepositTx {
                    account_id: account_id2,
                    token_id: token_id1,
                    amount: AmountType::from_decimal(&Decimal::new(1990, 0), prec_token_id(token_id1)).unwrap(),
                    l2key: None,
                },
                None,
            )
            .unwrap();

        // order1
        let order_id1 = 1;
        let mut order1 = OrderInput {
            order_id: order_id1,
            token_buy: Fr::from_u32(token_id1),
            token_sell: Fr::from_u32(token_id0),
            total_buy: Decimal::new(10000, 0).to_fr(prec_token_id(token_id1)),
            total_sell: Decimal::new(1000, 0).to_fr(prec_token_id(token_id0)),
            sig: Default::default(),
            account_id: 1,
            side: l2::order::OrderSide::Buy,
        };
        order1.sign_with(&account1).unwrap();
        // order_id is known to the user, user should sign this order_id
        // while order_idx(or order_pos) is maintained by the global state keeper. User dont need to know anything about order_pos
        // const order1_pos = state.nextOrderIds.get(accountID1);
        // assert(order1_pos === 1n, 'unexpected order pos');
        //manager.set_account_order(account_id1, order_id1, order1);
        // order2
        let order_id2 = 1;
        let mut order2 = OrderInput {
            order_id: order_id2,
            token_buy: Fr::from_u32(token_id0),
            token_sell: Fr::from_u32(token_id1),
            total_buy: Decimal::new(amount_1to2, 0).to_fr(prec_token_id(token_id0)),
            total_sell: Decimal::new(amount_2to1 + 10, 0).to_fr(prec_token_id(token_id1)),
            sig: Default::default(),
            account_id: 2,
            side: l2::order::OrderSide::Buy,
        };
        order2.sign_with(&account2).unwrap();
        //manager.set_account_order(account_id2, order_id2, order2);
        let trade = SpotTradeTx {
            order1_account_id: account_id1,
            order2_account_id: account_id2,
            token_id_1to2: token_id0,
            token_id_2to1: token_id1,
            amount_1to2: Decimal::new(amount_1to2, 0).to_fr(prec_token_id(token_id0)),
            amount_2to1: Decimal::new(amount_2to1, 0).to_fr(prec_token_id(token_id1)),
            order1_id: order_id1,
            order2_id: order_id2,
        };
        //manager.spot_trade(trade);

        let full_trade = l2::FullSpotTradeTx {
            trade,
            maker_order: Some(order1.into()),
            taker_order: Some(order2.into()),
        };
        manager.full_spot_trade(full_trade, None);

        manager.flush_with_nop();

        for block in manager.pop_all_blocks() {
            sender.try_send(block).unwrap();
        }

        receiver
            .try_iter()
            .enumerate()
            .map(|(i, block)| CircuitTestData {
                name: format!("nonempty_block_{}", i),
                input: json!(L2BlockSerde::from(block.detail)),
                output: None,
            })
            .collect()
    }

    fn empty_block_case(&self) -> CircuitTestData {
        let state = Arc::new(RwLock::new(GlobalState::new(
            self.balance_levels,
            self.order_levels,
            self.account_levels,
            self.verbose,
        )));
        let (sender, receiver) = crossbeam_channel::bounded(100);
        let mut manager = ManagerWrapper::new(state, self.n_txs, None, self.verbose);
        // we need to have at least 1 account
        manager.create_new_account(1).unwrap();
        manager.nop();
        manager.flush_with_nop();

        for block in manager.pop_all_blocks() {
            sender.try_send(block).unwrap();
        }

        let block = receiver.recv().unwrap();
        CircuitTestData {
            name: "empty_block".to_owned(),
            input: json!(L2BlockSerde::from(block.detail)),
            output: None,
        }
    }
}

pub fn get_l2_block_test_case() -> CircuitTestCase {
    let main = format!(
        "Block({}, {}, {}, {})",
        *params::NTXS,
        *params::BALANCELEVELS,
        *params::ORDERLEVELS,
        *params::ACCOUNTLEVELS
    );
    let test_data = Block::new(
        *params::NTXS,
        *params::BALANCELEVELS,
        *params::ORDERLEVELS,
        *params::ACCOUNTLEVELS,
        *params::VERBOSE,
    )
    .test_data();
    CircuitTestCase {
        source: CircuitSource {
            src: "src/block.circom".to_owned(),
            main,
        },
        data: test_data,
    }
}
