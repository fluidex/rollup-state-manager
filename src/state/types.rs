use lazy_static::lazy_static;

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
