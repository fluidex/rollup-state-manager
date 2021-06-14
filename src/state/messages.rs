use std::sync::{Arc, Mutex};

use crate::types::merkle_tree::Tree;
use crate::types::messages::TreeMessage;
use crate::types::primitives::fr_to_string;
use serde::{Deserialize, Serialize};

use super::AccountState;

#[derive(Serialize, Deserialize)]
pub struct OrderTreeMessage {
    pub account_id: u32,
    pub order_tree: TreeMessage,
}

impl From<(&u32, &Arc<Mutex<Tree>>)> for OrderTreeMessage {
    fn from(tree: (&u32, &Arc<Mutex<Tree>>)) -> Self {
        Self {
            account_id: *tree.0,
            order_tree: TreeMessage::from(tree.1.lock().unwrap()),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct BalanceTreeMessage {
    pub account_id: u32,
    pub balance_tree: TreeMessage,
}

impl From<(&u32, &Arc<Mutex<Tree>>)> for BalanceTreeMessage {
    fn from(tree: (&u32, &Arc<Mutex<Tree>>)) -> Self {
        Self {
            account_id: *tree.0,
            balance_tree: TreeMessage::from(tree.1.lock().unwrap()),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct AccountStateMessage {
    pub account_id: u32,
    pub nonce: String,
    pub sign: String,
    pub balance_root: String,
    pub ay: String,
    pub eth_addr: String,
    pub order_root: String,
}

impl From<(&u32, &AccountState)> for AccountStateMessage {
    fn from(tree: (&u32, &AccountState)) -> Self {
        Self {
            account_id: *tree.0,
            nonce: fr_to_string(&tree.1.nonce),
            sign: fr_to_string(&tree.1.sign),
            balance_root: fr_to_string(&tree.1.balance_root),
            ay: fr_to_string(&tree.1.ay),
            eth_addr: fr_to_string(&tree.1.eth_addr),
            order_root: fr_to_string(&tree.1.order_root),
        }
    }
}
