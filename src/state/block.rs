use crate::account::{rand_seed, Account, Signature};
use crate::state::global::GlobalState;
use crate::state::witness_generator::WitnessGenerator;
use crate::test_utils::types::prec_token_id;
use crate::test_utils::{CircuitTestData, L2BlockSerde};
use crate::types::fixnum::decimal_to_amount;
use crate::types::l2::{DepositToNewTx, DepositToOldTx, Order, TransferTx};
use crate::types::primitives::u32_to_fr;
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
    pub fn new(n_txs: usize, account_levels: usize, balance_levels: usize, order_levels: usize, verbose: bool) -> Self {
        Self {
            n_txs,
            account_levels,
            balance_levels,
            order_levels,
            verbose,
        }
    }

    pub fn test_data(&self) -> Vec<CircuitTestData> {
        vec![self.empty_block_case()]
    }

    fn block_cases(&self) -> Vec<CircuitTestData> {
        let state = GlobalState::new(self.balance_levels, self.order_levels, self.account_levels, self.verbose);
        let mut witgen = WitnessGenerator::new(state, self.n_txs, self.verbose);

        let token_id = 0;
        let token_id_1to2 = 0;
        let token_id_2to1 = 1;

        let account_id0 = witgen.create_new_account(1).unwrap();
        let account_id1 = witgen.create_new_account(1).unwrap();
        let account_id2 = witgen.create_new_account(1).unwrap();

        let account0 = Account::from_seed(account_id0, &rand_seed()).unwrap();
        let account1 = Account::from_seed(account_id1, &rand_seed()).unwrap();
        let account2 = Account::from_seed(account_id2, &rand_seed()).unwrap();

        // mock existing account1 data
        witgen.set_account_l2_addr(account_id1, account1.sign(), account1.ay(), account1.eth_addr());
        for i in 0..2u32.pow(self.balance_levels as u32) {
            witgen.set_token_balance(account_id1, i, u32_to_fr(10 + i));
        }
        witgen.set_account_nonce(account_id1, u32_to_fr(19));

        // mock existing account2 data
        witgen.set_account_l2_addr(account_id2, account2.sign(), account2.ay(), account2.eth_addr());
        for i in 0..2u32.pow(self.balance_levels as u32) {
            witgen.set_token_balance(account_id2, i, u32_to_fr(20 + i));
        }
        witgen.set_account_nonce(account_id2, u32_to_fr(29));

        // order2
        let order_id2 = 1;
        let mut order2 = Order {
            order_id: u32_to_fr(order_id2),
            tokenbuy: u32_to_fr(token_id_1to2),
            tokensell: u32_to_fr(token_id_2to1),
            total_buy: u32_to_fr(1000),
            total_sell: u32_to_fr(10000),
            filled_buy: u32_to_fr(1),
            filled_sell: u32_to_fr(10),
            sig: Signature::default(),
        };
        order2.sign_with(&account2).unwrap();
        witgen.set_account_order(account_id2, order_id2, order2);

        // start txs

        // assert(witgen.accounts.get(account_id0).eth_addr() == 0, 'account0 should be empty');
        witgen.deposit_to_new(DepositToNewTx {
            token_id,
            account_id: account_id0,
            amount: decimal_to_amount(&Decimal::new(200, 0), prec_token_id(token_id)),
            eth_addr: account0.eth_addr(),
            sign: account0.sign(),
            ay: account0.ay(),
        });

        // assert(state.accounts.get(account_id1).eth_addr() != 0n, 'account1 should not be empty');
        witgen.deposit_to_old(DepositToOldTx {
            token_id,
            account_id: account_id1,
            amount: decimal_to_amount(&Decimal::new(100, 0), prec_token_id(token_id)),
        });

        let mut transfer_tx = TransferTx::new(
            account_id1,
            account_id0,
            token_id,
            decimal_to_amount(&Decimal::new(50, 0), prec_token_id(token_id)),
        );
        witgen.fill_transfer_tx(&mut transfer_tx);
        let hash = transfer_tx.hash();
        transfer_tx.sig = account1.sign_hash(hash).unwrap();
        witgen.transfer(transfer_tx);

        /*

        function initBlockTestCase() {


          let withdrawTx = {
            accountID: accountID0,
            amount: 150n,
            tokenID: tokenID,
            signature: null,
          };
          let fullWithdrawTx = state.fillWithdrawTx(withdrawTx);
          hash = common.hashWithdraw(fullWithdrawTx);
          withdrawTx.signature = account0.signHash(hash);
          state.Withdraw(withdrawTx);

          // trade amount
          const amount_1to2 = 120n;
          const amount_2to1 = 1200n;
          // ensure balance to trade
          state.DepositToOld({
            accountID: accountID1,
            tokenID: tokenID_1to2,
            amount: 199n,
          });
          state.DepositToOld({
            accountID: accountID2,
            tokenID: tokenID_2to1,
            amount: 1990n,
          });
          const order1_id = 1n;
          const order1: OrderInput = new OrderInput({
            accountID: accountID1,
            order_id: order1_id,
            tokensell: tokenID_1to2,
            tokenbuy: tokenID_2to1,
            total_sell: 1000n,
            total_buy: 10000n,
          });
          order1.signWith(account1);
          // order_id is known to the user, user should sign this order_id
          // while order_idx(or order_pos) is maintained by the global state keeper. User dont need to know anything about order_pos
          //const order1_pos = state.nextOrderIds.get(accountID1);
          //assert(order1_pos === 1n, 'unexpected order pos');
          state.setAccountOrder(accountID1, OrderState.fromOrderInput(order1));

          let spotTradeTx = {
            order1_accountID: accountID1,
            order2_accountID: accountID2,
            tokenID_1to2: tokenID_1to2,
            tokenID_2to1: tokenID_2to1,
            amount_1to2: amount_1to2,
            amount_2to1: amount_2to1,
            order1_id: order1_id,
            order2_id: order2_id,
          };
          state.SpotTrade(spotTradeTx);

          for (var i = state.bufferedTxs.length; i < nTxs; i++) {
            state.Nop();
          }

          let blocks = state.forgeAllL2Blocks();
          return blocks;
        }

        */

        todo!()
    }

    fn empty_block_case(&self) -> CircuitTestData {
        let state = GlobalState::new(self.balance_levels, self.order_levels, self.account_levels, self.verbose);
        let mut witgen = WitnessGenerator::new(state, self.n_txs, self.verbose);
        // we need to have at least 1 account
        witgen.create_new_account(1).unwrap();
        for _ in 0..self.n_txs {
            witgen.nop();
        }
        let block = witgen.forge_all_l2_blocks()[0].clone();
        CircuitTestData {
            name: "empty_block".to_owned(),
            input: json!(L2BlockSerde::from(block)),
            output: json!({}),
        }
    }
}
