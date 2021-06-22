use ff::{Field, PrimeField};
use rollup_state_manager::test_utils::circuit::{CircuitSource, CircuitTestCase, CircuitTestData};
use rollup_state_manager::types::merkle_tree::Tree;
use rollup_state_manager::types::primitives::{fr_to_string, Fr};
use serde_json::json;

pub fn get_merkle_tree_test_case() -> CircuitTestCase {
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
    let field_slice_to_string = |arr: &[Fr]| arr.iter().map(fr_to_string).collect::<Vec<String>>();
    let input = json!({
        "enabled": 1,
        "oldLeaf": fr_to_string(&proof1.leaf),
        "oldRoot": fr_to_string(&proof1.root),
        "newLeaf": fr_to_string(&proof2.leaf),
        "newRoot": fr_to_string(&proof2.root),
        "pathElements": proof1.path_elements.iter().map(|x| field_slice_to_string(x)).collect::<Vec<_>>(),
        "pathIndex": [0, 1],
    });
    CircuitTestCase {
        source: CircuitSource {
            src: "src/lib/binary_merkle_tree.circom".to_owned(),
            main: "CheckLeafUpdate(2)".to_owned(),
        },
        data: vec![CircuitTestData {
            name: "test_check_leaf_update".to_owned(),
            input,
            output: None,
        }],
    }
}
