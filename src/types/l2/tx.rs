use super::order;
use crate::types::merkle_tree::MerklePath;
use anyhow::{anyhow, Result};
use ethers::core::types::U256;
use fluidex_common::ff::Field;
use fluidex_common::l2::account::Signature;
use fluidex_common::num_bigint::{BigUint, BigInt};
use fluidex_common::types::{Float40, FrExt};
use fluidex_common::Fr;
use num::{One, PrimInt, ToPrimitive, Zero};
use sha2::Digest;

#[derive(Copy, Clone, PartialEq, Debug)]
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
pub struct UpdateKeyTx {
    pub account_id: u32,
    pub l2key: L2Key,
}

#[derive(Debug)]
pub struct SpotTradeTx {
    pub order1_account_id: u32,
    pub order2_account_id: u32,
    pub token_id_1to2: u32,
    pub token_id_2to1: u32,
    pub amount_1to2: Fr,
    pub amount_2to1: Fr,
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
            nonce: Fr::zero(), //TODO: nonce is also not involved yet ...
                               //later we should also update the scripts in circuits
            old_balance: Fr::zero(), // TODO: Maybe we should not involve old_balance into hash
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
        if self.applying_bit != 0 {
            self.encoding_buf.push(self.encoding_char);
        }
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

    fn encode_big(&mut self, n: BigInt, bits: u32) -> Result<()> {
        let mut un = n.to_biguint().ok_or(anyhow!("have {}, can only encode positive integer", n))?;
        let mut i = bits;

        while i > 0 {
            self.apply_bit((BigUint::one() & &un) == BigUint::zero());
            un >>= 1;
            i -= 1;
        }

        if un != BigUint::zero() {
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

    fn check_align(&self, at: usize) -> Result<(), usize> {
        let total_bits = self.encoding_buf.len() * 8 + self.applying_bit;
        if total_bits % at == 0 {
            Ok(())
        }else {
            Err(at - total_bits % at)
        } 

    }

    fn padding(&mut self, align_at: usize){
        let total_bits = self.encoding_buf.len() * 8 + self.applying_bit;
        if total_bits % align_at == 0 {
            return;
        }

        let mut padding = align_at - total_bits % align_at;
        while padding > 0 {
            if padding >= 8 && self.applying_bit == 0 {
                //fast forward by pushing chars instead of bits
                let pad_bytes = padding / 8;
                padding -= pad_bytes * 8;
                self.encoding_buf.resize(self.encoding_buf.len()+ pad_bytes, 0u8);
            }else {
                self.apply_bit(true);
                padding -= 1;
            }
        }
    }

    fn encode_str(&mut self, s: &str) {
        self.encode_bytes(s.as_bytes())
    }
}

pub const AMOUNT_LEN: u32 = 5;

pub struct TxDataEncoder {
    ctx: Option<BitEncodeContext>,
    pub account_bits: u32,
    pub token_bits: u32,
    pub order_bits: u32,
    tx_encode_bits: usize,
}

impl TxDataEncoder {
    pub fn new(balance_levels: u32, order_levels: u32, account_levels: u32) -> Self {
        let mut ret = TxDataEncoder {
            ctx: Some(BitEncodeContext::new()),
            account_bits: account_levels,
            token_bits: balance_levels,
            order_bits: order_levels,
            tx_encode_bits: 0,
        };
        ret.tx_encode_bits = ret.data_bits();
        ret
    }

    pub fn reset(&mut self) {
        self.ctx.replace(BitEncodeContext::new());
    }

    pub fn pubdata_len_bits(&self) -> u32 {
        self.tx_encode_bits as u32
    }

    pub fn encode_account(&mut self, account_id: u32) -> Result<()> {
        self.ctx.as_mut().unwrap().encode_primint(account_id, self.account_bits)
    }

    pub fn encode_token(&mut self, token_id: u32) -> Result<()> {
        self.ctx.as_mut().unwrap().encode_primint(token_id, self.token_bits)
    }

    pub fn encode_order(&mut self, order_id: u32) -> Result<()> {
        self.ctx.as_mut().unwrap().encode_primint(order_id, self.order_bits)
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
        if bits < 64 {
            self.ctx.as_mut().unwrap().encode_primint(fr.to_i64(), bits)
        }else {
            self.ctx.as_mut().unwrap().encode_big(fr.to_bigint(), bits)
        }
        
    }

    pub fn encode_heading(&mut self, head: u32) -> Result<()> {
        self.ctx.as_ref().unwrap().check_align(self.tx_encode_bits).map_err(|n|anyhow!("not align: miss {} bits", n))?;
        self.ctx.as_mut().unwrap().encode_primint(head, 3)
    }

    pub fn encode_padding(&mut self) {
        self.ctx.as_mut().unwrap().padding(self.tx_encode_bits);
    }

    //finish encoding, output the result hash, and prepare for next encoding
    pub fn finish(&mut self) -> U256 {
        let encoded_bytes = self.ctx.replace(BitEncodeContext::new()).unwrap().seal();
        //        println!("{:02x?}", &encoded_bytes);
        U256::from_big_endian(&sha2::Sha256::digest(&encoded_bytes))
    }
}

pub trait EncodingParam {
    fn data_bits(&self) -> usize;
}

pub trait EncodeForPubData {
    fn encode_pubdata(&self, encoder: &mut TxDataEncoder) -> Result<()>;
}

impl EncodeForPubData for NopTx {
    fn encode_pubdata(&self, encoder: &mut TxDataEncoder) -> Result<()> {
        encoder.encode_heading(0)?;
        encoder.encode_padding();
        Ok(())
    }
}

impl EncodeForPubData for UpdateKeyTx {
    fn encode_pubdata(&self, encoder: &mut TxDataEncoder) -> Result<()> {
        encoder.encode_heading(1)?;
        encoder.encode_account(self.account_id)?;
        encoder.encode_fr(&self.l2key.sign, 1)?;
        encoder.encode_fr(&self.l2key.ay, 254)?;
        encoder.encode_padding();
        Ok(())        
    }
}


impl EncodeForPubData for DepositTx {
    fn encode_pubdata(&self, encoder: &mut TxDataEncoder) -> Result<()> {
        assert!(self.l2key.is_none());
        encoder.encode_heading(0)?;
        encoder.encode_account(self.account_id)?;
        encoder.encode_account(self.account_id)?;
        encoder.encode_token(self.token_id)?;
        encoder.encode_token(self.token_id)?;
        encoder.encode_amount(&self.amount)?;
        encoder.encode_padding();
        Ok(())        
    }
}

impl EncodeForPubData for TransferTx {
    fn encode_pubdata(&self, encoder: &mut TxDataEncoder) -> Result<()> {
        encoder.encode_heading(0)?;
        encoder.encode_account(self.from)?;
        encoder.encode_account(self.to)?;
        encoder.encode_token(self.token_id)?;
        encoder.encode_token(self.token_id)?;
        encoder.encode_amount(&self.amount)?;
        encoder.encode_padding();
        Ok(())        
    }
}

impl EncodeForPubData for FullSpotTradeTx {
    fn encode_pubdata(&self, encoder: &mut TxDataEncoder) -> Result<()> {
        let order1 = self.maker_order.as_ref().unwrap();
        let order2 = self.taker_order.as_ref().unwrap();
        let trade = &self.trade;
        let mut h = 0;
        //order 1
        h += if order1.is_filled() {2} else {0};
        h += if order2.is_filled() {4} else {0};
        encoder.encode_heading(h)?;
        encoder.encode_account(trade.order1_account_id)?;
        encoder.encode_account(trade.order2_account_id)?;
        encoder.encode_token(trade.token_id_1to2)?;
        encoder.encode_token(trade.token_id_2to1)?;
        encoder.encode_fr(&order1.total_sell, AMOUNT_LEN * 8)?;
        encoder.encode_fr(&order1.total_buy, AMOUNT_LEN * 8)?;
        encoder.encode_order(order1.order_id)?;
        encoder.encode_fr(&order2.total_sell, AMOUNT_LEN * 8)?;
        encoder.encode_fr(&order2.total_buy, AMOUNT_LEN * 8)?;
        encoder.encode_order(order2.order_id)?;
        encoder.encode_padding();
        Ok(())
    }
}

impl EncodeForPubData for WithdrawTx {
    fn encode_pubdata(&self, encoder: &mut TxDataEncoder) -> Result<()> {
        encoder.encode_heading(4)?; //001
        encoder.encode_account(self.account_id)?;
        encoder.encode_account(self.account_id)?;
        encoder.encode_token(self.token_id)?;
        encoder.encode_token(self.token_id)?;
        encoder.encode_amount(&self.amount)?;
        encoder.encode_padding();
        Ok(())        
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
    //testcase is picked from circuit's block testcase
    let mut tx_encoder = TxDataEncoder::new(2, 2, 2);

    //block empty
    let tx_nop = NopTx {};
    tx_nop.encode_pubdata(&mut tx_encoder).unwrap();
    tx_nop.encode_pubdata(&mut tx_encoder).unwrap();

    let hash = tx_encoder.finish();
    assert_eq!(hash.low_u128(), 13986966260155268189165613960437146670u128);

    //block 0
    let tx = UpdateKeyTx {
        account_id: 0,
        l2key: L2Key {
            eth_addr: Fr::zero(),
            sign: Fr::one(),
            ay: Fr::from_str("20929899733237450167431708044227754871358144348193832508253740860573780197290"),
        },
    };

    tx.encode_pubdata(&mut tx_encoder).unwrap();

    let tx = DepositTx {
        account_id: 0,
        token_id: 0,
        amount: AmountType{
            exponent: 2u8,
            significand: 2,
        }, //200
        l2key: None,
    };

    tx.encode_pubdata(&mut tx_encoder).unwrap();
    let hash = tx_encoder.finish();
    assert_eq!(hash.low_u128(), 23985885059632108908906696405152872466u128);

    //block 1
    let tx = DepositTx {
        account_id: 1,
        token_id: 0,
        amount: AmountType{
            exponent: 2u8,
            significand: 1,
        }, //100,
        l2key: None,
    };

    tx.encode_pubdata(&mut tx_encoder).unwrap();

    let tx = TransferTx {
        from: 1,
        to: 0,
        token_id: 0,
        amount: AmountType{
            exponent: 1u8,
            significand: 5,
        }, //50
        l2key: None,
        from_nonce: Fr::zero(),
        sig: Signature::default(),
    };

    tx.encode_pubdata(&mut tx_encoder).unwrap();

    let hash = tx_encoder.finish();
    assert_eq!(hash.low_u128(), 263734781515133552465673106314305164970u128);

    //block 2
    let tx = WithdrawTx {
        account_id: 0,
        token_id: 0,
        amount: AmountType{
            exponent: 1u8,
            significand: 15,
        }, //150
        nonce: Fr::zero(),
        old_balance: Fr::zero(),
        sig: Signature::default(),
    };

    tx.encode_pubdata(&mut tx_encoder).unwrap();

    let tx = DepositTx {
        account_id: 1,
        token_id: 0,
        amount: AmountType{
            exponent: 0u8,
            significand: 199,
        },
        l2key: None,
    };

    tx.encode_pubdata(&mut tx_encoder).unwrap();

    let hash = tx_encoder.finish();
    assert_eq!(hash.low_u128(), 113591718068163649357370944225903641747u128);

    //block 3
    let tx = DepositTx {
        account_id: 2,
        token_id: 1,
        amount: AmountType{
            exponent: 1u8,
            significand: 199,
        }, //1990
        l2key: None,
    };

    tx.encode_pubdata(&mut tx_encoder).unwrap();

    let mut mk_order = order::Order::default();
    let mut tk_order = order::Order::default();

    mk_order.order_id = 1;
    mk_order.token_buy = Fr::from_u32(1);
    mk_order.filled_buy = Fr::from_u32(1200);
    mk_order.filled_sell = Fr::from_u32(120);
    let amt = AmountType{
        exponent: 4u8,
        significand: 1,
    }; // 10000
    mk_order.total_buy = Fr::from_bigint(amt.to_encoded_int().unwrap());
    let amt = AmountType{
        exponent: 3u8,
        significand: 1,
    }; // 1000
    mk_order.total_sell = Fr::from_bigint(amt.to_encoded_int().unwrap());

    tk_order.order_id = 1;
    tk_order.token_sell = Fr::from_u32(1);
    tk_order.filled_sell = Fr::from_u32(1210);
    tk_order.filled_buy = Fr::from_u32(121);
    let amt = AmountType{
        exponent: 0u8,
        significand: 121,
    };//121
    tk_order.total_buy = Fr::from_bigint(amt.to_encoded_int().unwrap());
    let amt = AmountType{
        exponent: 1u8,
        significand: 121,
    };//1210
    tk_order.total_sell = Fr::from_bigint(amt.to_encoded_int().unwrap());

    let tx = FullSpotTradeTx {
        trade: SpotTradeTx {
            order1_account_id: 1,
            order2_account_id: 2,
            token_id_1to2: 0,
            token_id_2to1: 1,
            amount_1to2: Fr::from_u32(120),
            amount_2to1: Fr::from_u32(1200),
            order1_id: 1,
            order2_id: 1,            
        },
        maker_order: Some(mk_order),
        taker_order: Some(tk_order),
    };

    tx.encode_pubdata(&mut tx_encoder).unwrap();

    let hash = tx_encoder.finish();
    assert_eq!(hash.low_u128(), 146266009004152019134474450994520485822u128);

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
