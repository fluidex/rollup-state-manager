//use serde_json::Value;
use regex::Regex;

use crate::state::types::Fr;
use ff::to_hex;
use num_bigint::BigInt;

#[derive(Default, Clone)]
pub struct CircuitTestData {
    pub name: String,
    pub input: serde_json::Value,
    pub output: serde_json::Value,
}
#[derive(Default, Clone)]
pub struct CircuitSource {
    pub src: String,
    pub main: String,
}
#[derive(Default, Clone)]
pub struct CircuitTestCase {
    pub source: CircuitSource,
    pub data: CircuitTestData,
}

pub fn format_circuit_name(s: &str) -> String {
    // js: s.replace(/[ )]/g, '').replace(/[(,]/g, '_');
    let remove = Regex::new(r"[ )]").unwrap();
    let replace = Regex::new(r"[(,]").unwrap();
    replace.replace_all(&remove.replace_all(s, ""), "_").to_owned().to_string()
}
pub fn field_to_string(elem: &Fr) -> String {
    BigInt::parse_bytes(to_hex(elem).as_bytes(), 16).unwrap().to_str_radix(10)
}
