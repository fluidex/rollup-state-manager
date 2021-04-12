//use serde_json::Value;
use crate::state::{common, types};
use ff::to_hex;
use num_bigint::BigInt;
use regex::Regex;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::ser::SerializeSeq;
use serde::Serialize;
use std::convert::TryFrom;
pub use types::Fr;

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
