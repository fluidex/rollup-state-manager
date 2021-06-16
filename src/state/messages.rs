use std::sync::{Arc, Mutex};

use crate::types::merkle_tree::Tree;
use crate::types::messages::TreeMessage;
use serde::{Deserialize, Serialize};

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
