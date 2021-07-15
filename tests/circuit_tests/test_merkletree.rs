use fluidex_common::ff::Field;
use fluidex_common::{types::FrExt, Fr};
use rollup_state_manager::test_utils::circuit::{CircuitSource, CircuitTestCase, CircuitTestData};
use rollup_state_manager::types::merkle_tree::Tree;
use serde_json::json;

pub fn get_merkle_tree_test_case() -> CircuitTestCase {
    let leaves: Vec<Fr> = vec![10, 11, 12, 13].iter().map(|x| FrExt::from_str(&format!("{}", x))).collect();
    let mut tree = Tree::new(2, Fr::zero());
    tree.fill_with_leaves_vec(&leaves);
    let proof1 = tree.get_proof(2);
    tree.set_value(2, FrExt::from_str("19"));
    let proof2 = tree.get_proof(2);
    // TODO: we need a path index function?
    //
    let field_slice_to_string = |arr: &[Fr]| arr.iter().map(Fr::to_string).collect::<Vec<String>>();
    let input = json!({
        "enabled": 1,
        "oldLeaf": proof1.leaf.to_string(),
        "oldRoot": proof1.root.to_string(),
        "newLeaf": proof2.leaf.to_string(),
        "newRoot": proof2.root.to_string(),
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
