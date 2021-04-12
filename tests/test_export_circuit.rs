// use state_keeper::circuit_test;
use state_keeper::test_utils::{CircuitSource, CircuitTestCase, CircuitTestData};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
// use anyhow::anyhow;

/*
 * cargo run --bin export_circuit_test
 * npm -g install snarkit
 * npx snarkit test ../circuits/testdata/CheckLeafUpdate_2/
 */

// from https://github1s.com/Fluidex/circuits/blob/HEAD/test/binary_merkle_tree.ts
mod test_case {
    use state_keeper::test_utils::{CircuitSource, CircuitTestCase, CircuitTestData};

    pub fn check_leaf_update() -> CircuitTestCase {
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
}

fn write_test_case(circuit_repo: &Path, test_dir: &Path, t: CircuitTestCase) -> anyhow::Result<()> {
    //let mut t = t.clone();
    let circuit_name = circuit_test::types::format_circuit_name(&t.source.main);
    let circuit_dir = test_dir.join(circuit_name);
    fs::create_dir_all(circuit_dir.clone())?;
    let circuit_file = circuit_dir.join("circuit.circom");
    let file_content = format!(
        "include \"{}\";\ncomponent main = {}",
        circuit_repo.join(t.source.src).to_str().unwrap(),
        t.source.main
    );
    let mut f = File::create(circuit_file)?;
    f.write_all(&file_content.as_bytes())?;
    let test_data_dir = circuit_dir.join("data").join(t.data.name);
    fs::create_dir_all(test_data_dir.clone())?;
    let input_f = File::create(test_data_dir.join("input.json"))?;
    serde_json::to_writer_pretty(input_f, &t.data.input)?;
    let output_f = File::create(test_data_dir.join("output.json"))?;
    serde_json::to_writer_pretty(output_f, &t.data.output)?;
    Ok(())
}

fn run() -> anyhow::Result<()> {
    let circuit_repo = fs::canonicalize(PathBuf::from("../circuits")).expect("invalid circuits repo path");
    let test_dir = circuit_repo.join("testdata");
    write_test_case(&circuit_repo, &test_dir, test_case::check_leaf_update())
}

fn main() {
    match run() {
        Ok(_) => println!("test_export_circuit pass"),
        Err(e) => panic!("{:#?}", e),
    }
}
