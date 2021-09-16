pub use crate::types::l2;
pub use crate::types::merkle_tree::MerklePath;
use fluidex_common::num_bigint::BigInt;
use fluidex_common::types::FrExt;
use fluidex_common::Fr;
use serde::ser::SerializeSeq;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::convert::TryFrom;
use std::str::FromStr;

pub struct FrStr(pub Fr);

// TODO: May use or integrate serializers and deserializers with `https://github.com/fluidex/common-rs/blob/master/src/serde.rs`.

impl Serialize for FrStr {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.0.to_decimal_string().as_str())
    }
}

impl<'de> Deserialize<'de> for FrStr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct FrStrVisitor;

        impl<'de> de::Visitor<'de> for FrStrVisitor {
            type Value = Fr;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a Fr in decimal str repr")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if let Ok(fr) = BigInt::from_str(v) {
                    Ok(Fr::from_bigint(fr))
                } else {
                    Err(de::Error::invalid_type(de::Unexpected::Str(v), &self))
                }
            }
        }

        Ok(Self(deserializer.deserialize_str(FrStrVisitor)?))
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

impl<'de> Deserialize<'de> for MerkleLeafStr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MerkleLeafStrVisitor;

        impl<'de> de::Visitor<'de> for MerkleLeafStrVisitor {
            type Value = MerkleLeafStr;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a Merkle Leaf str repr")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
            where
                V: de::SeqAccess<'de>,
            {
                let fr_str = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(0, &self))?;
                Ok(MerkleLeafStr(fr_str))
            }
        }

        deserializer.deserialize_seq(MerkleLeafStrVisitor)
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

impl<'de> Deserialize<'de> for l2::TxType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct L2TxTypeVisitor;

        impl<'de> de::Visitor<'de> for L2TxTypeVisitor {
            type Value = l2::TxType;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a L2 TX type repr")
            }

            fn visit_i8<E>(self, v: i8) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_i64(i64::from(v))
            }

            fn visit_i16<E>(self, v: i16) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_i64(i64::from(v))
            }

            fn visit_i32<E>(self, v: i32) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_i64(i64::from(v))
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let tx_type = match v {
                    0 => l2::TxType::Nop,
                    1 => l2::TxType::Deposit,
                    2 => l2::TxType::Transfer,
                    3 => l2::TxType::Withdraw,
                    4 => l2::TxType::PlaceOrder,
                    5 => l2::TxType::SpotTrade,
                    _ => return Err(de::Error::invalid_type(de::Unexpected::Signed(v), &self)),
                };
                Ok(tx_type)
            }

            fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_i64(i64::from(v))
            }

            fn visit_u16<E>(self, v: u16) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_i64(i64::from(v))
            }

            fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_i64(i64::from(v))
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let tx_type = match v {
                    0 => l2::TxType::Nop,
                    1 => l2::TxType::Deposit,
                    2 => l2::TxType::Transfer,
                    3 => l2::TxType::Withdraw,
                    4 => l2::TxType::PlaceOrder,
                    5 => l2::TxType::SpotTrade,
                    _ => return Err(de::Error::invalid_type(de::Unexpected::Unsigned(v), &self)),
                };
                Ok(tx_type)
            }
        }

        deserializer.deserialize_i32(L2TxTypeVisitor)
    }
}

type MerklePathStr = Vec<MerkleLeafStr>;

#[derive(Serialize, Deserialize)]
pub struct L2BlockSerde {
    #[serde(rename = "oldRoot")]
    pub old_root: FrStr,
    #[serde(rename = "newRoot")]
    pub new_root: FrStr,
    #[serde(rename = "txDataHashHi")]
    pub txdata_hash_hi: u128,
    #[serde(rename = "txDataHashLo")]
    pub txdata_hash_lo: u128,
    #[serde(rename = "txsType")]
    pub txs_type: Vec<l2::TxType>,
    #[serde(rename = "encodedTxs")]
    pub encoded_txs: Vec<Vec<FrStr>>,
    #[serde(rename = "balancePathElements")]
    pub balance_path_elements: Vec<[MerklePathStr; 4]>,
    #[serde(rename = "orderPathElements")]
    pub order_path_elements: Vec<[MerklePathStr; 2]>,
    #[serde(rename = "accountPathElements")]
    pub account_path_elements: Vec<[MerklePathStr; 2]>,
    #[serde(rename = "orderRoots")]
    pub order_roots: Vec<[FrStr; 2]>,
    #[serde(rename = "oldAccountRoots")]
    pub old_account_roots: Vec<FrStr>,
    #[serde(rename = "newAccountRoots")]
    pub new_account_roots: Vec<FrStr>,
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

impl From<l2::L2BlockDetail> for L2BlockSerde {
    fn from(origin: l2::L2BlockDetail) -> Self {
        L2BlockSerde {
            old_root: origin.old_root.into(),
            new_root: origin.new_root.into(),
            txdata_hash_lo: origin.txdata_hash.low_u128(),
            txdata_hash_hi: (origin.txdata_hash >> 128u8).low_u128(),
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
