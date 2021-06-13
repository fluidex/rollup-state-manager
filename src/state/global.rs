#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::vec_init_then_push)]

use super::AccountState;
use crate::types::l2::Order;
use crate::types::merkle_tree::{MerkleProof, Tree};
use crate::types::primitives::Fr;
use anyhow::bail;
use ff::Field;
use fnv::FnvHashMap;
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::{
    sync::{Arc, Mutex},
    thread,
};

pub struct BalanceProof {
    pub leaf: Fr,
    pub balance_path: Vec<[Fr; 1]>,
    // in fact we can calculate xx_root using leaf and path
    pub balance_root: Fr,
    pub account_hash: Fr,
    pub account_path: Vec<[Fr; 1]>,
    pub root: Fr,
}
pub struct OrderProof {
    pub leaf: Fr,
    pub order_path: Vec<[Fr; 1]>,
    pub order_root: Fr,
    pub account_hash: Fr,
    pub account_path: Vec<[Fr; 1]>,
    pub root: Fr,
}
#[derive(Clone)]
pub struct AccountUpdates {
    pub account_id: u32,
    pub balance_updates: Vec<(u32, Fr)>,
    pub order_updates: Vec<(u32, Fr)>,
}

// TODO: too many unwrap here
// TODO: do we really need Arc/Mutex?
pub struct GlobalState {
    balance_levels: usize,
    order_levels: usize,
    account_levels: usize,
    account_tree: Arc<Mutex<Tree>>,
    // idx to balanceTree
    balance_trees: FnvHashMap<u32, Arc<Mutex<Tree>>>,
    // user -> order_id -> order
    order_map: FnvHashMap<u32, BTreeMap<u32, Order>>,
    // (user, order_id) -> order_pos
    order_id_to_pos: FnvHashMap<(u32, u32), u32>,
    // (user, order_pos) -> order_id
    order_pos_to_id: FnvHashMap<(u32, u32), u32>,
    // user -> order_pos -> order_hash
    order_trees: FnvHashMap<u32, Arc<Mutex<Tree>>>,
    accounts: FnvHashMap<u32, AccountState>,
    default_balance_root: Fr,
    default_order_leaf: Fr,
    default_order_root: Fr,
    default_account_leaf: Fr,
    // TODO: id or pos?
    default_next_order_id: u32,
    next_order_positions: FnvHashMap<u32, u32>,
    max_order_num_per_user: u32,

    // some precalculated items
    empty_order_tree: Tree,
    empty_balance_tree: Tree,
    trivial_order_path_elements: Vec<[Fr; 1]>,

    verbose: bool,
}

impl GlobalState {
    pub fn print_config() {
        Tree::print_config();
    }

    pub fn new(balance_levels: usize, order_levels: usize, account_levels: usize, verbose: bool) -> Self {
        let empty_balance_tree = Tree::new(balance_levels, Fr::zero());
        let default_balance_root = empty_balance_tree.get_root();

        let default_order_leaf = Order::default().hash();
        let empty_order_tree = Tree::new(order_levels, default_order_leaf);
        let default_order_root = empty_order_tree.get_root();
        let trivial_order_path_elements = empty_order_tree.get_proof(0).path_elements;

        let default_account_leaf = AccountState::empty(default_balance_root, default_order_root).hash();
        let max_order_num_per_user = empty_order_tree.max_leaf_num();
        Self {
            balance_levels,
            order_levels,
            account_levels,
            default_balance_root,
            default_order_leaf,
            default_order_root,
            // default_account_leaf depends on default_order_root and default_balance_root
            default_account_leaf,
            default_next_order_id: 1,
            account_tree: Arc::new(Mutex::new(Tree::new(account_levels, default_account_leaf))), // Tree<account_hash>
            balance_trees: FnvHashMap::default(),                                                // FnvHashMap[account_id]balance_tree
            order_trees: FnvHashMap::default(),                                                  // FnvHashMap[account_id]order_tree
            order_map: FnvHashMap::default(),
            order_id_to_pos: FnvHashMap::default(),
            order_pos_to_id: FnvHashMap::default(),
            accounts: FnvHashMap::default(), // FnvHashMap[account_id]acount_state
            next_order_positions: FnvHashMap::default(),
            max_order_num_per_user,
            empty_balance_tree,
            empty_order_tree,
            trivial_order_path_elements,
            verbose,
        }
    }
    pub fn root(&self) -> Fr {
        self.account_tree.lock().unwrap().get_root()
    }
    fn recalculate_account_state_hash(&mut self, account_id: u32) -> Fr {
        let mut acc = self.accounts.get_mut(&account_id).unwrap();
        // TODO: for balance_root/order_root, we maintain two 'truth' here
        // not a good idea
        acc.balance_root = self.balance_trees.get(&account_id).unwrap().lock().unwrap().get_root();
        acc.order_root = self.order_trees.get(&account_id).unwrap().lock().unwrap().get_root();
        acc.hash()
    }
    pub fn flush_account_state(&mut self, account_id: u32) {
        let hash = self.recalculate_account_state_hash(account_id);
        let tree = self.account_tree.clone();
        tree.lock().unwrap().set_value(account_id, hash);
    }
    pub fn set_account_l2_addr(&mut self, account_id: u32, sign: Fr, ay: Fr, eth_addr: Fr) {
        let account = self.accounts.get_mut(&account_id).unwrap();
        account.update_l2_addr(sign, ay, eth_addr);
        self.account_tree.lock().unwrap().set_value(account_id, account.hash());
    }
    pub fn get_l1_addr(&self, account_id: u32) -> Fr {
        return self.accounts.get(&account_id).unwrap().eth_addr;
    }
    pub fn get_account_nonce(&self, account_id: u32) -> Fr {
        self.get_account(account_id).nonce
    }
    pub fn set_account_nonce(&mut self, account_id: u32, nonce: Fr) {
        self.accounts.get_mut(&account_id).unwrap().update_nonce(nonce);
        self.flush_account_state(account_id);
    }
    // this function should only be used in tests for convenience
    pub fn set_account_order_root(&mut self, account_id: u32, order_root: Fr) {
        self.accounts.get_mut(&account_id).unwrap().update_order_root(order_root);
        self.flush_account_state(account_id);
    }
    pub fn increase_nonce(&mut self, account_id: u32) {
        let mut nonce = self.accounts.get(&account_id).unwrap().nonce;
        nonce.add_assign(&Fr::one());
        //println!("oldNonce", oldNonce);
        self.set_account_nonce(account_id, nonce);
    }
    pub fn get_account(&self, account_id: u32) -> AccountState {
        self.accounts
            .get(&account_id)
            .cloned()
            .unwrap_or_else(|| AccountState::empty(self.default_balance_root, self.default_order_root))
    }
    pub fn has_account(&self, account_id: u32) -> bool {
        !self.get_account(account_id).ay.is_zero()
    }
    fn get_next_order_pos_for_user(&self, account_id: u32) -> u32 {
        *self.next_order_positions.get(&account_id).unwrap()
    }
    pub fn get_next_account_id(&self) -> anyhow::Result<u32> {
        // TODO: should this function return Err(...) when the tree is full?
        // TODO: we may need to allow sparse account tree later,
        // eg, account 1 and account 5 is created, while account 2/3/4 is empty
        let account_id = self.balance_trees.len() as u32;
        if account_id >= 2u32.pow(self.account_levels as u32) {
            bail!("account_id {} overflows for account_levels {}", account_id, self.account_levels);
        }
        Ok(account_id)
    }
    // TODO: private or public? It is better this function is called automatically
    // rather than being called manully by the caller
    pub fn init_account(&mut self, account_id: u32, next_order_id: u32) -> anyhow::Result<u32> {
        if self.accounts.contains_key(&account_id) {
            return Ok(account_id);
        }
        if account_id >= 2u32.pow(self.account_levels as u32) {
            bail!("account_id {} overflows for account_levels {}", account_id, self.account_levels);
        }
        let account_state = AccountState::empty(self.default_balance_root, self.default_order_root);
        self.accounts.insert(account_id, account_state);
        self.balance_trees
            .insert(account_id, Arc::new(Mutex::new(Tree::new(self.balance_levels, Fr::zero()))));
        self.order_trees.insert(
            account_id,
            Arc::new(Mutex::new(Tree::new(self.order_levels, self.default_order_leaf))),
        );
        self.order_map.insert(account_id, BTreeMap::<u32, Order>::default());
        self.account_tree.lock().unwrap().set_value(account_id, self.default_account_leaf);
        self.next_order_positions.insert(account_id, next_order_id);
        Ok(account_id)
    }
    pub fn create_new_account(&mut self, next_order_id: u32) -> anyhow::Result<u32> {
        let account_id = self.get_next_account_id()?;
        self.init_account(account_id, next_order_id)
    }
    pub fn get_order_pos_by_id(&self, account_id: u32, order_id: u32) -> u32 {
        *self.order_id_to_pos.get(&(account_id, order_id)).unwrap()
    }
    fn get_order_id_by_pos(&self, account_id: u32, order_pos: u32) -> Option<&u32> {
        self.order_pos_to_id.get(&(account_id, order_pos))
    }

    pub fn set_account_order(&mut self, account_id: u32, order_pos: u32, order: Order) {
        assert!(self.order_trees.contains_key(&account_id), "set_account_order");
        if order_pos >= 2u32.pow(self.order_levels as u32) {
            panic!("order_pos {} invalid for order_levels {}", order_pos, self.order_levels);
        }
        self.order_trees
            .get_mut(&account_id)
            .unwrap()
            .lock()
            .unwrap()
            .set_value(order_pos, order.hash());
        self.order_map.get_mut(&account_id).unwrap().insert(order_pos, order);
        let order_id: u32 = order.order_id;
        self.order_id_to_pos.insert((account_id, order_id), order_pos);
        self.flush_account_state(account_id);
    }

    // find a position range 0..2**n where the slot is either empty or occupied by a close order
    // so we can place the new order here
    fn update_next_order_pos(&mut self, account_id: u32, start_pos: u32) {
        for i in 0..2u32.pow(self.order_levels as u32) {
            let candidate_pos = (start_pos + i) % 2u32.pow(self.order_levels as u32);
            let order = self.get_account_order_by_pos(account_id, candidate_pos);
            let is_empty_or_filled = order.filled_buy >= order.total_buy || order.filled_sell >= order.total_sell;
            if is_empty_or_filled {
                self.next_order_positions.insert(account_id, candidate_pos);
                return;
            }
        }
        panic!("Cannot find order pos");
    }

    pub fn update_order_state(&mut self, account_id: u32, order: Order) {
        self.order_map.get_mut(&account_id).unwrap().insert(order.order_id, order);
    }
    pub fn find_pos_for_order(&mut self, account_id: u32, order_id: u32) -> (u32, Order) {
        if !self.has_order(account_id, order_id) {
            panic!("invalid order {} {}", account_id, order_id);
        }
        match self.order_id_to_pos.get(&(account_id, order_id)) {
            Some(pos) => (*pos, self.get_account_order_by_id(account_id, order_id)),
            None => {
                let pos = self.get_next_order_pos_for_user(account_id);
                let old_order = self.get_account_order_by_pos(account_id, pos);
                self.link_order_pos_and_id(account_id, pos, order_id);
                self.update_next_order_pos(account_id, pos + 1);
                (pos, old_order)
            }
        }
    }
    pub fn link_order_pos_and_id(&mut self, account_id: u32, order_pos: u32, order_id: u32) {
        assert!(self.order_trees.contains_key(&account_id), "link_order_pos_and_id");

        if order_pos >= 2u32.pow(self.order_levels as u32) {
            panic!("order position {} invalid", order_pos);
        }

        self.order_id_to_pos.insert((account_id, order_id), order_pos);
        self.order_pos_to_id.insert((account_id, order_pos), order_id);
    }
    pub fn set_order_leaf_hash(&mut self, account_id: u32, order_pos: u32, order_hash: Fr) {
        self.set_order_leaf_hash_raw(account_id, order_pos, order_hash);
        self.flush_account_state(account_id);
    }
    pub fn set_order_leaf_hash_raw(&mut self, account_id: u32, order_pos: u32, order_hash: Fr) {
        assert!(self.order_trees.contains_key(&account_id), "set_order_leaf_hash_raw");
        let tree = self.order_trees.get_mut(&account_id).unwrap().clone();
        tree.lock().unwrap().set_value(order_pos, order_hash);
    }

    pub fn get_token_balance(&self, account_id: u32, token_id: u32) -> Fr {
        if !self.has_account(account_id) {
            return Fr::zero();
        }
        self.balance_trees.get(&account_id).unwrap().lock().unwrap().get_leaf(token_id)
    }
    pub fn set_token_balance(&mut self, account_id: u32, token_id: u32, balance: Fr) {
        if !self.accounts.contains_key(&account_id) {
            self.init_account(account_id, self.default_next_order_id).unwrap();
        }
        self.set_token_balance_raw(account_id, token_id, balance);
        self.flush_account_state(account_id)
    }
    pub fn batch_update(&mut self, updates: Vec<AccountUpdates>, parallel: bool) {
        if parallel {
            let balance_parallel = 2;
            let order_parallel = 1;
            let account_parallel = 2;

            let (tx, rx) = crossbeam_channel::bounded::<(Arc<Mutex<Tree>>, Vec<(u32, Fr)>, usize)>(updates.len());

            let set_job = thread::spawn(move || {
                rx.into_iter().par_bridge().for_each(|(tree, updates, parallel)| {
                    tree.lock().unwrap().set_value_parallel(&updates, parallel);
                });
            });

            for update in updates.clone() {
                let account_id = update.account_id;
                assert!(self.balance_trees.contains_key(&account_id), "set_token_balance");
                let balance_tree = self.balance_trees.get_mut(&account_id).unwrap().clone();
                let balance_updates = update.balance_updates;
                tx.send((balance_tree, balance_updates, balance_parallel)).unwrap();
                assert!(self.order_trees.contains_key(&account_id), "set_order_leaf_hash_raw");
                let order_tree = self.order_trees.get_mut(&account_id).unwrap().clone();
                let order_updates = update.order_updates;
                tx.send((order_tree, order_updates, order_parallel)).unwrap();
            }

            set_job.join().unwrap();

            let mut account_updates = vec![];
            for update in updates {
                let account_hash = self.recalculate_account_state_hash(update.account_id);
                account_updates.push((update.account_id, account_hash));
            }
            self.account_tree
                .lock()
                .unwrap()
                .set_value_parallel(&account_updates, account_parallel);
        } else {
            for update in updates {
                let account_id = update.account_id;
                for balance_update in update.balance_updates {
                    self.set_token_balance_raw(account_id, balance_update.0, balance_update.1);
                }
                for order_update in update.order_updates {
                    self.set_order_leaf_hash_raw(account_id, order_update.0, order_update.1);
                }
                self.flush_account_state(account_id);
            }
        }
    }
    pub fn set_token_balance_raw(&mut self, account_id: u32, token_id: u32, balance: Fr) {
        assert!(self.balance_trees.contains_key(&account_id), "set_token_balance");
        let tree = self.balance_trees.get_mut(&account_id).unwrap().clone();
        tree.lock().unwrap().set_value(token_id, balance);
    }
    pub fn has_order(&self, account_id: u32, order_id: u32) -> bool {
        self.order_map.contains_key(&account_id) && self.order_map.get(&account_id).unwrap().contains_key(&order_id)
    }
    fn get_account_order_by_pos(&self, account_id: u32, order_pos: u32) -> Order {
        match self.get_order_id_by_pos(account_id, order_pos) {
            Some(order_id) => self.get_account_order_by_id(account_id, *order_id),
            None => Order::default(),
        }
    }
    pub fn get_account_order_by_id(&self, account_id: u32, order_id: u32) -> Order {
        *self.order_map.get(&account_id).unwrap().get(&order_id).unwrap()
    }

    pub fn trivial_order_path_elements(&self) -> Vec<[Fr; 1]> {
        self.trivial_order_path_elements.clone()
    }
    pub fn order_proof(&self, account_id: u32, order_pos: u32) -> MerkleProof {
        self.order_trees.get(&account_id).unwrap().lock().unwrap().get_proof(order_pos)
    }
    pub fn balance_proof(&self, account_id: u32, token_id: u32) -> MerkleProof {
        if self.balance_trees.contains_key(&account_id) {
            self.balance_trees.get(&account_id).unwrap().lock().unwrap().get_proof(token_id)
        } else {
            self.empty_balance_tree.get_proof(token_id)
        }
    }
    // get proof if `value` is in the tree without really updating
    //pub fn balance_proof_with(self, account_id: u32, token_id: u32, value: Fr) -> MerkleProof
    pub fn account_proof(&self, account_id: u32) -> MerkleProof {
        self.account_tree.lock().unwrap().get_proof(account_id)
    }
    pub fn balance_full_proof(&self, account_id: u32, token_id: u32) -> BalanceProof {
        let account_proof = self.account_proof(account_id);
        let balance_proof = self.balance_proof(account_id, token_id);
        BalanceProof {
            leaf: balance_proof.leaf,
            balance_path: balance_proof.path_elements,
            balance_root: balance_proof.root,
            account_hash: account_proof.leaf,
            account_path: account_proof.path_elements,
            root: account_proof.root,
        }
    }
    pub fn trivial_state_proof(&self) -> BalanceProof {
        // TODO: cache this
        self.balance_full_proof(0, 0)
    }
}
