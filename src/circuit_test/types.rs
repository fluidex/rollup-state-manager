//use serde_json::Value;
use regex::Regex;

use crate::state::types;
pub use types::Fr;
use ff::to_hex;
use num_bigint::BigInt;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

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

pub fn number_to_integer(num: &Decimal, prec: u32) -> Fr {
    let prec_mul = Decimal::new(10, 0).powi(prec as u64);
    let adjusted = num * prec_mul;
    types::u64_to_fr(adjusted.floor().to_u64().unwrap())
}

pub use types::u32_to_fr;
pub use types::u64_to_fr;


#[cfg(test)]
#[test]
fn test_number_to_integer() {

    let pi = Decimal::new(3141, 3);
    let out = number_to_integer(&pi, 3);
    assert_eq!("Fr(0x0000000000000000000000000000000000000000000000000000000000000c45)", out.to_string());

}