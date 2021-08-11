#![allow(clippy::let_and_return)]
use crate::account::random_mnemonic_with_rng;
use ethers::core::rand::SeedableRng;
use ethers::prelude::coins_bip39::{English, Mnemonic};
use std::str::FromStr;

use coins_bip32::{path::DerivationPath, prelude::SigningKey};
use ethers::utils::{secret_key_to_address, to_checksum};
use serde::Serialize;
// TODO: Moves other test types to here.

// shoule be consistent with dingir-exchange/migrations/20210223072038_markets_preset.sql
// TODO: enum & impl
pub fn get_token_id_by_name(token_name: &str) -> u32 {
    match token_name {
        "ETH" => 0,
        "USDT" => 1,
        "UNI" => 2,
        "LINK" => 3,
        "YFI" => 4,
        "MATIC" => 5,
        _ => unreachable!(),
    }
}

// TODO: enum & impl
pub fn prec_token_id(token_id: u32) -> u32 {
    match token_id {
        0 | 2 | 3 | 4 | 5 => 4,
        1 => 4 + 2, // only USDT can be quote, quote prec = price prec + amount prec
        _ => unreachable!(),
    }
}

pub fn get_mnemonic_by_account_id(account_id: u32) -> Mnemonic<English> {
    let mut r = ethers::core::rand::rngs::StdRng::seed_from_u64(account_id as u64);
    let mnemonic = random_mnemonic_with_rng(&mut r);
    //println!("mnemonic for account {} is {}", account_id, mnemonic.to_phrase().unwrap());
    mnemonic
}

#[derive(Serialize)]
pub struct MockAccount {
    account_id: u32,
    mnemonic: String,
    priv_key: String,
    eth_addr: String,
}

pub fn get_mock_user_by_account_id(account_id: u32) -> MockAccount {
    let mnemonic = get_mnemonic_by_account_id(account_id);
    let mnemonic_str = mnemonic.to_phrase().unwrap();
    let path = DerivationPath::from_str("m/44'/60'/0'/0/0").unwrap();
    let priv_key = mnemonic.derive_key(path, None).unwrap();
    let priv_key_ref: &SigningKey = priv_key.as_ref();
    let priv_key_bytes = priv_key_ref.to_bytes();
    let eth_addr = secret_key_to_address(priv_key_ref);

    let acc = MockAccount {
        mnemonic: mnemonic_str,
        account_id,
        priv_key: format!("0x{:x}", priv_key_bytes),
        eth_addr: to_checksum(&eth_addr, None),
    };
    acc
}

#[cfg(test)]
#[test]
fn print_test_accounts() {
    let num = 20;
    for account_id in 0..num {
        let acc = get_mock_user_by_account_id(account_id);
        println!("{}", serde_json::to_string(&acc).unwrap());
    }
}
