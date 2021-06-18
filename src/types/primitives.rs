use anyhow::{anyhow, Result};
use ff::{from_hex, to_hex};
use ff::{Field, PrimeField, PrimeFieldRepr};
use lazy_static::lazy_static;
use num_bigint::BigInt;
use rust_decimal::Decimal;
//use std::str::FromStr;

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

pub fn fr_sub(a: &Fr, b: &Fr) -> Fr {
    let mut r = *a;
    r.sub_assign(b);
    r
}

pub fn fr_add(a: &Fr, b: &Fr) -> Fr {
    let mut r = *a;
    r.add_assign(b);
    r
}

// TODO: these functions needed to be rewrite...

pub fn u32_to_fr(x: u32) -> Fr {
    Fr::from_str(&format!("{}", x)).unwrap()
}
pub fn u64_to_fr(x: u64) -> Fr {
    Fr::from_repr(poseidon_rs::FrRepr::from(x)).unwrap()
}
pub fn bigint_to_fr(x: BigInt) -> Fr {
    let mut s = x.to_str_radix(16);
    if s.len() % 2 != 0 {
        // convert "f" to "0f"
        s.insert(0, '0');
    }
    from_hex(&s).unwrap()
}
pub fn vec_to_fr(arr: &[u8]) -> Result<Fr> {
    if arr.len() > 32 {
        anyhow::bail!("invalid vec len for fr");
    }
    let mut repr = <Fr as PrimeField>::Repr::default();

    // prepad 0
    let mut buf = arr.to_vec();
    let required_length = repr.as_ref().len() * 8;
    buf.reverse();
    buf.resize(required_length, 0);
    buf.reverse();

    repr.read_be(&buf[..])?;
    Ok(Fr::from_repr(repr)?)
}

pub fn fr_to_u32(f: &Fr) -> u32 {
    fr_to_string(f).parse::<u32>().unwrap()
}
pub fn fr_to_i64(f: &Fr) -> i64 {
    fr_to_string(f).parse::<i64>().unwrap()
}
pub fn fr_to_bigint(elem: &Fr) -> BigInt {
    BigInt::parse_bytes(to_hex(elem).as_bytes(), 16).unwrap()
}
pub fn fr_to_string(elem: &Fr) -> String {
    fr_to_bigint(&elem).to_str_radix(10)
}
pub fn fr_to_decimal(f: &Fr, scale: u32) -> Decimal {
    Decimal::new(fr_to_i64(f), scale)
}
// big endian
pub fn fr_to_vec(f: &Fr) -> Vec<u8> {
    let repr = f.into_repr();
    let required_length = repr.as_ref().len() * 8;
    let mut buf: Vec<u8> = Vec::with_capacity(required_length);
    repr.write_be(&mut buf).unwrap();
    buf
}
pub fn fr_to_bool(f: &Fr) -> Result<bool> {
    if f.is_zero() {
        Ok(false)
    } else if f == &Fr::one() {
        Ok(true)
    } else {
        Err(anyhow!("invalid fr"))
    }
}

pub mod fr_bytes {

    use super::*;
    use serde::{de, ser};

    #[doc(hidden)]
    #[derive(Debug)]
    pub struct FrBytesVisitor;

    pub fn serialize<S>(fr: &Fr, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_bytes(&fr_to_vec(fr))
    }

    pub fn deserialize<'de, D>(d: D) -> Result<Fr, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        Ok(d.deserialize_bytes(FrBytesVisitor)?)
    }

    impl<'de> de::Visitor<'de> for FrBytesVisitor {
        type Value = Fr;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a Fr in be bytes repr")
        }

        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if let Ok(fr) = vec_to_fr(v) {
                Ok(fr)
            } else {
                Err(de::Error::invalid_type(de::Unexpected::Bytes(v), &self))
            }
        }
    }
}

pub mod fr_map {

    use std::hash::Hash;

    use super::*;
    use fnv::FnvHashMap as MerkleValueMapType;
    use serde::{de, ser, Deserialize, Serialize};

    pub fn serialize<S, K>(fr: &MerkleValueMapType<K, Fr>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
        K: Eq + Hash + Serialize,
    {
        #[derive(Serialize)]
        struct Wrapper<'a>(#[serde(with = "fr_bytes")] &'a Fr);

        let map = fr.iter().map(|(k, v)| (k, Wrapper(v)));
        serializer.collect_map(map)
    }

    pub fn deserialize<'de, D, K>(deserializer: D) -> Result<MerkleValueMapType<K, Fr>, D::Error>
    where
        D: de::Deserializer<'de>,
        K: Eq + Hash + de::Deserialize<'de>,
    {
        #[derive(Deserialize)]
        struct Wrapper(#[serde(with = "fr_bytes")] Fr);

        let map = MerkleValueMapType::<K, Wrapper>::deserialize(deserializer)?;
        Ok(map.into_iter().map(|(k, Wrapper(v))| (k, v)).collect())
    }
}
