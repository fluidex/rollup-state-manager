pub mod circuit;

pub use circuit::{format_circuit_name, CircuitSource, CircuitTestCase, CircuitTestData};

use ff::to_hex;
use num_bigint::BigInt;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::ser::SerializeSeq;
use serde::Serialize;
use std::convert::TryFrom;
pub use types::Fr;
// use regex::Regex;
use crate::state::{common, types};
// use ff::to_hex;
// use num_bigint::BigInt;
// use rust_decimal::prelude::ToPrimitive;
// use rust_decimal::Decimal;
// use serde::ser::SerializeSeq;
// use serde::Serialize;
// use std::convert::TryFrom;
pub use types::u32_to_fr;
pub use types::u64_to_fr;

pub fn field_to_string(elem: &Fr) -> String {
    BigInt::parse_bytes(to_hex(elem).as_bytes(), 16).unwrap().to_str_radix(10)
}

pub fn number_to_integer(num: &Decimal, prec: u32) -> Fr {
    let prec_mul = Decimal::new(10, 0).powi(prec as u64);
    let adjusted = num * prec_mul;
    types::u64_to_fr(adjusted.floor().to_u64().unwrap())
}

#[cfg(test)]
#[test]
fn test_number_to_integer() {
    let pi = Decimal::new(3141, 3);
    let out = number_to_integer(&pi, 3);
    assert_eq!(
        "Fr(0x0000000000000000000000000000000000000000000000000000000000000c45)",
        out.to_string()
    );
}

pub struct FrStr(Fr);

impl Serialize for FrStr {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(types::field_to_string(&self.0).as_str())
    }
}

impl From<Fr> for FrStr {
    fn from(origin: Fr) -> Self {
        FrStr(origin)
    }
}

pub struct MerkleLeafStr(FrStr);

impl Serialize for MerkleLeafStr {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(1))?;
        seq.serialize_element(&self.0)?;
        seq.end()
    }
}

//convert MerkleLeafType embedded in MerklePath
impl From<&[Fr; 1]> for MerkleLeafStr {
    fn from(origin: &[Fr; 1]) -> Self {
        MerkleLeafStr(FrStr(origin[0]))
    }
}

impl Serialize for common::TxType {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i32(match self {
            common::TxType::DepositToNew => 0,
            common::TxType::DepositToOld => 1,
            common::TxType::Transfer => 2,
            common::TxType::Withdraw => 3,
            common::TxType::PlaceOrder => 4,
            common::TxType::SpotTrade => 5,
            common::TxType::Nop => 6,
        })
    }
}

type MerklePathStr = Vec<MerkleLeafStr>;

//use derive could save many efforts for impling Serialize
//TODO: carmel style except for three "elements" field
#[derive(Serialize)]
pub struct L2BlockSerde {
    #[serde(rename(serialize = "txsType"))]
    txs_type: Vec<common::TxType>,
    #[serde(rename(serialize = "encodedTxs"))]
    encoded_txs: Vec<Vec<FrStr>>,
    balance_path_elements: Vec<[MerklePathStr; 4]>,
    order_path_elements: Vec<[MerklePathStr; 2]>,
    account_path_elements: Vec<[MerklePathStr; 2]>,
    #[serde(rename(serialize = "orderRoots"))]
    order_roots: Vec<[FrStr; 2]>,
    #[serde(rename(serialize = "oldAccountRoots"))]
    old_account_roots: Vec<FrStr>,
    #[serde(rename(serialize = "newAccountRoots"))]
    new_account_roots: Vec<FrStr>,
}

//array::map is not stable
fn array_map<U, T: Clone + Into<U>, const N: usize>(origin: [T; N]) -> [U; N] {
    let mut collector: Vec<U> = Vec::new();
    for i in &origin {
        collector.push(i.clone().into());
    }
    TryFrom::try_from(collector).ok().unwrap()
}

fn from_merkle<const N: usize>(origin: [common::MerklePath; N]) -> [MerklePathStr; N] {
    let mut collector: Vec<MerklePathStr> = Vec::new();
    for i in &origin {
        collector.push(i.iter().map(From::from).collect());
    }
    TryFrom::try_from(collector).ok().unwrap()
}

impl From<common::L2Block> for L2BlockSerde {
    fn from(origin: common::L2Block) -> Self {
        L2BlockSerde {
            txs_type: origin.txs_type,
            encoded_txs: origin
                .encoded_txs
                .into_iter()
                .map(|i| i.into_iter().map(From::from).collect())
                .collect(),
            balance_path_elements: origin.balance_path_elements.into_iter().map(from_merkle).collect(),
            order_path_elements: origin.order_path_elements.into_iter().map(from_merkle).collect(),
            account_path_elements: origin.account_path_elements.into_iter().map(from_merkle).collect(),
            order_roots: origin.order_roots.into_iter().map(array_map).collect(),
            old_account_roots: origin.old_account_roots.into_iter().map(From::from).collect(),
            new_account_roots: origin.new_account_roots.into_iter().map(From::from).collect(),
        }
    }
}
