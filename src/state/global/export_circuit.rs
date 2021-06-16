// TODO: move this file in main repo rather than test folder

use crate::test_utils;
use crate::test_utils::L2BlockSerde;
use crate::types::l2;
use anyhow::Result;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

//just grap from export_circuit_test.rs ...
fn write_circuit(circuit_repo: &Path, test_dir: &Path, source: &test_utils::CircuitSource) -> Result<PathBuf> {
    let circuit_name = test_utils::format_circuit_name(source.main.as_str());
    let circuit_dir = test_dir.join(circuit_name);

    fs::create_dir_all(circuit_dir.clone())?;

    let circuit_file = circuit_dir.join("circuit.circom");

    // on other OS than UNIX the slash in source wolud not be considerred as separator
    //so we need to convert them explicity
    let src_path: PathBuf = source.src.split('/').collect();

    let file_content = format!(
        "include \"{}\";\ncomponent main = {}",
        circuit_repo.join(src_path).to_str().unwrap(),
        source.main
    );
    let mut f = File::create(circuit_file)?;
    f.write_all(&file_content.as_bytes())?;
    Ok(circuit_dir)
}

fn write_input_output(dir: &Path, block: l2::L2Block) -> Result<()> {
    fs::create_dir_all(dir)?;

    let input_f = File::create(dir.join("input.json"))?;
    serde_json::to_writer_pretty(input_f, &L2BlockSerde::from(block))?;

    let output_f = File::create(dir.join("output.json"))?;
    //TODO: no output?
    serde_json::to_writer_pretty(output_f, &serde_json::Value::Object(Default::default()))?;

    Ok(())
}

pub fn export_circuit_and_testdata(circuit_repo: &Path, blocks: Vec<l2::L2Block>, source: test_utils::CircuitSource) -> Result<PathBuf> {
    let test_dir = circuit_repo.join("testdata");
    let circuit_dir = write_circuit(circuit_repo, &test_dir, &source)?;

    for (blki, blk) in blocks.iter().enumerate() {
        let dir = circuit_dir.join(format!("{:04}", blki));
        write_input_output(&dir, blk.clone())?;
        //println!("{}", serde_json::to_string_pretty(&types::L2BlockSerde::from(blk)).unwrap());
    }

    println!("write {} test cases to {:?}", blocks.len(), circuit_dir.to_str());

    Ok(circuit_dir)
}
