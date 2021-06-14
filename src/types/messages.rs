use std::collections::HashMap;
use std::sync::MutexGuard;

use crate::types::merkle_tree::Tree;
use crate::types::primitives::fr_to_string;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct TreeMessage {
    pub height: usize,
    // Only preserve exist nodes (without default nodes) in the message.
    pub data: HashMap<String, String>,
}

impl From<Tree> for TreeMessage {
    fn from(tree: Tree) -> Self {
        let mut data = HashMap::new();
        for (idx, node) in tree.get_tree_data() {
            data.insert(idx.to_string(), fr_to_string(&node));
        }
        Self { height: tree.height, data }
    }
}

impl From<MutexGuard<'_, Tree>> for TreeMessage {
    fn from(tree: MutexGuard<Tree>) -> Self {
        let mut data = HashMap::new();
        for (idx, node) in tree.get_tree_data() {
            data.insert(idx.to_string(), fr_to_string(&node));
        }
        Self { height: tree.height, data }
    }
}
