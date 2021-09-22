#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::vec_init_then_push)]

use super::AccountState;
#[cfg(feature = "persist_sled")]
use crate::r#const::sled_db::*;
use crate::types::l2::Order;
use crate::types::merkle_tree::{MerkleProof, Tree};
use anyhow::bail;
use fluidex_common::ff::Field;
use fluidex_common::fnv::FnvHashMap;
#[cfg(feature = "persist_sled")]
use fluidex_common::serde::FrBytes;
use fluidex_common::Fr;
use rayon::prelude::*;
#[cfg(feature = "persist_sled")]
use serde::Serialize;
#[cfg(feature = "persist_sled")]
use sled::transaction::ConflictableTransactionError;
#[cfg(feature = "persist_sled")]
use sled::transaction::{TransactionError, Transactional, TransactionalTree};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

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
#[derive(Clone, Default)]
pub struct AccountUpdates {
    pub account_id: u32,
    pub balance_updates: Vec<(u32, Fr)>,
    pub order_updates: Vec<(u32, Fr)>,
    pub new_nonce: Option<Fr>,
}

#[derive(Debug, thiserror::Error)]
pub enum GlobalStateError {
    #[error(transparent)]
    #[cfg(feature = "persist_sled")]
    Sled(#[from] sled::Error),
    #[error(transparent)]
    #[cfg(feature = "persist_sled")]
    Bincode(#[from] bincode::Error),
    #[error("requested content not found in db")]
    NotFound,
}

#[derive(Debug, thiserror::Error)]
#[cfg(feature = "persist_sled")]
enum GlobalStateInternalError {
    #[error(transparent)]
    Unabortable(#[from] sled::transaction::UnabortableTransactionError),
    #[error(transparent)]
    Bincode(#[from] bincode::Error),
    #[error("requested content not found in db")]
    NotFound,
}

#[cfg(feature = "persist_sled")]
impl From<GlobalStateInternalError> for ConflictableTransactionError<GlobalStateError> {
    fn from(e: GlobalStateInternalError) -> Self {
        use GlobalStateInternalError::*;
        match e {
            Unabortable(ute) => ute.into(),
            Bincode(e) => Self::Abort(e.into()),
            NotFound => Self::Abort(GlobalStateError::NotFound),
        }
    }
}

#[cfg(feature = "persist_sled")]
impl From<TransactionError<GlobalStateError>> for GlobalStateError {
    fn from(e: TransactionError<GlobalStateError>) -> Self {
        use TransactionError::*;

        match e {
            Abort(e) => e,
            Storage(e) => e.into(),
        }
    }
}

type Result<T, E = GlobalStateError> = std::result::Result<T, E>;

// TODO: too many unwrap here
// TODO: do we really need Arc/Mutex?
pub struct GlobalState {
    balance_levels: usize,
    order_levels: usize,
    account_levels: usize,

    // account_id -> acount_state_hash
    account_tree: Arc<Mutex<Tree>>,
    // account_id -> acount_state
    account_states: FnvHashMap<u32, AccountState>,
    // account_id -> token_id -> balance
    balance_trees: FnvHashMap<u32, Arc<Mutex<Tree>>>,
    // account_id -> order_pos -> order_hash
    order_trees: FnvHashMap<u32, Arc<Mutex<Tree>>>,
    // account_id -> order_pos -> order
    order_states: FnvHashMap<u32, BTreeMap<u32, Order>>,
    // (account_id, order_id) -> order_pos
    order_id_to_pos: FnvHashMap<(u32, u32), u32>,

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
    allow_overwrite_order_leaf: bool,
}

impl GlobalState {
    pub fn print_config() {
        Tree::print_config();
    }

    pub fn balance_bits(&self) -> usize {
        self.balance_levels
    }
    pub fn order_bits(&self) -> usize {
        self.order_levels
    }
    pub fn account_bits(&self) -> usize {
        self.account_levels
    }

    pub fn new(balance_levels: usize, order_levels: usize, account_levels: usize, verbose: bool) -> Self {
        log::debug!(
            "create GlobalState account_levels {} balance_levels {} order_levels {}",
            account_levels,
            balance_levels,
            order_levels
        );
        let empty_balance_tree = Tree::new(balance_levels, Fr::zero());
        let default_balance_root = empty_balance_tree.get_root();

        let default_order_leaf = Order::default().hash();
        let empty_order_tree = Tree::new(order_levels, default_order_leaf);
        let default_order_root = empty_order_tree.get_root();
        let trivial_order_path_elements = empty_order_tree.get_proof(0).path_elements;

        // default_account_leaf depends on default_order_root and default_balance_root
        let default_account_leaf = AccountState::empty(default_balance_root, default_order_root).hash();
        let max_order_num_per_user = empty_order_tree.max_leaf_num();
        let account_tree = Arc::new(Mutex::new(Tree::new(account_levels, default_account_leaf)));
        Self {
            balance_levels,
            order_levels,
            account_levels,
            default_balance_root,
            default_order_leaf,
            default_order_root,
            default_account_leaf,
            default_next_order_id: 1,
            account_tree,
            balance_trees: FnvHashMap::default(),
            order_trees: FnvHashMap::default(),
            order_states: FnvHashMap::default(),
            order_id_to_pos: FnvHashMap::default(),
            account_states: FnvHashMap::default(),
            next_order_positions: FnvHashMap::default(),
            max_order_num_per_user,
            empty_balance_tree,
            empty_order_tree,
            trivial_order_path_elements,
            verbose,
            allow_overwrite_order_leaf: true,
        }
    }
    pub fn root(&self) -> Fr {
        self.account_tree.lock().unwrap().get_root()
    }
    fn recalculate_account_state_hash(&mut self, account_id: u32) -> Fr {
        let mut acc = self.account_states.get_mut(&account_id).unwrap();
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
        let account = self.account_states.get_mut(&account_id).unwrap();
        account.update_l2_addr(sign, ay, eth_addr);
        self.account_tree.lock().unwrap().set_value(account_id, account.hash());
    }
    pub fn get_l1_addr(&self, account_id: u32) -> Fr {
        return self.account_states.get(&account_id).unwrap().eth_addr;
    }
    pub fn get_account_nonce(&self, account_id: u32) -> Fr {
        self.get_account(account_id).nonce
    }
    pub fn set_account_nonce(&mut self, account_id: u32, nonce: Fr) {
        self.account_states.get_mut(&account_id).unwrap().update_nonce(nonce);
        self.flush_account_state(account_id);
    }
    // this function should only be used in tests for convenience
    pub fn set_account_order_root(&mut self, account_id: u32, order_root: Fr) {
        self.account_states.get_mut(&account_id).unwrap().update_order_root(order_root);
        self.flush_account_state(account_id);
    }
    pub fn increase_nonce(&mut self, account_id: u32) {
        let mut nonce = self.account_states.get(&account_id).unwrap().nonce;
        nonce.add_assign(&Fr::one());
        //println!("oldNonce", oldNonce);
        self.set_account_nonce(account_id, nonce);
    }
    pub fn get_account(&self, account_id: u32) -> AccountState {
        self.account_states
            .get(&account_id)
            .cloned()
            .unwrap_or_else(|| AccountState::empty(self.default_balance_root, self.default_order_root))
    }
    pub fn has_account(&self, account_id: u32) -> bool {
        !self.get_account(account_id).ay.is_zero()
    }

    // find a position range 0..2**n where the slot is either empty or occupied by a close order
    // so we can place the new order here
    fn get_next_order_pos_for_user(&mut self, account_id: u32, order_id: u32) -> u32 {
        let order_state_tree = self.order_states.get(&account_id).unwrap();
        let order_num = order_state_tree.len();
        debug_assert!(order_num <= self.max_order_num_per_user as usize);
        if order_num < self.max_order_num_per_user as usize {
            if cfg!(debug_assertions) {
                debug_assert!(order_state_tree.is_empty() || *order_state_tree.iter().rev().next().unwrap().0 == order_num as u32 - 1);
            }
            // return the last leaf location
            return order_num as u32;
        }
        // now the tree is full
        // we have to find a vicvim order to replace
        if self.allow_overwrite_order_leaf {
            let start_pos = *self.next_order_positions.get(&account_id).unwrap();
            for i in 0..2u32.pow(self.order_levels as u32) {
                let candidate_pos = (start_pos + i) % 2u32.pow(self.order_levels as u32);
                let order = self.get_account_order_by_pos(account_id, candidate_pos);
                debug_assert!(!order.is_default());
                if order.is_filled() || !order.is_active {
                    assert_ne!(order_id, order.order_id, "order already in tree, why search location for it?");
                    if order.order_id < order_id {
                        self.next_order_positions.insert(account_id, candidate_pos + 1);
                        log::debug!(
                            "replace order uid {} old order {} new order {} at {}. reason: {}",
                            account_id,
                            order.order_id,
                            order_id,
                            candidate_pos,
                            if order.is_filled() { "filled" } else { "cancelled" }
                        );
                        return candidate_pos;
                    }
                }
            }
        }
        panic!(
            "Cannot find order pos, please use larger order tree height. account_id {} order_id {}",
            account_id, order_id
        );
    }
    pub fn get_next_account_id(&self) -> anyhow::Result<u32> {
        let account_id = self.balance_trees.len() as u32;
        if account_id >= 2u32.pow(self.account_levels as u32) {
            bail!("account_id {} overflows for account_levels {}", account_id, self.account_levels);
        }
        Ok(account_id)
    }
    fn init_account(&mut self, account_id: u32, next_order_id: u32) -> anyhow::Result<u32> {
        if self.account_states.contains_key(&account_id) {
            return Ok(account_id);
        }
        if account_id >= 2u32.pow(self.account_levels as u32) {
            bail!("account_id {} overflows for account_levels {}", account_id, self.account_levels);
        }
        let account_state = AccountState::empty(self.default_balance_root, self.default_order_root);
        self.account_states.insert(account_id, account_state);
        self.balance_trees
            .insert(account_id, Arc::new(Mutex::new(Tree::new(self.balance_levels, Fr::zero()))));
        self.order_trees.insert(
            account_id,
            Arc::new(Mutex::new(Tree::new(self.order_levels, self.default_order_leaf))),
        );
        self.order_states.insert(account_id, BTreeMap::<u32, Order>::default());
        self.account_tree.lock().unwrap().set_value(account_id, self.default_account_leaf);
        self.next_order_positions.insert(account_id, next_order_id);
        Ok(account_id)
    }
    pub fn create_new_account(&mut self, next_order_id: u32) -> anyhow::Result<u32> {
        let account_id = self.get_next_account_id()?;
        self.init_account(account_id, next_order_id)
    }
    pub fn get_order_pos_by_id(&self, account_id: u32, order_id: u32) -> Option<u32> {
        self.order_id_to_pos.get(&(account_id, order_id)).cloned()
    }
    pub fn get_order_id_by_pos(&self, account_id: u32, order_pos: u32) -> Option<u32> {
        self.order_states.get(&account_id).unwrap().get(&order_pos).map(|o| o.account_id)
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
        self.order_states.get_mut(&account_id).unwrap().insert(order_pos, order);
        let order_id: u32 = order.order_id;
        self.order_id_to_pos.insert((account_id, order_id), order_pos);
        self.flush_account_state(account_id);
    }

    pub fn update_order_state(&mut self, account_id: u32, order_pos: u32, order: Order) {
        self.order_states.get_mut(&account_id).unwrap().insert(order_pos, order);
    }
    pub fn find_or_insert_order(&mut self, account_id: u32, order: &Order) -> (u32, Order) {
        let order_id = order.order_id;
        match self.get_order_pos_by_id(account_id, order_id) {
            Some(pos) => (pos, self.get_account_order_by_pos(account_id, pos)),
            None => {
                let pos = self.get_next_order_pos_for_user(account_id, order_id);
                // old_order may be empty
                let old_order = self.get_account_order_by_pos(account_id, pos);
                self.set_order_pos_for_id(account_id, pos, order_id);
                (pos, old_order)
            }
        }
    }
    fn set_order_pos_for_id(&mut self, account_id: u32, order_pos: u32, order_id: u32) {
        assert!(self.order_trees.contains_key(&account_id), "link_order_pos_and_id");

        if order_pos >= 2u32.pow(self.order_levels as u32) {
            panic!("order position {} invalid", order_pos);
        }

        self.order_id_to_pos.insert((account_id, order_id), order_pos);
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
        if !self.account_states.contains_key(&account_id) {
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

            updates
                .clone()
                .into_iter()
                .map(|update| {
                    let account_id = update.account_id;
                    assert!(self.balance_trees.contains_key(&account_id), "set_token_balance");
                    let balance_tree = self.balance_trees.get_mut(&account_id).unwrap().clone();
                    let balance_updates = update.balance_updates;

                    assert!(self.order_trees.contains_key(&account_id), "set_order_leaf_hash_raw");
                    let order_tree = self.order_trees.get_mut(&account_id).unwrap().clone();
                    let order_updates = update.order_updates;

                    (
                        (balance_tree, balance_updates, balance_parallel),
                        (order_tree, order_updates, order_parallel),
                    )
                })
                .flat_map(|(balance, order)| std::array::IntoIter::new([balance, order]))
                .par_bridge()
                .for_each(|(tree, updates, parallel)| {
                    tree.lock().unwrap().set_value_parallel(updates.as_slice(), parallel);
                });

            let mut account_updates = vec![];
            for update in updates {
                if let Some(nonce) = update.new_nonce {
                    self.account_states.get_mut(&update.account_id).unwrap().update_nonce(nonce);
                }
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
                if let Some(nonce) = update.new_nonce {
                    self.account_states.get_mut(&update.account_id).unwrap().update_nonce(nonce);
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
        self.order_id_to_pos.contains_key(&(account_id, order_id))
        //self.order_map.contains_key(&account_id) && self.order_map.get(&account_id).unwrap().contains_key(&order_id)
    }
    pub fn cancel_order(&mut self, account_id: u32, order_id: u32) {
        let order_pos = self.get_order_pos_by_id(account_id, order_id).unwrap();
        log::debug!(
            "cancel order account_id {} order_id {} order_pos {}",
            account_id,
            order_id,
            order_pos
        );
        self.order_states
            .get_mut(&account_id)
            .unwrap()
            .get_mut(&order_pos)
            .unwrap()
            .is_active = false;
    }
    fn get_account_order_by_pos(&self, account_id: u32, order_pos: u32) -> Order {
        *self
            .order_states
            .get(&account_id)
            .unwrap()
            .get(&order_pos)
            .unwrap_or(&Order::default())
    }
    pub fn get_account_order_by_id(&self, account_id: u32, order_id: u32) -> Order {
        assert!(self.has_order(account_id, order_id));
        let order_pos = self.get_order_pos_by_id(account_id, order_id).unwrap();
        self.get_account_order_by_pos(account_id, order_pos)
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

    #[cfg(feature = "persist_sled")]
    pub fn load_persist(&mut self, db: &sled::Db) -> Result<()> {
        let account_states = db.open_tree(ACCOUNTSTATES_KEY)?;
        let balance_trees = db.open_tree(BALANCETREES_KEY)?;
        let order_trees = db.open_tree(ORDERTREES_KEY)?;
        let order_states = db.open_tree(ORDERSTATES_KEY)?;
        let next_order_positions = db.open_tree(NEXT_ORDER_POSITIONS_KEY)?;
        let (account_tree, account_states, balance_trees, order_trees, order_states, next_order_positions) = (
            &**db,
            &account_states,
            &balance_trees,
            &order_trees,
            &order_states,
            &next_order_positions,
        )
            .transaction(
                |(db, account_states, balance_trees, order_trees, order_states, next_order_positions)| {
                    let account_tree = Self::load_account_tree(db)?;
                    let account_states = Self::load_account_state(&account_tree, account_states)?;
                    let balance_trees = Self::load_trees::<Arc<Mutex<Tree>>>(&account_states, balance_trees)?;
                    let order_trees = Self::load_trees::<Arc<Mutex<Tree>>>(&account_states, order_trees)?;
                    let order_states = Self::load_trees::<BTreeMap<u32, Order>>(&account_states, order_states)?;
                    let next_order_positions = Self::load_trees::<u32>(&account_states, next_order_positions)?;
                    Ok((
                        account_tree,
                        account_states,
                        balance_trees,
                        order_trees,
                        order_states,
                        next_order_positions,
                    ))
                },
            )?;
        self.account_tree = Arc::new(Mutex::new(account_tree));
        self.account_states = account_states;
        self.balance_trees = balance_trees;
        self.order_trees = order_trees;
        self.order_states = order_states;
        // order_id_to_pos[account_id][order_id] === order_pos and order_states[account_id][order_pos].order_id === order_id
        self.order_id_to_pos = self
            .order_states
            .iter()
            .map(|(account_id, orders)| {
                orders
                    .iter()
                    .map(move |(order_pos, order)| ((*account_id, order.order_id), *order_pos))
            })
            .flatten()
            .collect();
        self.next_order_positions = next_order_positions;
        Ok(())
    }

    #[cfg(feature = "persist_sled")]
    fn load_account_tree(db: &TransactionalTree) -> Result<Tree, GlobalStateInternalError> {
        Ok(bincode::deserialize(
            db.get(ACCOUNTTREE_KEY)?.ok_or(GlobalStateInternalError::NotFound)?.as_ref(),
        )?)
    }

    #[cfg(feature = "persist_sled")]
    fn load_account_state(account_tree: &Tree, db: &TransactionalTree) -> Result<FnvHashMap<u32, AccountState>, GlobalStateInternalError> {
        #[derive(Serialize)]
        struct FrWrapper(#[serde(with = "FrBytes")] Fr);

        account_tree
            .iter()
            .map(|(_id, hash)| match bincode::serialize(&FrWrapper(*hash)) {
                Ok(key) => db
                    .get(key)
                    .map_err(GlobalStateInternalError::from)
                    .and_then(|v| v.ok_or(GlobalStateInternalError::NotFound))
                    .and_then(|v| bincode::deserialize::<(u32, AccountState)>(v.as_ref()).map_err(GlobalStateInternalError::from)),
                Err(e) => Err(e.into()),
            })
            .collect::<Result<FnvHashMap<u32, AccountState>, GlobalStateInternalError>>()
    }

    #[cfg(feature = "persist_sled")]
    fn load_trees<T: serde::de::DeserializeOwned>(
        account_states: &FnvHashMap<u32, AccountState>,
        db: &TransactionalTree,
    ) -> Result<FnvHashMap<u32, T>, GlobalStateInternalError> {
        account_states
            .iter()
            .map(|(id, _state)| match bincode::serialize(id) {
                Ok(key) => db
                    .get(key)
                    .map_err(GlobalStateInternalError::from)
                    .and_then(|v| v.ok_or(GlobalStateInternalError::NotFound))
                    .and_then(|v| bincode::deserialize::<T>(v.as_ref()).map_err(GlobalStateInternalError::from))
                    .map(|tree| (*id, tree)),
                Err(e) => Err(e.into()),
            })
            .collect::<Result<FnvHashMap<u32, T>, GlobalStateInternalError>>()
    }

    #[cfg(feature = "persist_sled")]
    pub fn persist(&self, db: &sled::Db) -> Result<()> {
        let account_states = db.open_tree(ACCOUNTSTATES_KEY)?;
        let balance_trees = db.open_tree(BALANCETREES_KEY)?;
        let order_trees = db.open_tree(ORDERTREES_KEY)?;
        let order_states = db.open_tree(ORDERSTATES_KEY)?;
        let next_order_positions = db.open_tree(NEXT_ORDER_POSITIONS_KEY)?;
        (
            &**db,
            &account_states,
            &balance_trees,
            &order_trees,
            &order_states,
            &next_order_positions,
        )
            .transaction(
                |(db, account_states, balance_trees, order_trees, order_states, next_order_positions)| {
                    self.save_account_tree(db)?;
                    self.save_account_state(account_states)?;
                    self.save_balance_trees(balance_trees)?;
                    self.save_order_trees(order_trees)?;
                    self.save_order_states(order_states)?;
                    self.save_next_order_positions(next_order_positions)?;
                    Ok(())
                },
            )?;
        Ok(())
    }

    #[cfg(feature = "persist_sled")]
    fn save_account_state(&self, db: &TransactionalTree) -> Result<(), GlobalStateInternalError> {
        #[derive(Serialize)]
        struct FrWrapper(#[serde(with = "FrBytes")] Fr);

        self.account_states.iter().try_for_each(|(id, state)| {
            db.insert(bincode::serialize(&FrWrapper(state.hash()))?, bincode::serialize(&(id, state))?)
                .map(|_| ())
                .map_err(GlobalStateInternalError::from)
        })
    }

    #[cfg(feature = "persist_sled")]
    fn save_serializable_map<K, V>(db: &TransactionalTree, map: &FnvHashMap<K, V>) -> Result<(), GlobalStateInternalError>
    where
        K: Serialize,
        V: Serialize + Clone,
    {
        map.iter().try_for_each(|(id, value)| {
            db.insert(bincode::serialize(id)?, bincode::serialize(&value.clone())?)
                .map(|_| ())
                .map_err(GlobalStateInternalError::from)
        })
    }

    #[cfg(feature = "persist_sled")]
    fn save_order_states(&self, db: &TransactionalTree) -> Result<(), GlobalStateInternalError> {
        Self::save_serializable_map(db, &self.order_states)
    }

    #[cfg(feature = "persist_sled")]
    fn save_order_trees(&self, db: &TransactionalTree) -> Result<(), GlobalStateInternalError> {
        Self::save_serializable_map(db, &self.order_trees)
    }

    #[cfg(feature = "persist_sled")]
    fn save_balance_trees(&self, db: &TransactionalTree) -> Result<(), GlobalStateInternalError> {
        Self::save_serializable_map(db, &self.balance_trees)
    }

    #[cfg(feature = "persist_sled")]
    fn save_account_tree(&self, db: &TransactionalTree) -> Result<(), GlobalStateInternalError> {
        db.insert(ACCOUNTTREE_KEY, bincode::serialize(&*self.account_tree.clone())?)
            .map(|_| ())?;
        Ok(())
    }

    #[cfg(feature = "persist_sled")]
    fn save_next_order_positions(&self, db: &TransactionalTree) -> Result<(), GlobalStateInternalError> {
        Self::save_serializable_map(db, &self.next_order_positions)
    }
}
