// from https://github1s.com/Fluidex/circuits/blob/HEAD/test/binary_merkle_tree.ts
use super::types::*;
use crate::state::merkle_tree::Tree;
use serde_json::json;

use crate::state::types::Fr;
pub use ff::{Field, PrimeField};

pub fn test_check_leaf_update() -> CircuitTestCase {
    let leaves: Vec<Fr> = vec![10, 11, 12, 13]
        .iter()
        .map(|x| Fr::from_str(&format!("{}", x)).unwrap())
        .collect();
    let mut tree = Tree::new(2, Fr::zero());
    tree.fill_with_leaves_vec(&leaves);
    let proof1 = tree.get_proof(2);
    tree.set_value(2, Fr::from_str("19").unwrap());
    let proof2 = tree.get_proof(2);
    // TODO: we need a path index function?
    //
    let field_slice_to_string = |arr: &[Fr]| arr.iter().map(field_to_string).collect::<Vec<String>>();
    let input = json!({
        "enabled": 1,
        "oldLeaf": field_to_string(&proof1.leaf),
        "oldRoot": field_to_string(&proof1.root),
        "newLeaf": field_to_string(&proof2.leaf),
        "newRoot": field_to_string(&proof2.root),
        "path_elements": proof1.path_elements.iter().map(|x| field_slice_to_string(x)).collect::<Vec<_>>(),
        "path_index": [0, 1],
    });
    CircuitTestCase {
        source: CircuitSource {
            src: "src/lib/binary_merkle_tree.circom".to_owned(),
            main: "CheckLeafUpdate(2)".to_owned(),
        },
        data: CircuitTestData {
            name: "test_check_leaf_update".to_owned(),
            input,
            output: json!({}),
        },
    }
}
