#![allow(clippy::field_reassign_with_default)]

// from https://github1s.com/Fluidex/circuits/blob/HEAD/test/global_state.ts

use super::AccountState;
use crate::types::l2::Order;
use crate::types::merkle_tree::{empty_tree_root, MerkleProof, Tree};
use crate::types::primitives::{fr_to_u32, Fr};
use ff::Field;
use fnv::FnvHashMap;
use std::collections::BTreeMap;

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

// TODO: too many unwrap here
pub struct GlobalState {
    balance_levels: usize,
    order_levels: usize,
    account_levels: usize,
    account_tree: Tree,
    // idx to balanceTree
    balance_trees: FnvHashMap<u32, Tree>,
    // user -> order_id -> order
    order_map: FnvHashMap<u32, BTreeMap<u32, Order>>,
    // (user, order_id) -> order_pos
    order_id_to_pos: FnvHashMap<(u32, u32), u32>,
    // (user, order_pos) -> order_id
    order_pos_to_id: FnvHashMap<(u32, u32), u32>,
    // user -> order_pos -> order_hash
    order_trees: FnvHashMap<u32, Tree>,
    accounts: FnvHashMap<u32, AccountState>,
    default_balance_root: Fr,
    default_order_leaf: Fr,
    default_order_root: Fr,
    default_account_leaf: Fr,
    next_order_positions: FnvHashMap<u32, u32>,
    max_order_num_per_user: u32,

    // some precalculated items
    trivial_order_path_elements: Vec<[Fr; 1]>,

    verbose: bool,
}

impl GlobalState {
    pub fn print_config() {
        Tree::print_config();
    }

    pub fn new(balance_levels: usize, order_levels: usize, account_levels: usize, verbose: bool) -> Self {
        let default_balance_root = empty_tree_root(balance_levels, Fr::zero());
        let default_order_leaf = Order::default().hash();
        let dummy_order_tree = Tree::new(order_levels, default_order_leaf);
        let default_order_root = dummy_order_tree.get_root();
        let default_account_leaf = AccountState::empty(default_balance_root, default_order_root).hash();
        let max_order_num_per_user = dummy_order_tree.max_leaf_num();
        let trivial_order_path_elements = Tree::new(order_levels, Fr::zero()).get_proof(0).path_elements;
        Self {
            balance_levels,
            order_levels,
            account_levels,
            default_balance_root,
            default_order_leaf,
            default_order_root,
            // default_account_leaf depends on default_order_root and default_balance_root
            default_account_leaf,
            account_tree: Tree::new(account_levels, default_account_leaf), // Tree<account_hash>
            balance_trees: FnvHashMap::default(),                          // FnvHashMap[account_id]balance_tree
            order_trees: FnvHashMap::default(),                            // FnvHashMap[account_id]order_tree
            order_map: FnvHashMap::default(),
            order_id_to_pos: FnvHashMap::default(),
            order_pos_to_id: FnvHashMap::default(),
            accounts: FnvHashMap::default(), // FnvHashMap[account_id]acount_state
            next_order_positions: FnvHashMap::default(),
            max_order_num_per_user,
            trivial_order_path_elements,
            verbose,
        }
    }
    pub fn root(&self) -> Fr {
        self.account_tree.get_root()
    }
    pub fn recalculate_from_account_state(&mut self, account_id: u32) {
        let mut acc = self.accounts.get_mut(&account_id).unwrap();
        acc.balance_root = self.balance_trees.get(&account_id).unwrap().get_root();
        acc.order_root = self.order_trees.get(&account_id).unwrap().get_root();
        self.account_tree.set_value(account_id, acc.hash());
    }
    // deprecated
    fn recalculate_from_balance_tree(&mut self, account_id: u32) {
        self.accounts.get_mut(&account_id).unwrap().balance_root = self.balance_trees.get(&account_id).unwrap().get_root();
        self.recalculate_from_account_state(account_id);
    }
    // deprecated
    fn recalculate_from_order_tree(&mut self, account_id: u32) {
        self.accounts.get_mut(&account_id).unwrap().order_root = self.order_trees.get(&account_id).unwrap().get_root();
        self.recalculate_from_account_state(account_id);
    }
    /*
    pub fn setAccountKey(&mut self, account_id: Fr, account: Account) {
      //println!("setAccountKey", account_id);
      self.accounts.get(account_id).updateAccountKey(account);
      self.recalculate_from_account_state(account_id);
    }
    pub fn setAccountL2Addr(&mut self, account_id: Fr, sign, ay, eth_addr) {
      self.accounts.get(account_id).update_l2_addr(sign, ay, eth_addr);
      self.recalculate_from_account_state(account_id);
    }
    */
    pub fn get_l1_addr(&self, account_id: u32) -> Fr {
        return self.accounts.get(&account_id).unwrap().eth_addr;
    }

    // TODO: we should change account_id to u32 later?
    pub fn set_account_nonce(&mut self, account_id: u32, nonce: Fr) {
        self.accounts.get_mut(&account_id).unwrap().update_nonce(nonce);
        self.recalculate_from_account_state(account_id);
    }
    // self function should only be used in tests for convenience
    pub fn set_account_order_root(&mut self, account_id: u32, order_root: Fr) {
        self.accounts.get_mut(&account_id).unwrap().update_order_root(order_root);
        self.recalculate_from_account_state(account_id);
    }
    fn increase_nonce(&mut self, account_id: u32) {
        let mut nonce = self.accounts.get(&account_id).unwrap().nonce;
        nonce.add_assign(&Fr::one());
        //println!("oldNonce", oldNonce);
        self.set_account_nonce(account_id, nonce);
    }
    pub fn get_account(&self, account_id: u32) -> AccountState {
        *self.accounts.get(&account_id).unwrap()
    }
    fn get_next_order_pos_for_user(&self, account_id: u32) -> u32 {
        *self.next_order_positions.get(&account_id).unwrap()
    }
    pub fn create_new_account(&mut self, next_order_id: u32) -> u32 {
        let account_id = self.balance_trees.len() as u32;
        if account_id >= 2u32.pow(self.account_levels as u32) {
            panic!("account_id {} overflows for account_levels {}", account_id, self.account_levels);
        }

        let account_state = AccountState::empty(self.default_balance_root, self.default_order_root);
        self.accounts.insert(account_id, account_state);
        self.balance_trees.insert(account_id, Tree::new(self.balance_levels, Fr::zero()));
        self.order_trees
            .insert(account_id, Tree::new(self.order_levels, self.default_order_leaf));
        self.order_map.insert(account_id, BTreeMap::<u32, Order>::default());
        self.account_tree.set_value(account_id, self.default_account_leaf);
        self.next_order_positions.insert(account_id, next_order_id);
        //println!("add account", account_id);
        account_id
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
        self.order_trees.get_mut(&account_id).unwrap().set_value(order_pos, order.hash());
        self.order_map.get_mut(&account_id).unwrap().insert(order_pos, order);
        // TODO: better type here...
        let order_id: u32 = fr_to_u32(&order.order_id);
        self.order_id_to_pos.insert((account_id, order_id), order_pos);
        self.recalculate_from_order_tree(account_id);
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
        self.order_map
            .get_mut(&account_id)
            .unwrap()
            .insert(fr_to_u32(&order.order_id), order);
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
    pub fn set_order_leaf_hash(&mut self, account_id: u32, order_pos: u32, order_hash: &Fr) {
        self.set_order_leaf_hash_raw(account_id, order_pos, order_hash);
        self.recalculate_from_order_tree(account_id);
    }
    pub fn set_order_leaf_hash_raw(&mut self, account_id: u32, order_pos: u32, order_hash: &Fr) {
        assert!(self.order_trees.contains_key(&account_id), "set_order_leaf_hash_raw");
        self.order_trees.get_mut(&account_id).unwrap().set_value(order_pos, *order_hash);
    }

    pub fn get_token_balance(&self, account_id: u32, token_id: u32) -> Fr {
        self.balance_trees.get(&account_id).unwrap().get_leaf(token_id)
    }
    pub fn set_token_balance(&mut self, account_id: u32, token_id: u32, balance: Fr) {
        self.set_token_balance_raw(account_id, token_id, balance);
        self.recalculate_from_balance_tree(account_id);
    }
    pub fn set_token_balance_raw(&mut self, account_id: u32, token_id: u32, balance: Fr) {
        assert!(self.balance_trees.contains_key(&account_id), "set_token_balance");
        self.balance_trees.get_mut(&account_id).unwrap().set_value(token_id, balance);
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
        self.order_trees.get(&account_id).unwrap().get_proof(order_pos)
    }
    pub fn balance_proof(&self, account_id: u32, token_id: u32) -> MerkleProof {
        self.balance_trees.get(&account_id).unwrap().get_proof(token_id)
    }
    // get proof if `value` is in the tree without really updating
    //pub fn balance_proof_with(self, account_id: u32, token_id: u32, value: Fr) -> MerkleProof
    pub fn account_proof(&self, account_id: u32) -> MerkleProof {
        self.account_tree.get_proof(account_id)
    }
    pub fn balance_full_proof(&self, account_id: u32, token_id: u32) -> BalanceProof {
        let balance_proof = self.balance_proof(account_id, token_id);
        let account_proof = self.account_proof(account_id);
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
