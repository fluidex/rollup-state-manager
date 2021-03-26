use state_keeper::circuit_test;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

/*
 * cargo run --bin export_circuit_test
 * npm -g install https://github.com/Fluidex/circom-circuit-tester
 * npx snarkit test ../circuits/testdata/CheckLeafUpdate_2/
 */

fn write_test_case(circuit_repo: &Path, test_dir: &Path, t: circuit_test::types::CircuitTestCase) -> anyhow::Result<()> {
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
fn test_all() -> anyhow::Result<()> {
    let circuit_repo = fs::canonicalize(PathBuf::from("../circuits")).expect("invalid circuits repo path");
    let test_dir = circuit_repo.join("testdata");
    write_test_case(&circuit_repo, &test_dir, circuit_test::binary_merkle_tree::test_check_leaf_update())?;
    Ok(())
}
fn main() {
    match test_all() {
        Ok(_) => {}
        Err(e) => {
            eprintln!("{:#?}", e);
            std::process::exit(1);
        }
    }
}
