use anyhow::Result;
use coins_bip32::path::DerivationPath;
use ethers::core::k256::ecdsa::{SigningKey, VerifyingKey};
use ethers::core::types::Address;
use ethers::prelude::coins_bip39::{English, Mnemonic, Wordlist};
use ethers::utils::secret_key_to_address;
use fluidex_common::ff::from_hex;
use fluidex_common::l2::account::{L2Account, Signature, SignatureBJJ};
use fluidex_common::Fr;
use std::str::FromStr;

/// Derault derivation path.
/// Copied from https://github.com/gakonst/ethers-rs/blob/01cc80769c291fc80f5b1e9173b7b580ae6b6413/ethers-signers/src/wallet/mnemonic.rs#L16
const DEFAULT_DERIVATION_PATH_PREFIX: &str = "m/44'/60'/0'/0/";

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
        let public_key = priv_key.verifying_key();
        let eth_addr = secret_key_to_address(priv_key);

        let l2_account = L2Account::from_private_key(priv_key)?;
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
    pub fn eth_addr_str(&self) -> String {
        String::from("0x") + &hex::encode(self.eth_addr.as_bytes())
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

pub fn random_mnemonic<W: Wordlist>() -> Mnemonic<W> {
    let mut rng = ethers::core::rand::thread_rng();
    random_mnemonic_with_rng(&mut rng)
}

pub fn random_mnemonic_with_rng<W: Wordlist, R: ethers::core::rand::Rng>(rng: &mut R) -> Mnemonic<W> {
    Mnemonic::<W>::new_with_count::<R>(rng, 24).unwrap()
}

#[cfg(test)]
mod tests {
    use ethers::core::k256::EncodedPoint as K256PublicKey;
    use std::str::FromStr;

    use super::*;
    use crate::types::l2::*;
    use fluidex_common::types::FrExt;

    #[test]
    fn test_account() {
        // mnemonic => L1 account & eth addr & L2 account
        // https://github.com/fluidex/circuits/blob/d6e06e964b9d492f1fa5513bcc2295e7081c540d/helper.ts/account_test.ts#L7
        let mnemonic = Mnemonic::<English>::new_from_phrase("radar blur cabbage chef fix engine embark joy scheme fiction master release")
            .expect("should generate mnemonic from phrase");
        let acc = Account::from_mnemonic(0, &mnemonic).expect("should generate account from mnemonic");
        assert_eq!(
            K256PublicKey::from(&acc.public_key).decompress().unwrap().as_bytes(),
            hex::decode("0405b7d0996e99c4a49e6c3b83288f4740d53662839eab1d97d14660696944b8bbe24fabdd03888410ace3fa4c5a809e398f036f7b99d04f82a012dca95701d103").unwrap());
        assert_eq!(acc.eth_addr, Address::from_str("aC39b311DCEb2A4b2f5d8461c1cdaF756F4F7Ae9").unwrap());

        // priv key => L1 account & eth addr & L2 account
        // https://github.com/fluidex/circuits/blob/d6e06e964b9d492f1fa5513bcc2295e7081c540d/helper.ts/account_test.ts#L25
        let priv_key = SigningKey::from_bytes(&hex::decode("0b22f852cd07386bce533f2038821fdcebd9c5ced9e3cd51e3a05d421dbfd785").unwrap())
            .expect("should generate signing key from bytes");
        let acc = Account::from_priv_key(0, &priv_key).expect("should generate account from priv key");
        assert_eq!(
            K256PublicKey::from(&acc.public_key).decompress().unwrap().as_bytes(),
            hex::decode("04baac45822c3d99f88d346bd54054c5cf7362913566a03d2e7fb5941c22efa14a28d9ea9fa1301227119fbfd8e95afa99c06715bb00d8d3cc4cd51f061c36fc0f").unwrap());
        assert_eq!(acc.eth_addr, Address::from_str("25EC658304dd1e2a4E25B34Ad6aC5169746c4684").unwrap());
    }

    #[test]
    fn test_order_signature() {
        // https://github.com/fluidex/rollup-state-manager/blob/master/tests/data/accounts.jsonl account id 1
        let mnemonic = Mnemonic::<English>::new_from_phrase("olympic comfort palm large heavy verb acid lion attract vast dash memory olympic syrup announce sure body cruise flip merge fabric frame question result")
            .expect("should generate mnemonic from phrase");
        let acc = Account::from_mnemonic(0, &mnemonic).expect("should generate account from mnemonic");

        let mut order: l2::OrderInput = l2::OrderInput {
            account_id: 1,
            order_id: 1,
            side: l2::order::OrderSide::Buy,
            token_buy: Fr::from_u32(1),
            token_sell: Fr::from_u32(2),
            total_buy: Fr::from_u32(999),
            total_sell: Fr::from_u32(888),
            sig: None,
        };
        order.sign_with(&acc).unwrap();

        assert_eq!(
            order.hash().to_decimal_string(),
            "8056692562185768785417295010793063162660984530596417435073781442183268221458",
            "message (Fr) to sign"
        );

        assert_eq!(
            hex::encode(order.hash().to_vec_be()), // big endian
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
            s: Fr::from_bigint(sig.s),
            r8x: sig.r_b8.x,
            r8y: sig.r_b8.y,
        };
        assert!(acc.l2_account.verify(detailed_sig));
    }
}
