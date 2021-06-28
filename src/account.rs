#[cfg(not(feature = "fr_string_repr"))]
use crate::types::primitives::fr_bytes as fr_serde;
#[cfg(feature = "fr_string_repr")]
use crate::types::primitives::fr_str as fr_serde;
use crate::types::primitives::{bigint_to_fr, fr_to_bigint, Fr};
use anyhow::Result;
use arrayref::array_ref;
use babyjubjub_rs::{self, decompress_point, Point, PrivateKey};
use coins_bip32::{path::DerivationPath, prelude::DigestSigner};
use ethers::{
    core::k256::ecdsa::recoverable::Signature as RecoverableSignature, core::k256::ecdsa::Signature as K256Signature,
    core::k256::EncodedPoint as K256PublicKey, prelude::Signature as EthersSignature,
};
use ethers::{
    core::{
        k256::{
            ecdsa::{
                digest::{generic_array::GenericArray, BlockInput, Digest, FixedOutput, Output, Reset, Update},
                recoverable, Error, SigningKey, VerifyingKey,
            },
            elliptic_curve::{
                consts::{U32, U64},
                FieldBytes,
            },
            Secp256k1,
        },
        types::{Address, H256},
    },
    prelude::coins_bip39::{English, Mnemonic, Wordlist},
    signers::to_eip155_v,
    utils::{hash_message, keccak256, secret_key_to_address},
};
use ff::from_hex;
use ff::Field;
use lazy_static::lazy_static;
use num_bigint::BigInt;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Derault derivation path.
/// Copied from https://github.com/gakonst/ethers-rs/blob/01cc80769c291fc80f5b1e9173b7b580ae6b6413/ethers-signers/src/wallet/mnemonic.rs#L16
const DEFAULT_DERIVATION_PATH_PREFIX: &str = "m/44'/60'/0'/0/";

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Signature {
    #[serde(with = "fr_serde")]
    pub hash: Fr,
    #[serde(with = "fr_serde")]
    pub s: Fr,
    #[serde(with = "fr_serde")]
    pub r8x: Fr,
    #[serde(with = "fr_serde")]
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

pub type SignatureBJJ = babyjubjub_rs::Signature;
/*
// don't change struct fields here!!!
#[derive(Debug, Clone)]
pub struct SignatureBJJ {
    pub r_b8: Point,
    pub s: BigInt,
}

impl Default for SignatureBJJ {
    fn default() -> Self {
        Self {
            r_b8: Point {
                x: Fr::default(),
                y: Fr::default(),
            },
            s: BigInt::default(),
        }
    }
}
*/

impl L2Account {
    pub fn new(seed: Vec<u8>) -> Result<Self, String> {
        let priv_key = PrivateKey::import(seed)?;
        let pub_key: Point = priv_key.public();
        let ax = pub_key.x;
        let ay = pub_key.y;
        let bjj_compressed = pub_key.compress();
        let sign = if bjj_compressed[31] & 0x80 != 0x00 { Fr::one() } else { Fr::zero() };
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

    // TODO: sign and verify involves a lot of unnecessary computing
    pub fn sign_hash_safe(&self, hash: Fr) -> Result<Signature, String> {
        let b = self.priv_key.sign(fr_to_bigint(&hash))?.compress();
        let r_b8_bytes: [u8; 32] = *array_ref!(b[..32], 0, 32);
        let s = bigint_to_fr(BigInt::from_bytes_le(num_bigint::Sign::Plus, &b[32..]));
        let r_b8 = decompress_point(r_b8_bytes);
        match r_b8 {
            Result::Err(err) => Err(err),
            Result::Ok(Point { x: r8x, y: r8y }) => Ok(Signature { hash, s, r8x, r8y }),
        }
    }

    pub fn verify_safe(&self, sig: Signature) -> bool {
        let msg = fr_to_bigint(&sig.hash);
        let r_b8 = Point { x: sig.r8x, y: sig.r8y };

        let mut b: Vec<u8> = Vec::new();
        b.append(&mut r_b8.compress().to_vec());
        let (_, s_bytes) = fr_to_bigint(&sig.s).to_bytes_le();
        let mut s_32bytes: [u8; 32] = [0; 32];
        let len = std::cmp::min(s_bytes.len(), s_32bytes.len());
        s_32bytes[..len].copy_from_slice(&s_bytes[..len]);
        b.append(&mut s_32bytes.to_vec());
        let mut buf: [u8; 64] = [0; 64];
        buf[..].copy_from_slice(&b[..]);

        match babyjubjub_rs::decompress_signature(&buf) {
            Err(_) => false,
            Ok(sig) => babyjubjub_rs::verify(self.pub_key.clone(), sig, msg),
        }
    }
    pub fn sign_hash_raw(&self, hash: Fr) -> Result<SignatureBJJ, String> {
        self.priv_key.sign(fr_to_bigint(&hash))
    }
    pub fn sign_hash(&self, hash: Fr) -> Result<Signature, String> {
        let sig = self.sign_hash_raw(hash)?;
        let s = bigint_to_fr(sig.s);
        Ok(Signature {
            hash,
            s,
            r8x: sig.r_b8.x,
            r8y: sig.r_b8.y,
        })
    }
    pub fn sign_hash_packed(&self, hash: Fr) -> Result<[u8; 64], String> {
        Ok(self.priv_key.sign(fr_to_bigint(&hash))?.compress())
    }
    pub fn verify(&self, sig: Signature) -> bool {
        Self::verify_using_pubkey(sig, &self.pub_key)
    }
    pub fn verify_raw_using_pubkey(msg: Fr, sig_bjj: SignatureBJJ, pub_key: Point) -> bool {
        let msg = fr_to_bigint(&msg);
        babyjubjub_rs::verify(pub_key, sig_bjj, msg)
    }
    pub fn verify_using_pubkey(sig: Signature, pub_key: &Point) -> bool {
        let r_b8 = Point { x: sig.r8x, y: sig.r8y };
        let sig_bjj = SignatureBJJ {
            r_b8,
            s: fr_to_bigint(&sig.s),
        };
        Self::verify_raw_using_pubkey(sig.hash, sig_bjj, pub_key.clone())
    }
}

pub struct Account {
    pub uid: u32,
    pub public_key: VerifyingKey,
    pub eth_addr: Address,
    pub l2_account: L2Account,
}

impl Account {
    pub fn new(uid: u32) -> Self {
        let mnemonic = random_mnemonic::<English>();
        // TODO: retry if error.
        Self::from_mnemonic::<English>(uid, &mnemonic).unwrap()
    }
    pub fn from_mnemonic<W: Wordlist>(uid: u32, mnemonic: &Mnemonic<W>) -> Result<Self, String> {
        let path = DerivationPath::from_str(&format!("{}{}", DEFAULT_DERIVATION_PATH_PREFIX, 0)).unwrap();
        let priv_key = match mnemonic.derive_key(path, None) {
            Ok(key) => key,
            Err(_err) => return Err("private key generation error".to_string()),
        };
        Self::from_priv_key(uid, priv_key.as_ref())
    }
    pub fn from_priv_key(uid: u32, priv_key: &SigningKey) -> Result<Self, String> {
        let public_key = priv_key.verify_key();
        let eth_addr = secret_key_to_address(priv_key);

        let signature = sign_msg_with_signing_key(priv_key, &*CREATE_L2_ACCOUNT_MSG);
        let seed = &signature.to_vec()[0..32];
        let l2_account = L2Account::new(seed.to_vec())?;
        Ok(Self {
            uid,
            public_key,
            eth_addr,
            l2_account,
        })
    }
    pub fn from_signature(uid: u32, signature: &EthersSignature) -> Result<Self, String> {
        let msg_hash = hash_message(&*CREATE_L2_ACCOUNT_MSG);
        let recoverable_sig = match convert_signature(&signature) {
            Ok(sig) => sig,
            Err(_err) => return Err("signature convertion error".to_string()),
        };
        let public_key = match recoverable_sig.recover_verify_key_from_digest_bytes(msg_hash.as_ref().into()) {
            Ok(key) => key,
            Err(_err) => return Err("public key generation error".to_string()),
        };
        let eth_addr = compute_address_from_public_key(&public_key);

        // Signature recover fn for testing purpose.
        // let eth_addr_recovered = signature.recover(get_create_l2_account_msg(None)).unwrap();

        let seed = &signature.to_vec()[0..32];
        let l2_account = L2Account::new(seed.to_vec())?;
        Ok(Self {
            uid,
            public_key,
            eth_addr,
            l2_account,
        })
    }
    pub fn ay(&self) -> Fr {
        self.l2_account.ay
    }
    pub fn bjj_pub_key(&self) -> String {
        self.l2_account.bjj_pub_key.clone()
    }
    pub fn eth_addr(&self) -> Fr {
        from_hex(&hex::encode(self.eth_addr.as_bytes())).unwrap()
    }
    pub fn sign(&self) -> Fr {
        self.l2_account.sign
    }
    pub fn sign_hash(&self, hash: Fr) -> Result<Signature, String> {
        self.l2_account.sign_hash(hash)
    }
    pub fn sign_hash_raw(&self, hash: Fr) -> Result<SignatureBJJ, String> {
        self.l2_account.sign_hash_raw(hash)
    }
}

pub fn rand_seed() -> Vec<u8> {
    let mut rng = rand::thread_rng();
    (0..32).map(|_| rng.gen()).collect()
}

pub fn random_mnemonic<W: Wordlist>() -> Mnemonic<W> {
    let mut rng = ethers::core::rand::thread_rng();
    random_mnemonic_with_rng(&mut rng)
}

pub fn random_mnemonic_with_rng<W: Wordlist, R: ethers::core::rand::Rng>(rng: &mut R) -> Mnemonic<W> {
    Mnemonic::<W>::new_with_count::<R>(rng, 24).unwrap()
}

lazy_static! {
    static ref CHAIN_ID: u32 = std::env::var("CHAIN_ID")
        .unwrap_or_else(|_| "1".to_string())
        .parse::<u32>()
        .unwrap_or(1);
    pub static ref CREATE_L2_ACCOUNT_MSG: String = format!("FLUIDEX_L2_ACCOUNT\nChain ID: {}.", *CHAIN_ID);
}

/// Converts ethers core signature to recoverable signature
/// Copied from https://github.com/gakonst/ethers-rs/blob/01cc80769c291fc80f5b1e9173b7b580ae6b6413/ethers-core/src/types/signature.rs#L120
fn convert_signature(signature: &EthersSignature) -> Result<RecoverableSignature, Error> {
    let gar: &GenericArray<u8, U32> = GenericArray::from_slice(signature.r.as_bytes());
    let gas: &GenericArray<u8, U32> = GenericArray::from_slice(signature.s.as_bytes());
    let sig = K256Signature::from_scalars(*gar, *gas)?;
    RecoverableSignature::new(&sig, recoverable::Id::new(normalize_recovery_id(signature.v)).unwrap())
}

/// Normalizes recovery id for recoverable signature.
/// Copied from https://github.com/gakonst/ethers-rs/blob/01cc80769c291fc80f5b1e9173b7b580ae6b6413/ethers-core/src/types/signature.rs#L142
fn normalize_recovery_id(v: u64) -> u8 {
    match v {
        0 => 0,
        1 => 1,
        27 => 0,
        28 => 1,
        v if v >= 35 => ((v - 1) % 2) as _,
        _ => 4,
    }
}

/// Computes ETH address from public key.
fn compute_address_from_public_key(verify_key: &VerifyingKey) -> Address {
    let public_key = K256PublicKey::from(verify_key).decompress().unwrap().to_bytes();
    debug_assert_eq!(public_key[0], 0x04);
    let hash = keccak256(&public_key[1..]);
    Address::from_slice(&hash[12..])
}

/// Signs the message with the signing key and returns the ethers core signature.
/// Copied from https://github.com/gakonst/ethers-rs/blob/01cc80769c291fc80f5b1e9173b7b580ae6b6413/ethers-signers/src/wallet/mod.rs#L71
fn sign_msg_with_signing_key(priv_key: &SigningKey, msg: &str) -> EthersSignature {
    let msg_hash = hash_message(msg);
    let digest = Sha256Proxy::from(msg_hash);
    let recoverable_sig: RecoverableSignature = priv_key.sign_digest(digest);

    let v = to_eip155_v(recoverable_sig.recovery_id(), None);

    let r_bytes: FieldBytes<Secp256k1> = recoverable_sig.r().into();
    let s_bytes: FieldBytes<Secp256k1> = recoverable_sig.s().into();
    let r = H256::from_slice(&r_bytes.as_slice());
    let s = H256::from_slice(&s_bytes.as_slice());

    EthersSignature { r, s, v }
}

// Helper type for calling sign_digest method of SigningKey.
// Copied from https://github.com/gakonst/ethers-rs/blob/01cc80769c291fc80f5b1e9173b7b580ae6b6413/ethers-signers/src/wallet/hash.rs#L11
type Sha256Proxy = ProxyDigest<sha2::Sha256>;

#[derive(Clone)]
enum ProxyDigest<D: Digest> {
    Proxy(Output<D>),
    Digest(D),
}

impl<D: Digest + Clone> From<H256> for ProxyDigest<D>
where
    GenericArray<u8, <D as Digest>::OutputSize>: Copy,
{
    fn from(src: H256) -> Self {
        ProxyDigest::Proxy(*GenericArray::from_slice(src.as_bytes()))
    }
}

impl<D: Digest> Default for ProxyDigest<D> {
    fn default() -> Self {
        ProxyDigest::Digest(D::new())
    }
}

impl<D: Digest> Update for ProxyDigest<D> {
    // we update only if we are digest
    fn update(&mut self, data: impl AsRef<[u8]>) {
        match self {
            ProxyDigest::Digest(ref mut d) => {
                d.update(data);
            }
            ProxyDigest::Proxy(..) => {
                unreachable!("can not update if we are proxy");
            }
        }
    }

    // we chain only if we are digest
    fn chain(self, data: impl AsRef<[u8]>) -> Self {
        match self {
            ProxyDigest::Digest(d) => ProxyDigest::Digest(d.chain(data)),
            ProxyDigest::Proxy(..) => {
                unreachable!("can not update if we are proxy");
            }
        }
    }
}

impl<D: Digest> Reset for ProxyDigest<D> {
    // make new one
    fn reset(&mut self) {
        *self = Self::default();
    }
}

// Use Sha256 with 512 bit blocks
impl<D: Digest> BlockInput for ProxyDigest<D> {
    type BlockSize = U64;
}

impl<D: Digest> FixedOutput for ProxyDigest<D> {
    // we default to the output of the original digest
    type OutputSize = D::OutputSize;

    fn finalize_into(self, out: &mut GenericArray<u8, Self::OutputSize>) {
        match self {
            ProxyDigest::Digest(d) => {
                *out = d.finalize();
            }
            ProxyDigest::Proxy(p) => {
                *out = p;
            }
        }
    }

    fn finalize_into_reset(&mut self, out: &mut GenericArray<u8, Self::OutputSize>) {
        let s = std::mem::take(self);
        s.finalize_into(out);
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;
    use std::str::FromStr;

    use super::*;
    use crate::types::l2::*;
    use crate::types::primitives::*;
    use ff::PrimeField;

    #[test]
    fn test_account() {
        // Step1: test l2 keypair
        // https://github.com/Fluidex/circuits/blob/afeeda76e1309f3d8a14ec77ea082cb176acc90a/helper.ts/account_test.ts#L32
        let seed = hex::decode("87b34b2b842db0cc945659366068053f325ff227fd9c6788b2504ac2c4c5dc2a").unwrap();
        let acc: L2Account = L2Account::new(seed).unwrap();
        let priv_bigint = acc.priv_key.scalar_key().to_string();
        let pubkey_expected = hex::decode("a59226beb68d565521497d38e37f7d09c9d4e97ac1ebc94fba5de524cb1ca4a0").unwrap();
        assert_eq!(
            priv_bigint,
            "4168145781671332788401281374517684700242591274637494106675223138867941841158"
        );
        assert_eq!(hex::decode(acc.bjj_pub_key.clone()).unwrap(), pubkey_expected);
        assert_eq!(
            fr_to_bigint(&acc.ax).to_str_radix(16),
            "1fce25ec2e7eeec94079ec7866a933a8b21f33e0ebd575f3001d62d19251d455"
        );
        assert_eq!(
            fr_to_bigint(&acc.ay).to_str_radix(16),
            "20a41ccb24e55dba4fc9ebc17ae9d4c9097d7fe3387d492155568db6be2692a5"
        );
        assert_eq!(acc.sign, u32_to_fr(1));

        // Step2: test l2 sig
        let msg = Fr::from_str("1357924680").unwrap();
        let sig = acc.sign_hash(msg).unwrap();
        let sig_packed_expected = hex::decode("7ddc5c6aadf5e80200bd9f28e9d5bf932cbb7f4224cce0fa11154f4ad24dc5831c295fb522b7b8b4921e271bc6b265f4d7114fbe9516d23e69760065053ca704").unwrap();
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
        assert_eq!(acc.verify(sig), true);
        assert_eq!(acc.sign_hash_packed(msg).unwrap().to_vec(), sig_packed_expected);

        // test sig verification of packed pubkey and packed sig
        let sig_unpacked = babyjubjub_rs::decompress_signature(&sig_packed_expected.try_into().unwrap()).unwrap();
        let pubkey_unpacked = babyjubjub_rs::decompress_point(pubkey_expected.try_into().unwrap()).unwrap();
        assert!(babyjubjub_rs::verify(pubkey_unpacked, sig_unpacked, fr_to_bigint(&msg)));

        // Step3: l1 sig -> l2 keypair
        // mnemonic => L1 account & eth addr & L2 account
        // https://github.com/Fluidex/circuits/blob/d6e06e964b9d492f1fa5513bcc2295e7081c540d/helper.ts/account_test.ts#L7
        let mnemonic = Mnemonic::<English>::new_from_phrase("radar blur cabbage chef fix engine embark joy scheme fiction master release")
            .expect("should generate mnemonic from phrase");
        let acc = Account::from_mnemonic(0, &mnemonic).expect("should generate account from mnemonic");
        assert_eq!(
            K256PublicKey::from(&acc.public_key).decompress().unwrap().as_bytes(),
            hex::decode("0405b7d0996e99c4a49e6c3b83288f4740d53662839eab1d97d14660696944b8bbe24fabdd03888410ace3fa4c5a809e398f036f7b99d04f82a012dca95701d103").unwrap());
        assert_eq!(acc.eth_addr, Address::from_str("aC39b311DCEb2A4b2f5d8461c1cdaF756F4F7Ae9").unwrap());
        assert_eq!(
            acc.l2_account.bjj_pub_key,
            "2984fdce6d8914b34ef6f6d4738a792e853189d61fee02abfc3d2c4ac170aa11"
        );

        // priv key => L1 account & eth addr & L2 account
        // https://github.com/Fluidex/circuits/blob/d6e06e964b9d492f1fa5513bcc2295e7081c540d/helper.ts/account_test.ts#L25
        let priv_key = SigningKey::from_bytes(&hex::decode("0b22f852cd07386bce533f2038821fdcebd9c5ced9e3cd51e3a05d421dbfd785").unwrap())
            .expect("should generate signing key from bytes");
        let acc = Account::from_priv_key(0, &priv_key).expect("should generate account from priv key");
        assert_eq!(
            K256PublicKey::from(&acc.public_key).decompress().unwrap().as_bytes(),
            hex::decode("04baac45822c3d99f88d346bd54054c5cf7362913566a03d2e7fb5941c22efa14a28d9ea9fa1301227119fbfd8e95afa99c06715bb00d8d3cc4cd51f061c36fc0f").unwrap());
        assert_eq!(acc.eth_addr, Address::from_str("25EC658304dd1e2a4E25B34Ad6aC5169746c4684").unwrap());
        assert_eq!(
            acc.l2_account.bjj_pub_key,
            "7b70843a42114e88149e3961495c03f9a41292c8b97bd1e2026597d185478293"
        );

        // signature => L1 public key & eth addr & L2 account
        // https://github.com/Fluidex/circuits/blob/d6e06e964b9d492f1fa5513bcc2295e7081c540d/helper.ts/account_test.ts#L37
        let signature = EthersSignature::from_str("9982364bf709fecdf830a71f417182e3a7f717a6363180ff33784e2823935f8b55932a5353fb128fc7e3d6c4aed57138adce772ce594338a8f4985d6668627b31c").expect("should generate signature from string");
        let acc = Account::from_signature(0, &signature).expect("should generate account from signature");
        assert_eq!(
            K256PublicKey::from(&acc.public_key).decompress().unwrap().as_bytes(),
            hex::decode("04baac45822c3d99f88d346bd54054c5cf7362913566a03d2e7fb5941c22efa14a28d9ea9fa1301227119fbfd8e95afa99c06715bb00d8d3cc4cd51f061c36fc0f").unwrap());
        assert_eq!(acc.eth_addr, Address::from_str("25EC658304dd1e2a4E25B34Ad6aC5169746c4684").unwrap());
        assert_eq!(
            acc.l2_account.bjj_pub_key,
            "7b70843a42114e88149e3961495c03f9a41292c8b97bd1e2026597d185478293"
        );
    }

    #[test]
    fn test_order_signature() {
        // https://github.com/Fluidex/rollup-state-manager/blob/master/tests/data/accounts.jsonl account id 1
        let mnemonic = Mnemonic::<English>::new_from_phrase("olympic comfort palm large heavy verb acid lion attract vast dash memory olympic syrup announce sure body cruise flip merge fabric frame question result")
            .expect("should generate mnemonic from phrase");
        let acc = Account::from_mnemonic(0, &mnemonic).expect("should generate account from mnemonic");
        assert_eq!(
            acc.l2_account.bjj_pub_key,
            "5d182c51bcfe99583d7075a7a0c10d96bef82b8a059c4bf8c5f6e7124cf2bba3"
        );

        let mut order: l2::OrderInput = l2::OrderInput {
            account_id: 1,
            order_id: 1,
            side: l2::order::OrderSide::Buy,
            token_buy: u32_to_fr(1),
            token_sell: u32_to_fr(2),
            total_buy: u32_to_fr(999),
            total_sell: u32_to_fr(888),
            sig: None,
        };
        order.sign_with(&acc).unwrap();

        assert_eq!(
            fr_to_string(&order.hash()),
            "8056692562185768785417295010793063162660984530596417435073781442183268221458",
            "message (Fr) to sign"
        );

        assert_eq!(
            hex::encode(fr_to_vec(&order.hash())), // big endian
            "11cfed280efe7a90a79f3ff69ad6dafc57bfd03e24f176cd1149068268994212",
            "message (hexdecimal string) to sign"
        );

        let sig = order.clone().sig.unwrap();
        let sig_compressed = sig.compress();
        assert_eq!(
            hex::encode(sig_compressed),
            "57e6cf2e5b8db0a90072d15bc49e737df2e10746e5f531a24d72557894f2c90964d77726505232a4c9e7631eed22ad9210dce2858642fdfe3e58e95d44b99002",
        );

        let mut b: Vec<u8> = Vec::new();
        b.append(&mut sig.r_b8.compress().to_vec());
        let (_, s_bytes) = sig.s.to_bytes_le();
        let mut s_32bytes: [u8; 32] = [0; 32];
        let len = std::cmp::min(s_bytes.len(), s_32bytes.len());
        s_32bytes[..len].copy_from_slice(&s_bytes[..len]);
        b.append(&mut s_32bytes.to_vec());
        let mut buf: [u8; 64] = [0; 64];
        buf[..].copy_from_slice(&b[..]);
        assert_eq!(sig_compressed, buf, "different approaches to get sig_compressed");

        let detailed_sig = Signature {
            hash: order.hash(),
            s: bigint_to_fr(sig.s),
            r8x: sig.r_b8.x,
            r8y: sig.r_b8.y,
        };
        assert!(acc.l2_account.verify(detailed_sig));
    }
}
