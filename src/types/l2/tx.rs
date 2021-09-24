use super::order;
use super::tx_detail_idx;
use crate::account::Signature;
use crate::types::merkle_tree::MerklePath;
use anyhow::anyhow;
use anyhow::Result;
use ethers::core::types::U256;
use fluidex_common::ff::Field;
use fluidex_common::num_bigint::BigInt;
use fluidex_common::types::{Float40, FrExt};
use fluidex_common::Fr;
use num::{One, PrimInt, ToPrimitive, Zero};
use sha2::Digest;

#[derive(Copy, Clone)]
#[repr(u8)]
pub enum TxType {
    Nop,
    Deposit,
    Transfer,
    Withdraw,
    PlaceOrder,
    SpotTrade,
}

pub struct RawTx {
    pub tx_type: TxType,
    pub payload: Vec<Fr>,
    pub balance_path0: MerklePath,
    pub balance_path1: MerklePath,
    pub balance_path2: MerklePath,
    pub balance_path3: MerklePath,
    pub order_path0: MerklePath,
    pub order_path1: MerklePath,
    pub order_root0: Fr,
    pub order_root1: Fr,
    pub account_path0: MerklePath,
    pub account_path1: MerklePath,
    pub root_before: Fr,
    pub root_after: Fr,
    pub offset: Option<i64>,
    // debug info
    // extra: any;
}

pub type AmountType = Float40;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct L2Key {
    pub eth_addr: Fr,
    pub sign: Fr,
    pub ay: Fr,
}

pub enum L2Tx {
    Deposit(DepositTx),
    Transfer(TransferTx),
    FullSpotTrade(FullSpotTradeTx),
    Withdraw(WithdrawTx),
    Nop,
}

#[derive(Debug)]
pub struct NopTx {}

#[derive(Debug)]
pub struct DepositTx {
    pub account_id: u32,
    pub token_id: u32,
    pub amount: AmountType,
    // when l2key is none, deposit to existed account
    // else, deposit and create new account
    pub l2key: Option<L2Key>,
}

#[derive(Debug)]
pub struct SpotTradeTx {
    pub order1_account_id: u32,
    pub order2_account_id: u32,
    pub token_id_1to2: u32,
    pub token_id_2to1: u32,
    pub amount_1to2: AmountType,
    pub amount_2to1: AmountType,
    pub order1_id: u32,
    pub order2_id: u32,
}

#[derive(Debug)]
pub struct FullSpotTradeTx {
    pub trade: SpotTradeTx,
    // when xx_order is none, GlobalState must already has the order
    pub maker_order: Option<order::Order>,
    pub taker_order: Option<order::Order>,
}

#[derive(Debug, Clone)]
pub struct TransferTx {
    pub from: u32,
    pub to: u32,
    pub token_id: u32,
    pub amount: AmountType,
    pub from_nonce: Fr,
    pub sig: Signature,
    pub l2key: Option<L2Key>,
}

impl TransferTx {
    pub fn new(from: u32, to: u32, token_id: u32, amount: AmountType) -> Self {
        Self {
            from,
            to,
            token_id,
            amount,
            from_nonce: Fr::zero(),
            sig: Signature::default(),
            l2key: None,
        }
    }

    pub fn hash(&self) -> Fr {
        // adhoc ... FIXME
        // fluidex.js / dingir-exchange does not handle precision correctly now
        let amount = Fr::from_u32(self.amount.to_fr().to_i64() as u32 / 1000000);
        Fr::hash(&[
            Fr::from_u32(TxType::Transfer as u32),
            Fr::from_u32(self.token_id),
            amount,
            Fr::from_u32(self.from),
            self.from_nonce,
            Fr::from_u32(self.to),
        ])
        /*
        let data = Fr::hash(&[
            Fr::from_u32(TxType::Transfer as u32),
            Fr::from_u32(self.token_id),
            self.amount.to_fr(),
        ]);
        // do we really need to sign oldBalance?
        // i think we don't need to sign old_balance_from/to_nonce/old_balance_to?
        let data = Fr::hash(&[data, Fr::from_u32(self.from), self.from_nonce]);
        Fr::hash(&[data, Fr::from_u32(self.to)])
        */
    }
}

// WithdrawTx can only withdraw to one's own L1 address
#[derive(Debug)]
pub struct WithdrawTx {
    pub account_id: u32,
    pub token_id: u32,
    pub amount: AmountType,
    pub nonce: Fr,
    pub old_balance: Fr,
    pub sig: Signature,
}

impl WithdrawTx {
    pub fn new(account_id: u32, token_id: u32, amount: AmountType, _old_balance: Fr) -> Self {
        Self {
            account_id,
            token_id,
            amount,
            nonce: Fr::zero(),
            old_balance: Fr::zero(), // TODO: Use real `old_balance` for hash.
            sig: Signature::default(),
        }
    }

    pub fn hash(&self) -> Fr {
        // adhoc ... FIXME
        // fluidex.js / dingir-exchange does not handle precision correctly now
        let amount = Fr::from_u32(self.amount.to_fr().to_i64() as u32 / 1000000);
        Fr::hash(&[
            Fr::from_u32(TxType::Withdraw as u32),
            Fr::from_u32(self.account_id),
            Fr::from_u32(self.token_id),
            amount,
            self.nonce,
            self.old_balance,
        ])
    }
}

// https://github.com/fluidex/circuits/issues/144
// https://github.com/fluidex/circuits/pull/181
struct BitEncodeContext {
    encoding_buf: Vec<u8>,
    applying_bit: usize,
    encoding_char: u8,
}

impl BitEncodeContext {
    pub fn new() -> Self {
        BitEncodeContext {
            encoding_buf: Vec::new(),
            applying_bit: 0,
            encoding_char: 0,
        }
    }

    fn next_byte(&mut self) {
        assert_eq!(self.applying_bit, 8);
        self.encoding_buf.push(self.encoding_char);
        self.applying_bit = 0;
        self.encoding_char = 0u8;
    }

    fn apply_bit(&mut self, is_zero: bool) {
        let mask = [128, 64, 32, 16, 8, 4, 2, 1];

        if !is_zero {
            self.encoding_char += mask[self.applying_bit];
        }
        self.applying_bit += 1;

        if self.applying_bit == 8 {
            self.next_byte();
        }
    }

    fn seal(mut self) -> Vec<u8> {
        self.encoding_buf.push(self.encoding_char);
        self.encoding_buf
    }

    fn encode_primint<T: PrimInt + Zero + One>(&mut self, n: T, bits: u32) -> Result<()> {
        let mut n = n;
        let mut i = bits;

        while i > 0 {
            self.apply_bit((n & T::one()) == T::zero());
            n = n.unsigned_shr(1);
            i -= 1;
        }

        if n != T::zero() {
            Err(anyhow!("can not encode number within specified bits"))
        } else {
            Ok(())
        }
    }

    fn encode_bytes(&mut self, bts: &[u8]) {
        for ch in bts {
            self.encode_primint(*ch, 8).unwrap();
        }
    }

    fn encode_str(&mut self, s: &str) {
        self.encode_bytes(s.as_bytes())
    }
}

impl std::io::Write for BitEncodeContext {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.encode_bytes(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub const AMOUNT_LEN: u32 = 5;

pub struct TxDataEncoder {
    ctx: Option<BitEncodeContext>,
    pub account_bits: u32,
    pub token_bits: u32,
}

impl TxDataEncoder {
    pub fn new(balance_levels: u32, account_levels: u32) -> Self {
        TxDataEncoder {
            ctx: Some(BitEncodeContext::new()),
            account_bits: account_levels,
            token_bits: balance_levels,
        }
    }

    pub fn reset(&mut self) {
        self.ctx.replace(BitEncodeContext::new());
    }

    pub fn pubdata_len_bits(&self) -> u32 {
        self.account_bits + self.token_bits + AMOUNT_LEN * 8
    }

    pub fn encode_account(&mut self, account_id: u32) -> Result<()> {
        self.ctx.as_mut().unwrap().encode_primint(account_id, self.account_bits)
    }

    pub fn encode_token(&mut self, token_id: u32) -> Result<()> {
        self.ctx.as_mut().unwrap().encode_primint(token_id, self.token_bits)
    }

    pub fn encode_amount(&mut self, amount: &AmountType) -> Result<()> {
        let encoded_big = amount.to_encoded_int()?;
        assert_eq!(AMOUNT_LEN, AmountType::encode_len() as u32);
        self.ctx
            .as_mut()
            .unwrap()
            .encode_primint(encoded_big.to_u128().unwrap(), AMOUNT_LEN * 8)
    }

    pub fn encode_fr(&mut self, fr: &Fr, bits: u32) -> Result<()> {
        self.ctx.as_mut().unwrap().encode_primint(fr.to_i64(), bits)
    }

    //finish encoding, output the result hash, and prepare for next encoding
    pub fn finish(&mut self) -> (U256, Vec<u8>) {
        let encoded_bytes = self.ctx.replace(BitEncodeContext::new()).unwrap().seal();
        //        println!("{:x?}", &encoded_bytes);
        let hash = U256::from_big_endian(&sha2::Sha256::digest(&encoded_bytes));
        (hash, encoded_bytes)
    }
}

impl RawTx {
    pub fn encode_pubdata(&self, encoder: &mut TxDataEncoder) -> Result<()> {
        let payload = &self.payload;
        encoder.encode_fr(&payload[tx_detail_idx::ACCOUNT_ID1], encoder.account_bits)?;
        encoder.encode_fr(&payload[tx_detail_idx::ACCOUNT_ID2], encoder.account_bits)?;
        encoder.encode_fr(&payload[tx_detail_idx::TOKEN_ID1], encoder.token_bits)?;
        encoder.encode_fr(&payload[tx_detail_idx::AMOUNT], AMOUNT_LEN * 8)?;
        Ok(())
    }
}

impl NopTx {
    pub fn encode_pubdata(&self, encoder: &mut TxDataEncoder) -> Result<()> {
        encoder.encode_account(0)?;
        encoder.encode_account(0)?;
        encoder.encode_token(0)?;
        encoder.encode_amount(&AmountType::from_encoded_bigint(BigInt::from(0)).unwrap())
    }
}

impl DepositTx {
    pub fn encode_pubdata(&self, encoder: &mut TxDataEncoder) -> Result<()> {
        encoder.encode_account(self.account_id)?;
        encoder.encode_account(self.account_id)?;
        encoder.encode_token(self.token_id)?;
        encoder.encode_amount(&self.amount)
    }
}

impl TransferTx {
    pub fn encode_pubdata(&self, encoder: &mut TxDataEncoder) -> Result<()> {
        encoder.encode_account(self.from)?;
        encoder.encode_account(self.to)?;
        encoder.encode_token(self.token_id)?;
        encoder.encode_amount(&self.amount)
    }
}

impl SpotTradeTx {
    pub fn encode_pubdata(&self, encoder: &mut TxDataEncoder) -> Result<()> {
        //TODO: spot trade is not completely encoded yet
        encoder.encode_account(self.order1_account_id)?;
        encoder.encode_account(self.order2_account_id)?;
        encoder.encode_token(self.token_id_1to2)?;
        encoder.encode_amount(&self.amount_1to2)
    }
}

impl WithdrawTx {
    pub fn encode_pubdata(&self, encoder: &mut TxDataEncoder) -> Result<()> {
        //TODO: should we encode amount as minus?
        encoder.encode_account(self.account_id)?;
        encoder.encode_account(self.account_id)?;
        encoder.encode_token(self.token_id)?;
        encoder.encode_amount(&self.amount)
    }
}

/*
impl DepositToOldTx {
    pub fn to_pubdata(&self) -> Vec<u8> {
        let mut result = vec![TxType::DepositToOld as u8];
        result.append(&mut self.account_id.to_be_bytes().to_vec());
        result.append(&mut (self.token_id as u16).to_be_bytes().to_vec());
        result.append(&mut self.amount.encode());
        assert!(result.len() <= PUBDATA_LEN);
        result.append(&mut vec![0; PUBDATA_LEN - result.len()]);
        result
    }
    pub fn from_pubdata(data: &[u8]) -> Result<Self> {
        if data.len() != PUBDATA_LEN {
            bail!("invalid len for DepositToOldTx");
        }
        let mut idx: usize = 0;

        if data[0] != TxType::DepositToOld as u8 {
            bail!("invalid type for DepositToOldTx");
        }
        idx += 1;

        let account_id = u32::from_be_bytes(data[idx..(idx + ACCOUNT_ID_LEN)].try_into()?);
        idx += ACCOUNT_ID_LEN;

        let token_id = (u16::from_be_bytes(data[idx..(idx + TOKEN_ID_LEN)].try_into()?)) as u32;
        idx += TOKEN_ID_LEN;

        let amount = AmountType::decode(&data[idx..(idx + AMOUNT_LEN)])?;
        Ok(Self {
            account_id,
            token_id,
            amount,
        })
    }
}
*/
#[cfg(test)]
#[test]
fn test_tx_pubdata() {
    let mut tx_encoder = TxDataEncoder::new(3, 3);

    let tx1 = DepositTx {
        account_id: 2,
        token_id: 5,
        amount: AmountType::from_encoded_bigint(BigInt::from(1234567)).unwrap(),
        l2key: None,
    };

    tx1.encode_pubdata(&mut tx_encoder).unwrap();

    let tx2 = DepositTx {
        account_id: 2,
        token_id: 5,
        amount: AmountType::from_encoded_bigint(BigInt::from(3000)).unwrap(),
        l2key: None,
    };

    tx2.encode_pubdata(&mut tx_encoder).unwrap();

    let tx3 = TransferTx {
        from: 2,
        to: 6,
        token_id: 5,
        amount: AmountType::from_encoded_bigint(BigInt::from(6000)).unwrap(),
        l2key: None,
        from_nonce: Fr::zero(),
        sig: Signature::default(),
    };

    tx3.encode_pubdata(&mut tx_encoder).unwrap();

    let tx_nop = NopTx {};
    tx_nop.encode_pubdata(&mut tx_encoder).unwrap();
    tx_nop.encode_pubdata(&mut tx_encoder).unwrap();

    let (hash, _) = tx_encoder.finish();
    //preimage should be: 4af0b5a4000025477400000013a1dd00000000000000000000000000000000
    assert_eq!(hash.low_u128(), 273971448787759175191113939742247265668u128);
}
/*
#[cfg(test)]
#[test]
fn test_deposit_to_new_pubdata() {
    let tx = DepositTx {
        account_id: 1323,
        token_id: 232,
        amount: AmountType {
            significand: 756,
            exponent: 11,
        },
        l2key: Some(L2Key {
            eth_addr: Fr::from_u64(1223232332323233),
            sign: Fr::from_u64(1),
            ay: Fr::from_u64(987657654765),
        }),
    };
    let pubdata1 = tx.to_pubdata();
    println!("pubdata {:?}", pubdata1);
    let tx2 = DepositTx::from_pubdata(&pubdata1).unwrap();
    assert_eq!(tx.account_id, tx2.account_id);
    assert_eq!(tx.token_id, tx2.token_id);
    assert_eq!(tx.amount.to_bigint(), tx2.amount.to_bigint());
    assert_eq!(tx.l2key.unwrap(), tx2.l2key.unwrap());
}
*/
