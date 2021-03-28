// from https://github1s.com/Fluidex/circuits/blob/HEAD/test/global_state.ts

use super::common::{AccountState, DepositToOldTx, L2Block, Order, PlaceOrderTx, RawTx, SpotTradeTx, TxDetailIdx, TxLength, TxType};
use super::merkle_tree::{empty_tree_root, Tree};
use super::types::{u32_to_fr, Fr};
use ff::Field;
use fnv::FnvHashMap;

struct StateProof {
    leaf: Fr,
    root: Fr,
    balanceRoot: Fr,
    orderRoot: Fr,
    balancePath: Vec<[Fr; 1]>,
    accountPath: Vec<[Fr; 1]>,
}

// TODO: change to snake_case
// TODO: too many unwrap here
struct GlobalState {
    nTx: usize,
    balanceLevels: usize,
    orderLevels: usize,
    accountLevels: usize,
    accountTree: Tree,
    // idx to balanceTree
    balanceTrees: FnvHashMap<u32, Tree>,
    orderTrees: FnvHashMap<u32, Tree>,
    orderMap: FnvHashMap<u32, FnvHashMap<u32, Order>>,
    accounts: FnvHashMap<u32, AccountState>,
    bufferedTxs: Vec<RawTx>,
    bufferedBlocks: Vec<L2Block>,
    defaultBalanceRoot: Fr,
    defaultOrderLeaf: Fr,
    defaultOrderRoot: Fr,
    defaultAccountLeaf: Fr,
    nextOrderIds: FnvHashMap<u32, u32>,
    verbose: bool,
}

impl GlobalState {
    pub fn new(balanceLevels: usize, orderLevels: usize, accountLevels: usize, nTx: usize, verbose: bool) -> Self {
        let defaultBalanceRoot = empty_tree_root(balanceLevels, Fr::zero());
        let defaultOrderLeaf = Order::empty().hash();
        let defaultOrderRoot = empty_tree_root(orderLevels, defaultOrderLeaf);
        let defaultAccountLeaf = AccountState::empty(defaultBalanceRoot, defaultOrderRoot).hash();
        Self {
            balanceLevels,
            orderLevels,
            accountLevels,
            defaultBalanceRoot,
            defaultOrderLeaf,
            defaultOrderRoot,
            // defaultAccountLeaf depends on defaultOrderRoot and defaultBalanceRoot
            defaultAccountLeaf,
            accountTree: Tree::new(accountLevels, defaultAccountLeaf), // Tree<account_hash>
            balanceTrees: FnvHashMap::default(),                       // FnvHashMap[account_id]balance_tree
            orderTrees: FnvHashMap::default(),                         // FnvHashMap[account_id]order_tree
            orderMap: FnvHashMap::default(),
            accounts: FnvHashMap::default(), // FnvHashMap[account_id]acount_state
            bufferedTxs: Vec::new(),
            bufferedBlocks: Vec::new(),
            nextOrderIds: FnvHashMap::default(),
            nTx,
            verbose,
        }
    }
    pub fn root(&self) -> Fr {
        return self.accountTree.get_root();
    }
    fn recalculateFromAccountState(&mut self, accountID: u32) {
        self.accountTree.set_value(accountID, self.accounts.get(&accountID).unwrap().hash());
    }
    fn recalculateFromBalanceTree(&mut self, accountID: u32) {
        self.accounts.get_mut(&accountID).unwrap().balanceRoot = self.balanceTrees.get(&accountID).unwrap().get_root();
        self.recalculateFromAccountState(accountID);
    }
    fn recalculateFromOrderTree(&mut self, accountID: u32) {
        self.accounts.get_mut(&accountID).unwrap().orderRoot = self.orderTrees.get(&accountID).unwrap().get_root();
        self.recalculateFromAccountState(accountID);
    }
    /*
    pub fn setAccountKey(&mut self, accountID: Fr, account: Account) {
      //println!("setAccountKey", accountID);
      self.accounts.get(accountID).updateAccountKey(account);
      self.recalculateFromAccountState(accountID);
    }
    pub fn setAccountL2Addr(&mut self, accountID: Fr, sign, ay, ethAddr) {
      self.accounts.get(accountID).updateL2Addr(sign, ay, ethAddr);
      self.recalculateFromAccountState(accountID);
    }
    */
    // TODO: we should change accountID to u32 later?
    pub fn setAccountNonce(&mut self, accountID: u32, nonce: Fr) {
        self.accounts.get_mut(&accountID).unwrap().updateNonce(nonce);
        self.recalculateFromAccountState(accountID);
    }
    // self function should only be used in tests for convenience
    pub fn setAccountOrderRoot(&mut self, accountID: u32, orderRoot: Fr) {
        self.accounts.get_mut(&accountID).unwrap().updateOrderRoot(orderRoot);
        self.recalculateFromAccountState(accountID);
    }
    fn increaseNonce(&mut self, accountID: u32) {
        let mut nonce = self.accounts.get(&accountID).unwrap().nonce;
        nonce.add_assign(&Fr::one());
        //println!("oldNonce", oldNonce);
        self.setAccountNonce(accountID, nonce);
    }
    pub fn getAccount(&self, account_id: u32) -> AccountState {
        *self.accounts.get(&account_id).unwrap()
    }
    fn getNextOrderIdForUser(&self, accountID: u32) -> u32 {
        *self.nextOrderIds.get(&accountID).unwrap()
    }
    pub fn createNewAccount(&mut self, next_order_id: u32) -> u32 {
        let accountID = self.balanceTrees.len() as u32;
        if accountID >= 2u32.pow(self.accountLevels as u32) {
            panic!("account_id {} overflows for accountLevels {}", accountID, self.accountLevels);
        }

        let accountState = AccountState::empty(self.defaultBalanceRoot, self.defaultOrderRoot);
        self.accounts.insert(accountID, accountState);
        self.balanceTrees.insert(accountID, Tree::new(self.balanceLevels, Fr::zero()));
        self.orderTrees
            .insert(accountID, Tree::new(self.orderLevels, self.defaultOrderLeaf));
        self.orderMap.insert(accountID, FnvHashMap::<u32, Order>::default());
        self.accountTree.set_value(accountID, self.defaultAccountLeaf);
        self.nextOrderIds.insert(accountID, next_order_id);
        //println!("add account", accountID);
        return accountID;
    }

    pub fn setAccountOrder(&mut self, accountID: u32, orderID: u32, order: Order) {
        assert!(self.orderTrees.contains_key(&accountID), "setAccountOrder");
        if orderID >= 2u32.pow(self.orderLevels as u32) {
            panic!("order_id {} overflows for orderLevels {}", orderID, self.orderLevels);
        }
        self.orderTrees.get_mut(&accountID).unwrap().set_value(orderID, order.hash());
        self.orderMap.get_mut(&accountID).unwrap().insert(orderID, order);
        self.recalculateFromOrderTree(accountID);
    }
    pub fn createNewOrder(&mut self, tx: &PlaceOrderTx) -> u32 {
        let mut orderID = self.getNextOrderIdForUser(tx.accountID);
        if orderID >= 2u32.pow(self.orderLevels as u32) {
            panic!("order_id {} overflows for orderLevels {}", orderID, self.orderLevels);
        }

        let order = Order {
            status: Fr::zero(), //open
            tokenbuy: u32_to_fr(tx.tokenID_buy),
            tokensell: u32_to_fr(tx.tokenID_sell),
            filled_sell: Fr::zero(),
            filled_buy: Fr::zero(),
            total_sell: tx.amount_sell,
            total_buy: tx.amount_buy,
        };
        self.setAccountOrder(tx.accountID, orderID, order);
        self.nextOrderIds.insert(tx.accountID, orderID + 1);
        orderID
    }

    pub fn getTokenBalance(&self, accountID: u32, tokenID: u32) -> Fr {
        self.balanceTrees.get(&accountID).unwrap().get_leaf(tokenID)
    }
    pub fn setTokenBalance(&mut self, accountID: u32, tokenID: u32, balance: Fr) {
        assert!(self.balanceTrees.contains_key(&accountID), "setTokenBalance");
        self.balanceTrees.get_mut(&accountID).unwrap().set_value(tokenID, balance);
        self.recalculateFromBalanceTree(accountID);
    }
    pub fn getAccountOrder(&self, accountID: u32, orderID: u32) -> Order {
        *self.orderMap.get(&accountID).unwrap().get(&orderID).unwrap()
    }

    pub fn trivialOrderPathElements(&self) -> Vec<[Fr; 1]> {
        Tree::new(self.orderLevels, Fr::zero()).get_proof(0).path_elements
    }

    pub fn stateProof(&self, accountID: u32, tokenID: u32) -> StateProof {
        let balanceProof = self.balanceTrees.get(&accountID).unwrap().get_proof(tokenID);
        let orderRoot = self.orderTrees.get(&accountID).unwrap().get_root();
        let accountProof = self.accountTree.get_proof(accountID);
        //assert!(accountLeaf == balanceRoot, "stateProof");
        StateProof {
            leaf: balanceProof.leaf,
            root: accountProof.root,
            balanceRoot: balanceProof.root,
            orderRoot,
            balancePath: balanceProof.path_elements,
            accountPath: accountProof.path_elements,
        }
    }
    pub fn getL1Addr(&self, accountID: u32) -> Fr {
        return self.accounts.get(&accountID).unwrap().ethAddr;
    }
    pub fn forgeWithTxs(&self, bufferedTxs: &[RawTx]) -> L2Block {
        assert!(bufferedTxs.len() == self.nTx, "invalid txs len");
        let txsType = bufferedTxs.iter().map(|tx| tx.txType).collect();
        let encodedTxs = bufferedTxs.iter().map(|tx| tx.payload.clone()).collect();
        let balance_path_elements = bufferedTxs
            .iter()
            .map(|tx| {
                [
                    tx.balancePath0.clone(),
                    tx.balancePath1.clone(),
                    tx.balancePath2.clone(),
                    tx.balancePath3.clone(),
                ]
            })
            .collect();
        let order_path_elements = bufferedTxs
            .iter()
            .map(|tx| [tx.orderPath0.clone(), tx.orderPath1.clone()])
            .collect();
        let orderRoots = bufferedTxs.iter().map(|tx| [tx.orderRoot0, tx.orderRoot1]).collect();
        let account_path_elements = bufferedTxs
            .iter()
            .map(|tx| [tx.accountPath0.clone(), tx.accountPath1.clone()])
            .collect();
        let oldAccountRoots = bufferedTxs.iter().map(|tx| tx.rootBefore).collect();
        let newAccountRoots = bufferedTxs.iter().map(|tx| tx.rootAfter).collect();
        L2Block {
            txsType,
            encodedTxs,
            balance_path_elements,
            order_path_elements,
            account_path_elements,
            orderRoots,
            oldAccountRoots,
            newAccountRoots,
        }
    }
    pub fn forge(&self) -> L2Block {
        return self.forgeWithTxs(&self.bufferedTxs);
    }
    pub fn addRawTx(&mut self, rawTx: RawTx) {
        self.bufferedTxs.push(rawTx);
        if self.bufferedTxs.len() % self.nTx == 0 {
            // forge next block, using last nTx txs
            let txs = &self.bufferedTxs[(self.bufferedTxs.len() - self.nTx)..];
            let block = self.forgeWithTxs(txs);
            self.bufferedBlocks.push(block);
            assert!(self.bufferedBlocks.len() * self.nTx == self.bufferedTxs.len(), "invalid block num");
            if self.verbose {
                println!("forge block {} done", self.bufferedBlocks.len() - 1);
            }
        }
    }
    /*
    DepositToNew(tx: DepositToNewTx) {
      assert!(self.accounts.get(tx.accountID).ethAddr == 0n, "DepositToNew");
      let proof = self.stateProof(tx.accountID, tx.tokenID);
      // first, generate the tx
      let encodedTx: Array<Fr> = new Array(Txlen());
      encodedTx.fill(0n, 0, Txlen());
      encodedTx[TxDetailIdx::TokenID] = Scalar.e(tx.tokenID);
      encodedTx[TxDetailIdx::Amount] = tx.amount;
      encodedTx[TxDetailIdx::AccountID2] = Scalar.e(tx.accountID);
      encodedTx[TxDetailIdx::EthAddr2] = tx.ethAddr;
      encodedTx[TxDetailIdx::Sign2] = Scalar.e(tx.sign);
      encodedTx[TxDetailIdx::Ay2] = tx.ay;
      let rawTx: RawTx = {
        txType: TxType.DepositToNew,
        payload: encodedTx,
        balancePath0: proof.balancePath,
        balancePath1: proof.balancePath,
        balancePath2: proof.balancePath,
        balancePath3: proof.balancePath,
        orderPath0: self.trivialOrderPathElements(),
        orderPath1: self.trivialOrderPathElements(),
        orderRoot0: self.defaultOrderRoot,
        orderRoot1: self.defaultOrderRoot,
        accountPath0: proof.accountPath,
        accountPath1: proof.accountPath,
        rootBefore: proof.root,
        rootAfter: 0n,
      };

      // then update global state
      self.setTokenBalance(tx.accountID, tx.tokenID, tx.amount);
      self.setAccountL2Addr(tx.accountID, tx.sign, tx.ay, tx.ethAddr);
      rawTx.rootAfter = self.root();
      self.addRawTx(rawTx);
    }
    */
    pub fn DepositToOld(&mut self, tx: DepositToOldTx) {
        //assert!(self.accounts.get(tx.accountID).ethAddr != 0n, "DepositToOld");
        let proof = self.stateProof(tx.accountID, tx.tokenID);
        // first, generate the tx

        let mut encodedTx = [Fr::zero(); TxLength];
        encodedTx[TxDetailIdx::TokenID] = u32_to_fr(tx.tokenID);
        encodedTx[TxDetailIdx::Amount] = tx.amount;
        encodedTx[TxDetailIdx::AccountID2] = u32_to_fr(tx.accountID);
        let oldBalance = self.getTokenBalance(tx.accountID, tx.tokenID);
        encodedTx[TxDetailIdx::Balance2] = oldBalance;
        encodedTx[TxDetailIdx::Nonce2] = self.accounts.get(&tx.accountID).unwrap().nonce;
        let acc = self.accounts.get(&tx.accountID).unwrap();
        encodedTx[TxDetailIdx::EthAddr2] = acc.ethAddr;
        encodedTx[TxDetailIdx::Sign2] = acc.sign;
        encodedTx[TxDetailIdx::Ay2] = acc.ay;

        let mut rawTx = RawTx {
            txType: TxType::DepositToOld,
            payload: encodedTx.to_vec(),
            balancePath0: proof.balancePath.clone(),
            balancePath1: proof.balancePath.clone(),
            balancePath2: proof.balancePath.clone(),
            balancePath3: proof.balancePath,
            orderPath0: self.trivialOrderPathElements(),
            orderPath1: self.trivialOrderPathElements(),
            orderRoot0: acc.orderRoot,
            orderRoot1: acc.orderRoot,
            accountPath0: proof.accountPath.clone(),
            accountPath1: proof.accountPath,
            rootBefore: proof.root,
            rootAfter: Fr::zero(),
        };

        let mut balance = oldBalance;
        balance.add_assign(&tx.amount);
        self.setTokenBalance(tx.accountID, tx.tokenID, balance);

        rawTx.rootAfter = self.root();
        self.addRawTx(rawTx);
    }
    /*
    fillTransferTx(tx: TranferTx) {
      let fullTx = {
        from: tx.from,
        to: tx.to,
        tokenID: tx.tokenID,
        amount: tx.amount,
        fromNonce: self.accounts.get(tx.from).nonce,
        toNonce: self.accounts.get(tx.to).nonce,
        oldBalanceFrom: self.getTokenBalance(tx.from, tx.tokenID),
        oldBalanceTo: self.getTokenBalance(tx.to, tx.tokenID),
      };
      return fullTx;
    }
    fillWithdrawTx(tx: WithdrawTx) {
      let fullTx = {
        accountID: tx.accountID,
        tokenID: tx.tokenID,
        amount: tx.amount,
        nonce: self.accounts.get(tx.accountID).nonce,
        oldBalance: self.getTokenBalance(tx.accountID, tx.tokenID),
      };
      return fullTx;
    }
    Transfer(tx: TranferTx) {
      assert!(self.accounts.get(tx.from).ethAddr != 0n, "TransferTx: empty fromAccount");
      assert!(self.accounts.get(tx.to).ethAddr != 0n, "Transfer: empty toAccount");
      let proofFrom = self.stateProof(tx.from, tx.tokenID);
      let fromAccount = self.accounts.get(tx.from);
      let toAccount = self.accounts.get(tx.to);

      // first, generate the tx
      let encodedTx: Array<Fr> = new Array(Txlen());
      encodedTx.fill(0n, 0, Txlen());

      let fromOldBalance = self.getTokenBalance(tx.from, tx.tokenID);
      let toOldBalance = self.getTokenBalance(tx.to, tx.tokenID);
      assert!(fromOldBalance > tx.amount, "Transfer balance not enough");
      encodedTx[TxDetailIdx::AccountID1] = tx.from;
      encodedTx[TxDetailIdx::AccountID2] = tx.to;
      encodedTx[TxDetailIdx::TokenID] = tx.tokenID;
      encodedTx[TxDetailIdx::Amount] = tx.amount;
      encodedTx[TxDetailIdx::Nonce1] = fromAccount.nonce;
      encodedTx[TxDetailIdx::Nonce2] = toAccount.nonce;
      encodedTx[TxDetailIdx::Sign1] = fromAccount.sign;
      encodedTx[TxDetailIdx::Sign2] = toAccount.sign;
      encodedTx[TxDetailIdx::Ay1] = fromAccount.ay;
      encodedTx[TxDetailIdx::Ay2] = toAccount.ay;
      encodedTx[TxDetailIdx::EthAddr1] = fromAccount.ethAddr;
      encodedTx[TxDetailIdx::EthAddr2] = toAccount.ethAddr;
      encodedTx[TxDetailIdx::Balance1] = fromOldBalance;
      encodedTx[TxDetailIdx::Balance2] = toOldBalance;
      encodedTx[TxDetailIdx::SigL2Hash] = tx.signature.hash;
      encodedTx[TxDetailIdx::S] = tx.signature.S;
      encodedTx[TxDetailIdx::R8x] = tx.signature.R8x;
      encodedTx[TxDetailIdx::R8y] = tx.signature.R8y;

      let rawTx: RawTx = {
        txType: TxType.Transfer,
        payload: encodedTx,
        balancePath0: proofFrom.balancePath,
        balancePath1: null,
        balancePath2: proofFrom.balancePath,
        balancePath3: null,
        orderPath0: self.trivialOrderPathElements(),
        orderPath1: self.trivialOrderPathElements(),
        orderRoot0: fromAccount.orderRoot,
        orderRoot1: toAccount.orderRoot,
        accountPath0: proofFrom.accountPath,
        accountPath1: null,
        rootBefore: proofFrom.root,
        rootAfter: 0n,
      };

      self.setTokenBalance(tx.from, tx.tokenID, fromOldBalance - tx.amount);
      self.increaseNonce(tx.from);

      let proofTo = self.stateProof(tx.to, tx.tokenID);
      rawTx.balancePath1 = proofTo.balancePath;
      rawTx.balancePath3 = proofTo.balancePath;
      rawTx.accountPath1 = proofTo.accountPath;
      self.setTokenBalance(tx.to, tx.tokenID, toOldBalance + tx.amount);

      rawTx.rootAfter = self.root();
      self.addRawTx(rawTx);
    }
    Withdraw(tx: WithdrawTx) {
      assert!(self.accounts.get(tx.accountID).ethAddr != 0n, "Withdraw");
      let proof = self.stateProof(tx.accountID, tx.tokenID);
      // first, generate the tx
      let encodedTx: Array<Fr> = new Array(Txlen());
      encodedTx.fill(0n, 0, Txlen());

      let acc = self.accounts.get(tx.accountID);
      let balanceBefore = self.getTokenBalance(tx.accountID, tx.tokenID);
      assert!(balanceBefore > tx.amount, "Withdraw balance");
      encodedTx[TxDetailIdx::AccountID1] = tx.accountID;
      encodedTx[TxDetailIdx::TokenID] = tx.tokenID;
      encodedTx[TxDetailIdx::Amount] = tx.amount;
      encodedTx[TxDetailIdx::Nonce1] = acc.nonce;
      encodedTx[TxDetailIdx::Sign1] = acc.sign;
      encodedTx[TxDetailIdx::Ay1] = acc.ay;
      encodedTx[TxDetailIdx::EthAddr1] = acc.ethAddr;
      encodedTx[TxDetailIdx::Balance1] = balanceBefore;

      encodedTx[TxDetailIdx::SigL2Hash] = tx.signature.hash;
      encodedTx[TxDetailIdx::S] = tx.signature.S;
      encodedTx[TxDetailIdx::R8x] = tx.signature.R8x;
      encodedTx[TxDetailIdx::R8y] = tx.signature.R8y;

      let rawTx: RawTx = {
        txType: TxType.Withdraw,
        payload: encodedTx,
        balancePath0: proof.balancePath,
        balancePath1: proof.balancePath,
        balancePath2: proof.balancePath,
        balancePath3: proof.balancePath,
        orderPath0: self.trivialOrderPathElements(),
        orderPath1: self.trivialOrderPathElements(),
        orderRoot0: acc.orderRoot,
        orderRoot1: acc.orderRoot,
        accountPath0: proof.accountPath,
        accountPath1: proof.accountPath,
        rootBefore: proof.root,
        rootAfter: 0n,
      };

      self.setTokenBalance(tx.accountID, tx.tokenID, balanceBefore - tx.amount);
      self.increaseNonce(tx.accountID);

      rawTx.rootAfter = self.root();
      self.addRawTx(rawTx);
    }
    */
    pub fn PlaceOrder(&mut self, tx: PlaceOrderTx) -> u32 {
        if self.verbose {
            //println!("PlaceOrder", tx, "operation id", self.bufferedTxs.len());
        }
        // TODO: check order signature
        //assert!(self.accounts.get(tx.accountID).ethAddr != 0n, "PlaceOrder account: accountID" + tx.accountID);

        let account = *self.accounts.get(&tx.accountID).unwrap();
        let proof = self.stateProof(tx.accountID, tx.tokenID_sell);

        let mut rawTx = RawTx {
            txType: TxType::PlaceOrder,
            payload: Default::default(),
            balancePath0: proof.balancePath.clone(),
            balancePath1: proof.balancePath.clone(),
            balancePath2: proof.balancePath.clone(),
            balancePath3: proof.balancePath,
            orderPath0: Default::default(),
            orderPath1: self.trivialOrderPathElements(),
            orderRoot0: account.orderRoot,
            orderRoot1: Default::default(),
            accountPath0: proof.accountPath.clone(),
            accountPath1: proof.accountPath,
            rootBefore: self.root(),
            rootAfter: Default::default(),
        };
        //println!("orderRoo0", rawTx.orderRoot0);

        let order_id = self.createNewOrder(&tx);

        // fill in the tx

        let mut encodedTx = [Fr::zero(); TxLength];
        encodedTx[TxDetailIdx::Order1ID] = u32_to_fr(order_id);
        encodedTx[TxDetailIdx::TokenID] = u32_to_fr(tx.previous_tokenID_sell);
        encodedTx[TxDetailIdx::TokenID2] = u32_to_fr(tx.previous_tokenID_buy);
        encodedTx[TxDetailIdx::TokenID3] = u32_to_fr(tx.tokenID_sell);
        encodedTx[TxDetailIdx::TokenID4] = u32_to_fr(tx.tokenID_buy);
        encodedTx[TxDetailIdx::AccountID1] = u32_to_fr(tx.accountID.clone());
        encodedTx[TxDetailIdx::EthAddr1] = account.ethAddr;
        encodedTx[TxDetailIdx::Sign1] = account.sign;
        encodedTx[TxDetailIdx::Ay1] = account.ay;
        encodedTx[TxDetailIdx::Nonce1] = account.nonce;
        encodedTx[TxDetailIdx::Balance1] = proof.leaf;
        encodedTx[TxDetailIdx::Order1AmountSell] = tx.previous_amount_sell;
        encodedTx[TxDetailIdx::Order1AmountBuy] = tx.previous_amount_buy;
        encodedTx[TxDetailIdx::Order1FilledSell] = tx.previous_filled_sell;
        encodedTx[TxDetailIdx::Order1FilledBuy] = tx.previous_filled_buy;
        encodedTx[TxDetailIdx::Order2AmountSell] = tx.amount_sell;
        encodedTx[TxDetailIdx::Order2AmountBuy] = tx.amount_buy;
        rawTx.payload = encodedTx.to_vec();
        rawTx.orderPath0 = self.orderTrees.get(&tx.accountID).unwrap().get_proof(order_id).path_elements;
        //println!("rawTx.orderPath0", rawTx.orderPath0)
        rawTx.orderRoot1 = self.orderTrees.get(&tx.accountID).unwrap().get_proof(order_id).root;

        rawTx.rootAfter = self.root();
        self.addRawTx(rawTx);
        if self.verbose {
            //println!("create order ", order_id, tx);
        }
        return order_id;
    }
    pub fn SpotTrade(&mut self, tx: SpotTradeTx) {
        //assert!(self.accounts.get(tx.order1_accountID).ethAddr != 0n, "SpotTrade account1");
        //assert!(self.accounts.get(tx.order2_accountID).ethAddr != 0n, "SpotTrade account2");

        assert!(tx.order1_id < 2u32.pow(self.orderLevels as u32), "order1 id overflows");
        assert!(tx.order2_id < 2u32.pow(self.orderLevels as u32), "order2 id overflows");

        let account1 = self.accounts.get(&tx.order1_accountID).unwrap();
        let account2 = self.accounts.get(&tx.order2_accountID).unwrap();
        let proof_order1_seller = self.stateProof(tx.order1_accountID, tx.tokenID_1to2);
        let proof_order2_seller = self.stateProof(tx.order2_accountID, tx.tokenID_2to1);

        let order1 = *self.orderMap.get(&tx.order1_accountID).unwrap().get(&tx.order1_id).unwrap();
        let order2 = *self.orderMap.get(&tx.order2_accountID).unwrap().get(&tx.order2_id).unwrap();

        // first, generate the tx

        let mut encodedTx = [Fr::zero(); TxLength];
        encodedTx[TxDetailIdx::AccountID1] = u32_to_fr(tx.order1_accountID);
        encodedTx[TxDetailIdx::AccountID2] = u32_to_fr(tx.order2_accountID);
        encodedTx[TxDetailIdx::EthAddr1] = account1.ethAddr;
        encodedTx[TxDetailIdx::EthAddr2] = account2.ethAddr;
        encodedTx[TxDetailIdx::Sign1] = account1.sign;
        encodedTx[TxDetailIdx::Sign2] = account2.sign;
        encodedTx[TxDetailIdx::Ay1] = account1.ay;
        encodedTx[TxDetailIdx::Ay2] = account2.ay;
        encodedTx[TxDetailIdx::Nonce1] = account1.nonce;
        encodedTx[TxDetailIdx::Nonce2] = account2.nonce;
        let account1_balance_sell = self.getTokenBalance(tx.order1_accountID, tx.tokenID_1to2);
        let account2_balance_buy = self.getTokenBalance(tx.order2_accountID, tx.tokenID_1to2);
        let account2_balance_sell = self.getTokenBalance(tx.order2_accountID, tx.tokenID_2to1);
        let account1_balance_buy = self.getTokenBalance(tx.order1_accountID, tx.tokenID_2to1);
        assert!(account1_balance_sell > tx.amount_1to2, "balance_1to2");
        assert!(account2_balance_sell > tx.amount_2to1, "balance_2to1");
        encodedTx[TxDetailIdx::TokenID] = u32_to_fr(tx.tokenID_1to2);
        encodedTx[TxDetailIdx::Amount] = tx.amount_1to2;
        encodedTx[TxDetailIdx::Balance1] = account1_balance_sell;
        encodedTx[TxDetailIdx::Balance2] = account2_balance_buy;
        encodedTx[TxDetailIdx::Balance3] = account2_balance_sell;
        encodedTx[TxDetailIdx::Balance4] = account1_balance_buy;
        encodedTx[TxDetailIdx::TokenID2] = u32_to_fr(tx.tokenID_2to1);
        encodedTx[TxDetailIdx::Amount2] = tx.amount_2to1;
        encodedTx[TxDetailIdx::Order1ID] = u32_to_fr(tx.order1_id);
        encodedTx[TxDetailIdx::Order1AmountSell] = order1.total_sell;
        encodedTx[TxDetailIdx::Order1AmountBuy] = order1.total_buy;
        encodedTx[TxDetailIdx::Order1FilledSell] = order1.filled_sell;
        encodedTx[TxDetailIdx::Order1FilledBuy] = order1.filled_buy;
        encodedTx[TxDetailIdx::Order2ID] = u32_to_fr(tx.order2_id);
        encodedTx[TxDetailIdx::Order2AmountSell] = order2.total_sell;
        encodedTx[TxDetailIdx::Order2AmountBuy] = order2.total_buy;
        encodedTx[TxDetailIdx::Order2FilledSell] = order2.filled_sell;
        encodedTx[TxDetailIdx::Order2FilledBuy] = order2.filled_buy;

        let mut rawTx = RawTx {
            txType: TxType::SpotTrade,
            payload: encodedTx.to_vec(),
            balancePath0: proof_order1_seller.balancePath,
            balancePath1: Default::default(),
            balancePath2: proof_order2_seller.balancePath,
            balancePath3: Default::default(),
            orderPath0: self
                .orderTrees
                .get(&tx.order1_accountID)
                .unwrap()
                .get_proof(tx.order1_id)
                .path_elements,
            orderPath1: self
                .orderTrees
                .get(&tx.order2_accountID)
                .unwrap()
                .get_proof(tx.order2_id)
                .path_elements,
            orderRoot0: account1.orderRoot, // not really used in the circuit
            orderRoot1: account2.orderRoot, // not really used in the circuit
            accountPath0: proof_order1_seller.accountPath,
            accountPath1: Default::default(),
            rootBefore: self.root(),
            rootAfter: Default::default(),
        };

        // do not update state root
        // account1 after sending, before receiving
        let mut balance1 = account1_balance_sell;
        balance1.sub_assign(&tx.amount_1to2);
        self.balanceTrees
            .get_mut(&tx.order1_accountID)
            .unwrap()
            .set_value(tx.tokenID_1to2, balance1);
        rawTx.balancePath3 = self
            .balanceTrees
            .get(&tx.order1_accountID)
            .unwrap()
            .get_proof(tx.tokenID_2to1)
            .path_elements;
        // account2 after sending, before receiving
        let mut balance2 = account2_balance_sell;
        balance2.sub_assign(&tx.amount_2to1);
        self.balanceTrees
            .get_mut(&tx.order2_accountID)
            .unwrap()
            .set_value(tx.tokenID_2to1, balance2);
        rawTx.balancePath1 = self
            .balanceTrees
            .get(&tx.order2_accountID)
            .unwrap()
            .get_proof(tx.tokenID_1to2)
            .path_elements;

        let mut order1_filled_sell = order1.filled_sell;
        order1_filled_sell.add_assign(&tx.amount_1to2);
        let mut order1_filled_buy = order1.filled_buy;
        order1_filled_buy.add_assign(&tx.amount_2to1);
        let newOrder1 = Order {
            status: Fr::zero(), // open
            tokenbuy: u32_to_fr(tx.tokenID_2to1),
            tokensell: u32_to_fr(tx.tokenID_1to2),
            filled_sell: order1_filled_sell,
            filled_buy: order1_filled_buy,
            total_sell: order1.total_sell,
            total_buy: order1.total_buy,
        };
        self.setAccountOrder(tx.order1_accountID, tx.order1_id, newOrder1);
        let mut account1_balance_buy = account1_balance_buy;
        account1_balance_buy.add_assign(&tx.amount_2to1);
        self.setTokenBalance(tx.order1_accountID, tx.tokenID_2to1, account1_balance_buy);
        rawTx.accountPath1 = self.accountTree.get_proof(tx.order2_accountID).path_elements;

        let mut order2_filled_sell = order2.filled_sell;
        order2_filled_sell.add_assign(&tx.amount_2to1);
        let mut order2_filled_buy = order2.filled_buy;
        order2_filled_buy.add_assign(&tx.amount_1to2);
        let newOrder2 = Order {
            status: Fr::zero(), // open
            tokenbuy: u32_to_fr(tx.tokenID_1to2),
            tokensell: u32_to_fr(tx.tokenID_2to1),
            filled_sell: order2_filled_sell,
            filled_buy: order2_filled_buy,
            total_sell: order2.total_sell,
            total_buy: order2.total_buy,
        };
        self.setAccountOrder(tx.order2_accountID, tx.order2_id, newOrder2);
        let mut account2_balance_buy = account2_balance_buy;
        account2_balance_buy.add_assign(&tx.amount_1to2);
        self.setTokenBalance(tx.order2_accountID, tx.tokenID_1to2, account2_balance_buy);

        rawTx.rootAfter = self.root();
        self.addRawTx(rawTx);
    }
    pub fn Nop(&mut self) {
        // assume we already have initialized the account tree and the balance tree
        let trivialProof = self.stateProof(0, 0);
        let mut encodedTx = [Fr::zero(); TxLength];
        let rawTx = RawTx {
            txType: TxType::Nop,
            payload: encodedTx.to_vec(),
            balancePath0: trivialProof.balancePath.clone(),
            balancePath1: trivialProof.balancePath.clone(),
            balancePath2: trivialProof.balancePath.clone(),
            balancePath3: trivialProof.balancePath,
            orderPath0: self.trivialOrderPathElements(),
            orderPath1: self.trivialOrderPathElements(),
            orderRoot0: trivialProof.orderRoot,
            orderRoot1: trivialProof.orderRoot,
            accountPath0: trivialProof.accountPath.clone(),
            accountPath1: trivialProof.accountPath,
            rootBefore: self.root(),
            rootAfter: self.root(),
        };
        self.addRawTx(rawTx);
    }

    pub fn flushWithNop(&mut self) {
        while self.bufferedTxs.len() % self.nTx != 0 {
            self.Nop();
        }
    }
}
