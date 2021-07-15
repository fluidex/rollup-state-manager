pub use crate::types::l2;
pub use crate::types::merkle_tree::MerklePath;
use fluidex_common::Fr;
use serde::ser::SerializeSeq;
use serde::Serialize;
use std::convert::TryFrom;

pub struct FrStr(Fr);

impl Serialize for FrStr {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.0.to_string().as_str())
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

impl Serialize for l2::TxType {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i32(match self {
            l2::TxType::Nop => 0,
            l2::TxType::Deposit => 1,
            l2::TxType::Transfer => 2,
            l2::TxType::Withdraw => 3,
            l2::TxType::PlaceOrder => 4,
            l2::TxType::SpotTrade => 5,
        })
    }
}

type MerklePathStr = Vec<MerkleLeafStr>;

//use derive could save many efforts for impling Serialize
//TODO: carmel style except for three "elements" field
#[derive(Serialize)]
pub struct L2BlockSerde {
    #[serde(rename(serialize = "oldRoot"))]
    old_root: FrStr,
    #[serde(rename(serialize = "newRoot"))]
    new_root: FrStr,
    #[serde(rename(serialize = "txsType"))]
    txs_type: Vec<l2::TxType>,
    #[serde(rename(serialize = "encodedTxs"))]
    encoded_txs: Vec<Vec<FrStr>>,
    #[serde(rename(serialize = "balancePathElements"))]
    balance_path_elements: Vec<[MerklePathStr; 4]>,
    #[serde(rename(serialize = "orderPathElements"))]
    order_path_elements: Vec<[MerklePathStr; 2]>,
    #[serde(rename(serialize = "accountPathElements"))]
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

fn from_merkle<const N: usize>(origin: [MerklePath; N]) -> [MerklePathStr; N] {
    let mut collector: Vec<MerklePathStr> = Vec::new();
    for i in &origin {
        collector.push(i.iter().map(From::from).collect());
    }
    TryFrom::try_from(collector).ok().unwrap()
}

impl From<l2::L2BlockWitness> for L2BlockSerde {
    fn from(origin: l2::L2BlockWitness) -> Self {
        L2BlockSerde {
            old_root: origin.old_root.into(),
            new_root: origin.new_root.into(),
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
