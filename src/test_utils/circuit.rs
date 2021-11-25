use regex::Regex;

use anyhow::Result;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Default, Clone)]
pub struct CircuitTestData {
    pub name: String,
    pub input: serde_json::Value,
    // circuits testdata may have not output
    pub output: Option<serde_json::Value>,
}

#[derive(Default, Clone)]
pub struct CircuitSource {
    pub src: String,
    pub main: String,
}

#[derive(Default, Clone)]
pub struct CircuitTestCase {
    pub source: CircuitSource,
    pub data: Vec<CircuitTestData>,
}

pub fn format_circuit_name(s: &str) -> String {
    // js: s.replace(/[ )]/g, '').replace(/[(,]/g, '_');
    let remove = Regex::new(r"[ )]").unwrap();
    let replace = Regex::new(r"[(,]").unwrap();
    replace.replace_all(&remove.replace_all(s, ""), "_").to_owned().to_string()
}

pub fn write_test_case(circuit_repo: &Path, test_dir: &Path, t: CircuitTestCase) -> anyhow::Result<PathBuf> {
    let circuit_dir = write_circuit(circuit_repo, test_dir, &t.source)?;
    for data in t.data {
        let test_data_dir = circuit_dir.join("data").join(data.name);
        fs::create_dir_all(test_data_dir.clone())?;
        let input_f = File::create(test_data_dir.join("input.json"))?;
        serde_json::to_writer_pretty(input_f, &data.input)?;

        if let Some(o) = data.output {
            let output_f = File::create(test_data_dir.join("output.json"))?;
            serde_json::to_writer_pretty(output_f, &o)?;
        }
    }
    Ok(circuit_dir)
}
fn write_circuit(circuit_repo: &Path, test_dir: &Path, source: &CircuitSource) -> Result<PathBuf> {
    let circuit_name = format_circuit_name(source.main.as_str());
    let circuit_dir = test_dir.join(circuit_name);
    fs::create_dir_all(circuit_dir.clone())?;
    let circuit_file = circuit_dir.join("circuit.circom");

    // on other OS than UNIX the slash in source wolud not be considerred as separator
    //so we need to convert them explicity
    let src_path: PathBuf = source.src.split('/').collect();

    let file_content = format!(
        "pragma circom 2.0.0;\ninclude \"{}\";\ncomponent main = {};",
        circuit_repo.join(src_path).to_str().unwrap(),
        source.main
    );
    let mut f = File::create(circuit_file)?;
    f.write_all(file_content.as_bytes())?;
    Ok(circuit_dir)
}
