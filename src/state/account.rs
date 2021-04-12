use crate::types::primitives::{field_to_bigint, Fr};
use anyhow::Result;
use arrayref::array_ref;
use babyjubjub_rs::{decompress_point, Point, PrivateKey};
use num_bigint::BigInt;

#[derive(Debug, Clone)]
pub struct Signature {
    pub r_b8: Point,
    pub s: BigInt,
}

pub fn decompress_signature(b: &[u8; 64]) -> Result<Signature, String> {
    let r_b8_bytes: [u8; 32] = *array_ref!(b[..32], 0, 32);
    let s: BigInt = BigInt::from_bytes_le(num_bigint::Sign::Plus, &b[32..]);
    let r_b8 = decompress_point(r_b8_bytes);
    match r_b8 {
        Result::Err(err) => Err(err),
        Result::Ok(res) => Ok(Signature { r_b8: res, s }),
    }
}

pub struct L2Account {
    priv_key: PrivateKey,
    pub pub_key: Point,
    pub ax: Fr,
    pub ay: Fr,
    pub sign: bool,
    pub bjj_compressed: [u8; 32],
}

impl L2Account {
    pub fn new(seed: Vec<u8>) -> Result<Self, String> {
        let priv_key = PrivateKey::import(seed)?;
        let pub_key: Point = priv_key.public();
        let ax = pub_key.x;
        let ay = pub_key.y;
        let bjj_compressed = pub_key.compress();
        // pub_key.x < 0
        let sign = bjj_compressed[31] & 0x80 != 0x00;
        Ok(Self {
            priv_key,
            pub_key,
            ax,
            ay,
            sign,
            bjj_compressed,
        })
    }
    pub fn sign_hash(&self, h: &Fr) -> Result<Signature, String> {
        let h_b = field_to_bigint(h);
        decompress_signature(&self.priv_key.sign(h_b)?.compress())
    }
}

#[cfg(test)]
mod tests {
    use crate::types::primitives::field_to_string;
    use super::*;
    use ff::PrimeField;

    #[test]
    fn test_account() {
        // https://github.com/Fluidex/circuits/blob/46e7ee0bc69a49c981ccccbb9003900f78eb3d59/helper.ts/account_test.ts#L25
        let seed = hex::decode("87b34b2b842db0cc945659366068053f325ff227fd9c6788b2504ac2c4c5dc2a").unwrap();
        let acc: L2Account = L2Account::new(seed).unwrap();
        let priv_bigint = acc.priv_key.scalar_key().to_string();
        assert_eq!(
            priv_bigint,
            "4168145781671332788401281374517684700242591274637494106675223138867941841158"
        );
        assert_eq!(
            acc.ax.to_string(),
            "Fr(0x1fce25ec2e7eeec94079ec7866a933a8b21f33e0ebd575f3001d62d19251d455)"
        );
        assert_eq!(
            acc.ay.to_string(),
            "Fr(0x20a41ccb24e55dba4fc9ebc17ae9d4c9097d7fe3387d492155568db6be2692a5)"
        );
        assert_eq!(acc.sign, true);
        // TODO: which encoding is more reasonable?
        let mut bjj_compressed = acc.bjj_compressed;
        bjj_compressed.reverse();
        assert_eq!(
            hex::encode(bjj_compressed),
            "a0a41ccb24e55dba4fc9ebc17ae9d4c9097d7fe3387d492155568db6be2692a5"
        );
        let sig = acc.sign_hash(&Fr::from_str("1357924680").unwrap()).unwrap();
        assert_eq!(
            field_to_string(&sig.r_b8.x),
            "15679698175365968671287592821268512384454163537665670071564984871581219397966"
        );
        assert_eq!(
            field_to_string(&sig.r_b8.y),
            "1705544521394286010135369499330220710333064238375605681220284175409544486013"
        );
        assert_eq!(
            sig.s.to_string(),
            "2104729104368328243963691045555606467740179640947024714099030450797354625308"
        );
    }
}
