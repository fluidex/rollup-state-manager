use ff::to_hex;
use ff::{PrimeField, PrimeFieldRepr};
use lazy_static::lazy_static;
use num_bigint::BigInt;

/*
// if use rescue
use franklin_crypto::rescue::rescue_hash;
use franklin_crypto::bellman::bn256::{Bn256};
use franklin_crypto::rescue::bn256::Bn256RescueParams;
lazy_static! {
    pub static ref RESCUE_PARAMS: Bn256RescueParams = Bn256RescueParams::new_checked_2_into_1();
}
pub type Fr = franklin_crypto::bellman::bn256::Fr;
pub fn hash(inputs: &[Fr]) -> Fr {
    rescue_hash::<Bn256>(&RESCUE_PARAMS, &inputs)[0]
}
*/

// if use poseidon
pub type Fr = poseidon_rs::Fr;
lazy_static! {
    //pub static ref POSEIDON_PARAMS: poseidon_rs::Constants = poseidon_rs::load_constants();
    pub static ref POSEIDON_HASHER: poseidon_rs::Poseidon = poseidon_rs::Poseidon::new();
}
pub fn hash(inputs: &[Fr]) -> Fr {
    (&POSEIDON_HASHER).hash(inputs.to_vec()).unwrap()
}
pub fn shl(a: &Fr, x: u32) -> Fr {
    let mut repr = a.into_repr();
    repr.shl(x);
    Fr::from_repr(repr).unwrap()
}
pub fn u32_to_fr(x: u32) -> Fr {
    Fr::from_str(&format!("{}", x)).unwrap()
}
pub fn u64_to_fr(x: u64) -> Fr {
    Fr::from_repr(poseidon_rs::FrRepr::from(x)).unwrap()
}
pub fn field_to_u32(x: &Fr) -> u32 {
    field_to_string(x).parse::<u32>().unwrap()
}
pub fn field_to_bigint(elem: &Fr) -> BigInt {
    BigInt::parse_bytes(to_hex(elem).as_bytes(), 16).unwrap()
}
pub fn field_to_string(elem: &Fr) -> String {
    field_to_bigint(&elem).to_str_radix(10)
}
