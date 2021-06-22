#![allow(clippy::let_and_return)]
use crate::account::random_mnemonic_with_rng;
use ethers::core::rand::SeedableRng;
use ethers::prelude::coins_bip39::{English, Mnemonic};
// TODO: Moves other test types to here.

// TODO: enum & impl
pub fn get_token_id_by_name(token_name: &str) -> u32 {
    match token_name {
        "ETH" => 0,
        "USDT" => 1,
        _ => unreachable!(),
    }
}

// TODO: enum & impl
// shoule be consistent with dingir-exchange/migrations/20210223072038_markets_preset.sql
pub fn prec_token_id(token_id: u32) -> u32 {
    match token_id {
        0 => 4,
        1 => 4 + 2, // quote prec = price prec + amount prec
        _ => unreachable!(),
    }
}

pub fn get_mnemonic_by_account_id(account_id: u32) -> Mnemonic<English> {
    let mut r = ethers::core::rand::rngs::StdRng::seed_from_u64(account_id as u64);
    let mnemonic = random_mnemonic_with_rng(&mut r);
    //println!("mnemonic for account {} is {}", account_id, mnemonic.to_phrase().unwrap());
    mnemonic
}
