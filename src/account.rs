use crate::types::primitives::{bigint_to_fr, fr_to_bigint, u32_to_fr, Fr};
use anyhow::Result;
use arrayref::array_ref;
use babyjubjub_rs::{decompress_point, Point, PrivateKey};
use ff::Field;
use num_bigint::BigInt;
use rand::Rng;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Signature {
    pub hash: Fr,
    pub s: Fr,
    pub r8x: Fr,
    pub r8y: Fr,
}

impl Default for Signature {
    fn default() -> Self {
        Self {
            hash: Fr::zero(),
            s: Fr::zero(),
            r8x: Fr::zero(),
            r8y: Fr::zero(),
        }
    }
}

pub struct L2Account {
    priv_key: PrivateKey,
    pub pub_key: Point,
    pub ax: Fr,
    pub ay: Fr,
    pub sign: Fr,
    pub bjj_pub_key: String,
}

impl L2Account {
    pub fn new(seed: Vec<u8>) -> Result<Self, String> {
        let priv_key = PrivateKey::import(seed)?;
        let pub_key: Point = priv_key.public();
        let ax = pub_key.x;
        let ay = pub_key.y;
        let bjj_compressed = pub_key.compress();
        let sign = if bjj_compressed[31] & 0x80 != 0x00 {
            u32_to_fr(1)
        } else {
            Fr::zero()
        };
        let bjj_pub_key = hex::encode(bjj_compressed);
        Ok(Self {
            priv_key,
            pub_key,
            ax,
            ay,
            sign,
            bjj_pub_key,
        })
    }

    pub fn sign_hash(&self, hash: Fr) -> Result<Signature, String> {
        let b = self.priv_key.sign(fr_to_bigint(&hash))?.compress();
        let r_b8_bytes: [u8; 32] = *array_ref!(b[..32], 0, 32);
        let s = bigint_to_fr(BigInt::from_bytes_le(num_bigint::Sign::Plus, &b[32..]));
        let r_b8 = decompress_point(r_b8_bytes);
        match r_b8 {
            Result::Err(err) => Err(err),
            Result::Ok(Point { x: r8x, y: r8y }) => Ok(Signature { hash, s, r8x, r8y }),
        }
    }
}

pub struct Account {
    pub uid: u32,
    pub l2_account: L2Account,
}

impl Account {
    pub fn new(uid: u32) -> Result<Self, String> {
        // TODO: Tries to generate a random Account as `ethers.js`.
        let l2_account = L2Account::new(rand_seed())?;
        Ok(Self { uid, l2_account })
    }
    pub fn ay(&self) -> Fr {
        self.l2_account.ay
    }
    pub fn eth_addr(&self) -> Fr {
        // TODO: Generates and returns ether address.
        Fr::zero()
    }
    pub fn sign(&self) -> Fr {
        self.l2_account.sign
    }
    pub fn sign_hash(&self, hash: Fr) -> Result<Signature, String> {
        self.l2_account.sign_hash(hash)
    }
}

fn rand_seed() -> Vec<u8> {
    let mut rng = rand::thread_rng();
    (0..32).map(|_| rng.gen()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::primitives::fr_to_string;
    use ff::PrimeField;

    #[test]
    fn test_account() {
        // https://github.com/Fluidex/circuits/blob/afeeda76e1309f3d8a14ec77ea082cb176acc90a/helper.ts/account_test.ts#L32
        let seed = hex::decode("87b34b2b842db0cc945659366068053f325ff227fd9c6788b2504ac2c4c5dc2a").unwrap();
        let acc: L2Account = L2Account::new(seed).unwrap();
        let priv_bigint = acc.priv_key.scalar_key().to_string();
        assert_eq!(
            priv_bigint,
            "4168145781671332788401281374517684700242591274637494106675223138867941841158"
        );
        assert_eq!(acc.bjj_pub_key, "a59226beb68d565521497d38e37f7d09c9d4e97ac1ebc94fba5de524cb1ca4a0");
        assert_eq!(
            fr_to_bigint(&acc.ax).to_str_radix(16),
            "1fce25ec2e7eeec94079ec7866a933a8b21f33e0ebd575f3001d62d19251d455"
        );
        assert_eq!(
            fr_to_bigint(&acc.ay).to_str_radix(16),
            "20a41ccb24e55dba4fc9ebc17ae9d4c9097d7fe3387d492155568db6be2692a5"
        );
        assert_eq!(acc.sign, u32_to_fr(1));
        let sig = acc.sign_hash(Fr::from_str("1357924680").unwrap()).unwrap();
        assert_eq!(
            fr_to_string(&sig.r8x),
            "15679698175365968671287592821268512384454163537665670071564984871581219397966"
        );
        assert_eq!(
            fr_to_string(&sig.r8y),
            "1705544521394286010135369499330220710333064238375605681220284175409544486013"
        );
        assert_eq!(
            fr_to_bigint(&sig.s).to_string(),
            "2104729104368328243963691045555606467740179640947024714099030450797354625308"
        );
    }
}
