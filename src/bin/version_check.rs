#![allow(dead_code)]
use std::env;

//this utility check if the circuit submodule has same version as which is specified to used in enviroment
fn main() {
    dotenv::dotenv().ok();
    let circuit_ver = include_str!("../../circuits/circuits.ver");
    log::info!("circuits version is {}", circuit_ver);

    if let Ok(ver) = env::var("CIRCUIT_VER"){
        assert_eq!(ver, circuit_ver, "circuit ver ({}) is not consistent with the ver current used: {}", circuit_ver, ver);
    }
}