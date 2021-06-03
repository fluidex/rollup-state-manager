#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::vec_init_then_push)]

use super::global::{AccountUpdates, GlobalState};
use crate::types::l2::{
    tx_detail_idx, DepositToNewTx, DepositToOldTx, FullSpotTradeTx, L2Block, Order, RawTx, SpotTradeTx, TransferTx, TxType, WithdrawTx,
    TX_LENGTH,
};
use crate::types::merkle_tree::Tree;
use crate::types::primitives::{fr_add, fr_sub, u32_to_fr, Fr};
use ff::Field;

// TODO: too many unwrap here
pub struct WitnessGenerator {
    state: GlobalState,
    n_tx: usize,
    buffered_txs: Vec<RawTx>,
    buffered_blocks: Vec<L2Block>,
    verbose: bool,
}

impl WitnessGenerator {
    pub fn print_config() {
        Tree::print_config();
    }
    pub fn new(state: GlobalState, n_tx: usize, verbose: bool) -> Self {
        Self {
            state,
            n_tx,
            buffered_txs: Vec::new(),
            buffered_blocks: Vec::new(),
            verbose,
        }
    }

    /////////////////// forward method call to self.state //////////////////////////////////
    pub fn root(&self) -> Fr {
        self.state.root()
    }
    pub fn has_order(&self, account_id: u32, order_id: u32) -> bool {
        self.state.has_order(account_id, order_id)
    }
    pub fn has_account(&self, account_id: u32) -> bool {
        self.state.has_account(account_id)
    }
    pub fn get_token_balance(&self, account_id: u32, token_id: u32) -> Fr {
        self.state.get_token_balance(account_id, token_id)
    }
    pub fn update_order_state(&mut self, account_id: u32, order: Order) {
        self.state.update_order_state(account_id, order)
    }
    pub fn create_new_account(&mut self, next_order_id: u32) -> anyhow::Result<u32> {
        self.state.create_new_account(next_order_id)
    }
    pub fn get_account_order_by_id(&self, account_id: u32, order_id: u32) -> Order {
        self.state.get_account_order_by_id(account_id, order_id)
    }
    pub fn set_account_l2_addr(&mut self, account_id: u32, sign: Fr, ay: Fr, eth_addr: Fr) {
        self.state.set_account_l2_addr(account_id, sign, ay, eth_addr);
    }
    pub fn set_account_nonce(&mut self, account_id: u32, nonce: Fr) {
        self.state.set_account_nonce(account_id, nonce);
    }
    pub fn set_account_order(&mut self, account_id: u32, order_pos: u32, order: Order) {
        self.state.set_account_order(account_id, order_pos, order);
    }
    pub fn set_token_balance(&mut self, account_id: u32, token_id: u32, balance: Fr) {
        self.state.set_token_balance(account_id, token_id, balance);
    }

    pub fn forge_with_txs(&self, buffered_txs: &[RawTx]) -> L2Block {
        assert!(buffered_txs.len() == self.n_tx, "invalid txs len");
        let txs_type = buffered_txs.iter().map(|tx| tx.tx_type).collect();
        let encoded_txs = buffered_txs.iter().map(|tx| tx.payload.clone()).collect();
        let balance_path_elements = buffered_txs
            .iter()
            .map(|tx| {
                [
                    tx.balance_path0.clone(),
                    tx.balance_path1.clone(),
                    tx.balance_path2.clone(),
                    tx.balance_path3.clone(),
                ]
            })
            .collect();
        let order_path_elements = buffered_txs
            .iter()
            .map(|tx| [tx.order_path0.clone(), tx.order_path1.clone()])
            .collect();
        let order_roots = buffered_txs.iter().map(|tx| [tx.order_root0, tx.order_root1]).collect();
        let account_path_elements = buffered_txs
            .iter()
            .map(|tx| [tx.account_path0.clone(), tx.account_path1.clone()])
            .collect();
        let old_account_roots = buffered_txs.iter().map(|tx| tx.root_before).collect();
        let new_account_roots = buffered_txs.iter().map(|tx| tx.root_after).collect();
        L2Block {
            txs_type,
            encoded_txs,
            balance_path_elements,
            order_path_elements,
            account_path_elements,
            order_roots,
            old_account_roots,
            new_account_roots,
        }
    }
    pub fn forge(&mut self) -> L2Block {
        self.flush_with_nop();
        self.forge_with_txs(&self.buffered_txs)
    }
    pub fn forge_all_l2_blocks(&mut self) -> Vec<L2Block> {
        self.buffered_blocks.clone()
    }
    pub fn add_raw_tx(&mut self, raw_tx: RawTx) {
        self.buffered_txs.push(raw_tx);
        if self.buffered_txs.len() % self.n_tx == 0 {
            // forge next block, using last n_tx txs
            let txs = &self.buffered_txs[(self.buffered_txs.len() - self.n_tx)..];
            let block = self.forge_with_txs(txs);
            self.buffered_blocks.push(block);
            assert!(
                self.buffered_blocks.len() * self.n_tx == self.buffered_txs.len(),
                "invalid block num"
            );
            if self.verbose {
                println!("forge block {} done", self.buffered_blocks.len() - 1);
            }
        }
    }
    pub fn get_buffered_blocks(&self) -> &[L2Block] {
        self.buffered_blocks.as_slice()
    }
    pub fn take_blocks(self) -> Vec<L2Block> {
        self.buffered_blocks
    }
    pub fn deposit_to_new(&mut self, tx: DepositToNewTx) {
        // assert!(self.accounts.get(tx.account_id).eth_addr == 0n, "deposit_to_new");
        if !self.has_account(tx.account_id) {
            // TODO: return err
            self.state.init_account(tx.account_id, 1).unwrap();
        }
        let proof = self.state.balance_full_proof(tx.account_id, tx.token_id);
        let acc = self.state.get_account(tx.account_id);

        // first, generate the tx
        let mut encoded_tx = [Fr::zero(); TX_LENGTH];
        encoded_tx[tx_detail_idx::AMOUNT] = tx.amount.to_fr();

        encoded_tx[tx_detail_idx::TOKEN_ID1] = u32_to_fr(tx.token_id);
        encoded_tx[tx_detail_idx::ACCOUNT_ID1] = u32_to_fr(tx.account_id);
        encoded_tx[tx_detail_idx::BALANCE1] = Fr::zero();
        encoded_tx[tx_detail_idx::NONCE1] = Fr::zero();
        encoded_tx[tx_detail_idx::ETH_ADDR1] = Fr::zero();
        encoded_tx[tx_detail_idx::SIGN1] = Fr::zero();
        encoded_tx[tx_detail_idx::AY1] = Fr::zero();

        encoded_tx[tx_detail_idx::TOKEN_ID2] = u32_to_fr(tx.token_id);
        encoded_tx[tx_detail_idx::ACCOUNT_ID2] = u32_to_fr(tx.account_id);
        encoded_tx[tx_detail_idx::BALANCE2] = tx.amount.to_fr();
        encoded_tx[tx_detail_idx::NONCE2] = Fr::zero();
        encoded_tx[tx_detail_idx::ETH_ADDR2] = tx.eth_addr;
        encoded_tx[tx_detail_idx::SIGN2] = tx.sign;
        encoded_tx[tx_detail_idx::AY2] = tx.ay;

        encoded_tx[tx_detail_idx::ENABLE_BALANCE_CHECK1] = Fr::one();
        encoded_tx[tx_detail_idx::ENABLE_BALANCE_CHECK2] = Fr::one();

        let mut raw_tx = RawTx {
            tx_type: TxType::DepositToNew,
            payload: encoded_tx.to_vec(),
            balance_path0: proof.balance_path.clone(),
            balance_path1: proof.balance_path.clone(),
            balance_path2: proof.balance_path.clone(),
            balance_path3: proof.balance_path,
            order_path0: self.state.trivial_order_path_elements(),
            order_path1: self.state.trivial_order_path_elements(),
            order_root0: acc.order_root,
            order_root1: acc.order_root,
            account_path0: proof.account_path.clone(),
            account_path1: proof.account_path,
            root_before: proof.root,
            root_after: Fr::zero(),
        };

        // then update global state
        self.state.set_token_balance(tx.account_id, tx.token_id, tx.amount.to_fr());
        self.state.set_account_l2_addr(tx.account_id, tx.sign, tx.ay, tx.eth_addr);

        raw_tx.root_after = self.state.root();
        self.add_raw_tx(raw_tx);
    }
    pub fn deposit_to_old(&mut self, tx: DepositToOldTx) {
        //assert!(self.accounts.get(tx.account_id).eth_addr != 0n, "deposit_to_old");
        let proof = self.state.balance_full_proof(tx.account_id, tx.token_id);
        let old_balance = self.state.get_token_balance(tx.account_id, tx.token_id);
        let acc = self.state.get_account(tx.account_id);
        let nonce = acc.nonce;
        // first, generate the tx

        let mut encoded_tx = [Fr::zero(); TX_LENGTH];
        encoded_tx[tx_detail_idx::AMOUNT] = tx.amount.to_fr();

        encoded_tx[tx_detail_idx::TOKEN_ID1] = u32_to_fr(tx.token_id);
        encoded_tx[tx_detail_idx::ACCOUNT_ID1] = u32_to_fr(tx.account_id);
        encoded_tx[tx_detail_idx::BALANCE1] = old_balance;
        encoded_tx[tx_detail_idx::NONCE1] = nonce;
        encoded_tx[tx_detail_idx::ETH_ADDR1] = acc.eth_addr;
        encoded_tx[tx_detail_idx::SIGN1] = acc.sign;
        encoded_tx[tx_detail_idx::AY1] = acc.ay;

        encoded_tx[tx_detail_idx::TOKEN_ID2] = u32_to_fr(tx.token_id);
        encoded_tx[tx_detail_idx::ACCOUNT_ID2] = u32_to_fr(tx.account_id);
        // TODO: rewrite this
        encoded_tx[tx_detail_idx::BALANCE2] = fr_add(&old_balance, &tx.amount.to_fr());
        encoded_tx[tx_detail_idx::NONCE2] = nonce;
        encoded_tx[tx_detail_idx::ETH_ADDR2] = acc.eth_addr;
        encoded_tx[tx_detail_idx::SIGN2] = acc.sign;
        encoded_tx[tx_detail_idx::AY2] = acc.ay;

        encoded_tx[tx_detail_idx::ENABLE_BALANCE_CHECK1] = Fr::one();
        encoded_tx[tx_detail_idx::ENABLE_BALANCE_CHECK2] = Fr::one();

        let mut raw_tx = RawTx {
            tx_type: TxType::DepositToOld,
            payload: encoded_tx.to_vec(),
            balance_path0: proof.balance_path.clone(),
            balance_path1: proof.balance_path.clone(),
            balance_path2: proof.balance_path.clone(),
            balance_path3: proof.balance_path,
            order_path0: self.state.trivial_order_path_elements(),
            order_path1: self.state.trivial_order_path_elements(),
            order_root0: acc.order_root,
            order_root1: acc.order_root,
            account_path0: proof.account_path.clone(),
            account_path1: proof.account_path,
            root_before: proof.root,
            root_after: Fr::zero(),
        };

        let mut balance = old_balance;
        balance.add_assign(&tx.amount.to_fr());
        self.state.set_token_balance(tx.account_id, tx.token_id, balance);

        raw_tx.root_after = self.state.root();
        self.add_raw_tx(raw_tx);
    }
    pub fn fill_transfer_tx(&self, tx: &mut TransferTx) {
        tx.from_nonce = self.state.get_account(tx.from).nonce;
        tx.to_nonce = self.state.get_account(tx.to).nonce;
        tx.old_balance_from = self.state.get_token_balance(tx.from, tx.token_id);
        tx.old_balance_to = self.state.get_token_balance(tx.to, tx.token_id);
    }
    pub fn fill_withdraw_tx(&self, tx: &mut WithdrawTx) {
        tx.nonce = self.state.get_account(tx.account_id).nonce;
        tx.old_balance = self.get_token_balance(tx.account_id, tx.token_id);
    }
    pub fn transfer(&mut self, tx: TransferTx) {
        // assert(this.accounts.get(tx.from).eth_addr != 0, 'TransferTx: empty fromAccount');
        // assert(this.accounts.get(tx.to).eth_addr != 0, 'Transfer: empty toAccount');

        let proof_from = self.state.balance_full_proof(tx.from, tx.token_id);
        let from_account = self.state.get_account(tx.from);
        let to_account = self.state.get_account(tx.to);

        let from_old_balance = self.get_token_balance(tx.from, tx.token_id);
        let to_old_balance = self.get_token_balance(tx.to, tx.token_id);
        let from_new_balance = fr_sub(&from_old_balance, &tx.amount.to_fr());
        let to_new_balance = fr_add(&to_old_balance, &tx.amount.to_fr());
        // assert(from_old_balance > tx.amount, 'Transfer balance not enough');

        let mut encoded_tx = [Fr::zero(); TX_LENGTH];
        encoded_tx[tx_detail_idx::ACCOUNT_ID1] = u32_to_fr(tx.from);
        encoded_tx[tx_detail_idx::ACCOUNT_ID2] = u32_to_fr(tx.to);
        encoded_tx[tx_detail_idx::TOKEN_ID1] = u32_to_fr(tx.token_id);
        encoded_tx[tx_detail_idx::AMOUNT] = tx.amount.to_fr();
        encoded_tx[tx_detail_idx::NONCE1] = from_account.nonce;
        encoded_tx[tx_detail_idx::NONCE2] = to_account.nonce;
        encoded_tx[tx_detail_idx::SIGN1] = from_account.sign;
        encoded_tx[tx_detail_idx::SIGN2] = to_account.sign;
        encoded_tx[tx_detail_idx::AY1] = from_account.ay;
        encoded_tx[tx_detail_idx::AY2] = to_account.ay;
        encoded_tx[tx_detail_idx::ETH_ADDR1] = from_account.eth_addr;
        encoded_tx[tx_detail_idx::ETH_ADDR2] = to_account.eth_addr;
        encoded_tx[tx_detail_idx::BALANCE1] = from_old_balance;
        encoded_tx[tx_detail_idx::BALANCE2] = to_new_balance;

        encoded_tx[tx_detail_idx::SIG_L2_HASH1] = tx.sig.hash;
        encoded_tx[tx_detail_idx::S1] = tx.sig.s;
        encoded_tx[tx_detail_idx::R8X1] = tx.sig.r8x;
        encoded_tx[tx_detail_idx::R8Y1] = tx.sig.r8y;
        encoded_tx[tx_detail_idx::ENABLE_BALANCE_CHECK1] = Fr::one();
        encoded_tx[tx_detail_idx::ENABLE_BALANCE_CHECK2] = Fr::one();
        encoded_tx[tx_detail_idx::ENABLE_SIG_CHECK1] = Fr::one();

        self.set_token_balance(tx.from, tx.token_id, from_new_balance);
        self.state.increase_nonce(tx.from);

        let proof_to = self.state.balance_full_proof(tx.to, tx.token_id);
        self.set_token_balance(tx.to, tx.token_id, to_new_balance);

        let raw_tx = RawTx {
            tx_type: TxType::Transfer,
            payload: encoded_tx.to_vec(),
            balance_path0: proof_from.balance_path.clone(),
            balance_path1: proof_to.balance_path.clone(),
            balance_path2: proof_from.balance_path,
            balance_path3: proof_to.balance_path,
            order_path0: self.state.trivial_order_path_elements(),
            order_path1: self.state.trivial_order_path_elements(),
            order_root0: from_account.order_root,
            order_root1: to_account.order_root,
            account_path0: proof_from.account_path,
            account_path1: proof_to.account_path,
            root_before: proof_from.root,
            root_after: self.root(),
        };

        self.add_raw_tx(raw_tx);
    }
    pub fn withdraw(&mut self, tx: WithdrawTx) {
        // assert(this.accounts.get(tx.accountID).ethAddr != 0n, 'Withdraw');
        let account_id = tx.account_id;
        let token_id = tx.token_id;
        let proof = self.state.balance_full_proof(account_id, token_id);

        let acc = self.state.get_account(account_id);
        let old_balance = self.get_token_balance(account_id, token_id);
        let new_balance = fr_sub(&old_balance, &tx.amount.to_fr());
        let nonce = acc.nonce;
        // assert(oldBalance > tx.amount, 'Withdraw balance');

        // first, generate the tx
        let mut encoded_tx = [Fr::zero(); TX_LENGTH];

        encoded_tx[tx_detail_idx::AMOUNT] = tx.amount.to_fr();

        encoded_tx[tx_detail_idx::TOKEN_ID1] = u32_to_fr(token_id);
        encoded_tx[tx_detail_idx::ACCOUNT_ID1] = u32_to_fr(account_id);
        encoded_tx[tx_detail_idx::BALANCE1] = old_balance;
        encoded_tx[tx_detail_idx::NONCE1] = nonce;
        encoded_tx[tx_detail_idx::ETH_ADDR1] = acc.eth_addr;
        encoded_tx[tx_detail_idx::SIGN1] = acc.sign;
        encoded_tx[tx_detail_idx::AY1] = acc.ay;

        encoded_tx[tx_detail_idx::TOKEN_ID2] = u32_to_fr(token_id);
        encoded_tx[tx_detail_idx::ACCOUNT_ID2] = u32_to_fr(account_id);
        encoded_tx[tx_detail_idx::BALANCE2] = new_balance;
        encoded_tx[tx_detail_idx::NONCE2] = fr_add(&nonce, &Fr::one());
        encoded_tx[tx_detail_idx::ETH_ADDR2] = acc.eth_addr;
        encoded_tx[tx_detail_idx::SIGN2] = acc.sign;
        encoded_tx[tx_detail_idx::AY2] = acc.ay;

        encoded_tx[tx_detail_idx::ENABLE_BALANCE_CHECK1] = Fr::one();
        encoded_tx[tx_detail_idx::ENABLE_BALANCE_CHECK2] = Fr::one();
        encoded_tx[tx_detail_idx::ENABLE_SIG_CHECK1] = Fr::one();

        encoded_tx[tx_detail_idx::SIG_L2_HASH1] = tx.sig.hash;
        encoded_tx[tx_detail_idx::S1] = tx.sig.s;
        encoded_tx[tx_detail_idx::R8X1] = tx.sig.r8x;
        encoded_tx[tx_detail_idx::R8Y1] = tx.sig.r8y;

        let mut raw_tx = RawTx {
            tx_type: TxType::Withdraw,
            payload: encoded_tx.to_vec(),
            balance_path0: proof.balance_path.clone(),
            balance_path1: proof.balance_path.clone(),
            balance_path2: proof.balance_path.clone(),
            balance_path3: proof.balance_path,
            order_path0: self.state.trivial_order_path_elements(),
            order_path1: self.state.trivial_order_path_elements(),
            order_root0: acc.order_root,
            order_root1: acc.order_root,
            account_path0: proof.account_path.clone(),
            account_path1: proof.account_path,
            root_before: proof.root,
            root_after: Fr::zero(),
        };

        self.state.set_token_balance(account_id, token_id, new_balance);
        self.state.increase_nonce(account_id);

        raw_tx.root_after = self.state.root();
        self.add_raw_tx(raw_tx);
    }
    // we keep full_spot_trade and spot_trade both now
    pub fn full_spot_trade(&mut self, tx: FullSpotTradeTx) {
        if !self.has_order(tx.maker_order.account_id, tx.maker_order.order_id) {
            assert_eq!(tx.maker_order.filled_buy, Fr::zero());
            assert_eq!(tx.maker_order.filled_sell, Fr::zero());
            self.state.update_order_state(tx.maker_order.account_id, tx.maker_order);
        }
        if !self.has_order(tx.taker_order.account_id, tx.taker_order.order_id) {
            assert_eq!(tx.taker_order.filled_buy, Fr::zero());
            assert_eq!(tx.taker_order.filled_sell, Fr::zero());
            self.state.update_order_state(tx.taker_order.account_id, tx.taker_order);
        }
        self.spot_trade(tx.trade);
    }
    // case1: old order is empty
    // case2: old order is valid old order with different order id, but we will replace it.
    // case3: old order has same order id, we will modify it
    pub fn spot_trade(&mut self, tx: SpotTradeTx) {
        //assert!(self.accounts.get(accID1).eth_addr != 0n, "SpotTrade account1");
        //assert!(self.accounts.get(accID2).eth_addr != 0n, "SpotTrade account2");
        let acc_id1 = tx.order1_account_id;
        let acc_id2 = tx.order2_account_id;
        if acc_id1 == acc_id2 {
            panic!("self trade no allowed");
        }
        assert!(self.state.has_order(acc_id1, tx.order1_id), "unknown order1");
        assert!(self.state.has_order(acc_id2, tx.order2_id), "unknown order2");

        let old_root = self.state.root();
        let proof_order1_seller = self.state.balance_full_proof(acc_id1, tx.token_id_1to2);
        let proof_order2_seller = self.state.balance_full_proof(acc_id2, tx.token_id_2to1);

        let account1 = self.state.get_account(acc_id1);
        let order_root0 = account1.order_root;
        let account2 = self.state.get_account(acc_id2);

        // old_order1 is same as old_order1_in_tree when case3
        // not same when case1 and case2
        let mut order1 = self.state.get_account_order_by_id(acc_id1, tx.order1_id);
        let mut order2 = self.state.get_account_order_by_id(acc_id2, tx.order2_id);
        let (order1_pos, old_order1_in_tree) = self.state.find_pos_for_order(acc_id1, tx.order1_id);
        let (order2_pos, old_order2_in_tree) = self.state.find_pos_for_order(acc_id2, tx.order2_id);

        // first, generate the tx

        let mut encoded_tx = [Fr::zero(); TX_LENGTH];
        encoded_tx[tx_detail_idx::ACCOUNT_ID1] = u32_to_fr(acc_id1);
        encoded_tx[tx_detail_idx::ACCOUNT_ID2] = u32_to_fr(acc_id2);
        encoded_tx[tx_detail_idx::ETH_ADDR1] = account1.eth_addr;
        encoded_tx[tx_detail_idx::ETH_ADDR2] = account2.eth_addr;
        encoded_tx[tx_detail_idx::SIGN1] = account1.sign;
        encoded_tx[tx_detail_idx::SIGN2] = account2.sign;
        encoded_tx[tx_detail_idx::AY1] = account1.ay;
        encoded_tx[tx_detail_idx::AY2] = account2.ay;
        encoded_tx[tx_detail_idx::NONCE1] = account1.nonce;
        encoded_tx[tx_detail_idx::NONCE2] = account2.nonce;

        encoded_tx[tx_detail_idx::S1] = order1.sig.s;
        encoded_tx[tx_detail_idx::R8X1] = order1.sig.r8x;
        encoded_tx[tx_detail_idx::R8Y1] = order1.sig.r8y;
        encoded_tx[tx_detail_idx::SIG_L2_HASH1] = order1.sig.hash;
        encoded_tx[tx_detail_idx::S2] = order2.sig.s;
        encoded_tx[tx_detail_idx::R8X2] = order2.sig.r8x;
        encoded_tx[tx_detail_idx::R8Y2] = order2.sig.r8y;
        encoded_tx[tx_detail_idx::SIG_L2_HASH2] = order2.sig.hash;

        encoded_tx[tx_detail_idx::OLD_ORDER1_ID] = u32_to_fr(old_order1_in_tree.order_id);
        encoded_tx[tx_detail_idx::OLD_ORDER1_TOKEN_SELL] = old_order1_in_tree.tokensell;
        encoded_tx[tx_detail_idx::OLD_ORDER1_FILLED_SELL] = old_order1_in_tree.filled_sell;
        encoded_tx[tx_detail_idx::OLD_ORDER1_AMOUNT_SELL] = old_order1_in_tree.total_sell;
        encoded_tx[tx_detail_idx::OLD_ORDER1_TOKEN_BUY] = old_order1_in_tree.tokenbuy;
        encoded_tx[tx_detail_idx::OLD_ORDER1_FILLED_BUY] = old_order1_in_tree.filled_buy;
        encoded_tx[tx_detail_idx::OLD_ORDER1_AMOUNT_BUY] = old_order1_in_tree.total_buy;

        encoded_tx[tx_detail_idx::OLD_ORDER2_ID] = u32_to_fr(old_order2_in_tree.order_id);
        encoded_tx[tx_detail_idx::OLD_ORDER2_TOKEN_SELL] = old_order2_in_tree.tokensell;
        encoded_tx[tx_detail_idx::OLD_ORDER2_FILLED_SELL] = old_order2_in_tree.filled_sell;
        encoded_tx[tx_detail_idx::OLD_ORDER2_AMOUNT_SELL] = old_order2_in_tree.total_sell;
        encoded_tx[tx_detail_idx::OLD_ORDER2_TOKEN_BUY] = old_order2_in_tree.tokenbuy;
        encoded_tx[tx_detail_idx::OLD_ORDER2_FILLED_BUY] = old_order2_in_tree.filled_buy;
        encoded_tx[tx_detail_idx::OLD_ORDER2_AMOUNT_BUY] = old_order2_in_tree.total_buy;

        encoded_tx[tx_detail_idx::AMOUNT] = tx.amount_1to2.to_fr();
        encoded_tx[tx_detail_idx::AMOUNT2] = tx.amount_2to1.to_fr();
        encoded_tx[tx_detail_idx::ORDER1_POS] = u32_to_fr(order1_pos);
        encoded_tx[tx_detail_idx::ORDER2_POS] = u32_to_fr(order2_pos);

        let acc1_balance_sell = self.state.get_token_balance(acc_id1, tx.token_id_1to2);
        assert!(acc1_balance_sell > tx.amount_1to2.to_fr(), "balance_1to2");
        let acc1_balance_sell_new = fr_sub(&acc1_balance_sell, &tx.amount_1to2.to_fr());
        let acc1_balance_buy = self.state.get_token_balance(acc_id1, tx.token_id_2to1);
        let acc1_balance_buy_new = fr_add(&acc1_balance_buy, &tx.amount_2to1.to_fr());

        let acc2_balance_sell = self.state.get_token_balance(acc_id2, tx.token_id_2to1);
        assert!(acc2_balance_sell > tx.amount_2to1.to_fr(), "balance_2to1");
        let acc2_balance_sell_new = fr_sub(&acc2_balance_sell, &tx.amount_2to1.to_fr());
        let acc2_balance_buy = self.state.get_token_balance(acc_id2, tx.token_id_1to2);
        let acc2_balance_buy_new = fr_add(&acc2_balance_buy, &tx.amount_1to2.to_fr());

        encoded_tx[tx_detail_idx::BALANCE1] = acc1_balance_sell;
        encoded_tx[tx_detail_idx::BALANCE2] = acc2_balance_buy_new;
        encoded_tx[tx_detail_idx::BALANCE3] = acc2_balance_sell;
        encoded_tx[tx_detail_idx::BALANCE4] = acc1_balance_buy_new;

        encoded_tx[tx_detail_idx::ENABLE_BALANCE_CHECK1] = Fr::one();
        encoded_tx[tx_detail_idx::ENABLE_BALANCE_CHECK2] = Fr::one();
        encoded_tx[tx_detail_idx::ENABLE_SIG_CHECK1] = Fr::one();
        encoded_tx[tx_detail_idx::ENABLE_SIG_CHECK2] = Fr::one();

        let mut raw_tx = RawTx {
            tx_type: TxType::SpotTrade,
            payload: Vec::default(),
            balance_path0: proof_order1_seller.balance_path,
            balance_path1: Default::default(),
            balance_path2: proof_order2_seller.balance_path,
            balance_path3: Default::default(),
            order_path0: self.state.order_proof(acc_id1, order1_pos).path_elements,
            order_path1: self.state.order_proof(acc_id2, order2_pos).path_elements,
            order_root0,
            order_root1: Default::default(),
            account_path0: proof_order1_seller.account_path,
            account_path1: Default::default(),
            root_before: old_root,
            root_after: Default::default(),
        };

        order1.trade_with(&tx.amount_1to2.to_fr(), &tx.amount_2to1.to_fr());
        self.state.update_order_state(acc_id1, order1);
        order2.trade_with(&tx.amount_2to1.to_fr(), &tx.amount_1to2.to_fr());
        self.state.update_order_state(acc_id2, order2);

        let acc1_updates = AccountUpdates {
            account_id: acc_id1,
            balance_updates: vec![(tx.token_id_1to2, acc1_balance_sell_new), (tx.token_id_2to1, acc1_balance_buy_new)],
            order_updates: vec![(order1_pos, order1.hash())],
        };
        let acc2_updates = AccountUpdates {
            account_id: acc_id2,
            balance_updates: vec![(tx.token_id_1to2, acc2_balance_buy_new), (tx.token_id_2to1, acc2_balance_sell_new)],
            order_updates: vec![(order2_pos, order2.hash())],
        };
        self.state.batch_update(vec![acc1_updates, acc2_updates], true);

        raw_tx.balance_path3 = self.state.balance_proof(acc_id1, tx.token_id_2to1).path_elements;
        raw_tx.balance_path1 = self.state.balance_proof(acc_id2, tx.token_id_1to2).path_elements;
        raw_tx.account_path1 = self.state.account_proof(acc_id2).path_elements;
        raw_tx.order_root1 = self.state.get_account(acc_id2).order_root;

        encoded_tx[tx_detail_idx::NEW_ORDER1_ID] = u32_to_fr(order1.order_id);
        encoded_tx[tx_detail_idx::NEW_ORDER1_TOKEN_SELL] = order1.tokensell;
        encoded_tx[tx_detail_idx::NEW_ORDER1_FILLED_SELL] = order1.filled_sell;
        encoded_tx[tx_detail_idx::NEW_ORDER1_AMOUNT_SELL] = order1.total_sell;
        encoded_tx[tx_detail_idx::NEW_ORDER1_TOKEN_BUY] = order1.tokenbuy;
        encoded_tx[tx_detail_idx::NEW_ORDER1_FILLED_BUY] = order1.filled_buy;
        encoded_tx[tx_detail_idx::NEW_ORDER1_AMOUNT_BUY] = order1.total_buy;

        encoded_tx[tx_detail_idx::NEW_ORDER2_ID] = u32_to_fr(order2.order_id);

        encoded_tx[tx_detail_idx::NEW_ORDER2_TOKEN_SELL] = order2.tokensell;
        encoded_tx[tx_detail_idx::NEW_ORDER2_FILLED_SELL] = order2.filled_sell;
        encoded_tx[tx_detail_idx::NEW_ORDER2_AMOUNT_SELL] = order2.total_sell;
        encoded_tx[tx_detail_idx::NEW_ORDER2_TOKEN_BUY] = order2.tokenbuy;
        encoded_tx[tx_detail_idx::NEW_ORDER2_FILLED_BUY] = order2.filled_buy;
        encoded_tx[tx_detail_idx::NEW_ORDER2_AMOUNT_BUY] = order2.total_buy;

        encoded_tx[tx_detail_idx::TOKEN_ID1] = order1.tokensell;
        encoded_tx[tx_detail_idx::TOKEN_ID2] = order2.tokenbuy;

        raw_tx.payload = encoded_tx.to_vec();
        raw_tx.root_after = self.state.root();
        self.add_raw_tx(raw_tx);
    }

    pub fn nop(&mut self) {
        // assume we already have initialized the account tree and the balance tree
        let trivial_proof = self.state.trivial_state_proof();
        let encoded_tx = [Fr::zero(); TX_LENGTH];
        let raw_tx = RawTx {
            tx_type: TxType::Nop,
            payload: encoded_tx.to_vec(),
            balance_path0: trivial_proof.balance_path.clone(),
            balance_path1: trivial_proof.balance_path.clone(),
            balance_path2: trivial_proof.balance_path.clone(),
            balance_path3: trivial_proof.balance_path,
            order_path0: self.state.trivial_order_path_elements(),
            order_path1: self.state.trivial_order_path_elements(),
            order_root0: Fr::zero(),
            order_root1: Fr::zero(),
            account_path0: trivial_proof.account_path.clone(),
            account_path1: trivial_proof.account_path,
            root_before: self.state.root(),
            root_after: self.state.root(),
        };
        self.add_raw_tx(raw_tx);
    }

    pub fn flush_with_nop(&mut self) {
        while self.buffered_txs.len() % self.n_tx != 0 {
            self.nop();
        }
    }
}
