// Generated from tpl/ejs/extra/rollup-state-manager/src/types/l2/tx_encode.rs.ejs. Don't modify this file manually
#![allow(clippy::identity_op)]
use super::{tx_detail_idx, EncodingParam, RawTx, TxDataEncoder};
use anyhow::Result;

impl EncodingParam for TxDataEncoder {
    fn data_bits(&self) -> usize {
        let mut ret = 0;
        let scheme_len = self.account_bits * 2 + self.token_bits * 2 + 40 * 1;
        ret = if ret < scheme_len { scheme_len } else { ret };
        let scheme_len = 32 * 2 + self.account_bits * 2 + self.token_bits * 2 + 40 * 4 + self.order_bits * 2;
        ret = if ret < scheme_len { scheme_len } else { ret };
        let scheme_len = 1 * 1 + 254 * 1 + self.account_bits * 1;
        ret = if ret < scheme_len { scheme_len } else { ret };

        ret += 3;
        if ret % 8 == 0 {
            ret as usize
        } else {
            (ret + 8 - (ret % 8)) as usize
        }
    }
}

pub trait EncodeForScheme {
    fn encode(self, encoder: &mut TxDataEncoder) -> Result<()>;
}

pub struct ForCommonTx<'d>(pub &'d RawTx);

impl EncodeForScheme for ForCommonTx<'_> {
    fn encode(self, encoder: &mut TxDataEncoder) -> Result<()> {
        let payload = &self.0.payload;
        encoder.encode_fr(&payload[tx_detail_idx::ACCOUNT_ID1], encoder.account_bits)?;
        encoder.encode_fr(&payload[tx_detail_idx::ACCOUNT_ID2], encoder.account_bits)?;
        encoder.encode_fr(&payload[tx_detail_idx::TOKEN_ID1], encoder.token_bits)?;
        encoder.encode_fr(&payload[tx_detail_idx::TOKEN_ID2], encoder.token_bits)?;
        encoder.encode_fr(&payload[tx_detail_idx::AMOUNT], 40)?;

        Ok(())
    }
}

pub struct ForSpotTradeTx<'d>(pub &'d RawTx);

impl EncodeForScheme for ForSpotTradeTx<'_> {
    fn encode(self, encoder: &mut TxDataEncoder) -> Result<()> {
        let payload = &self.0.payload;
        encoder.encode_fr(&payload[tx_detail_idx::ACCOUNT_ID1], encoder.account_bits)?;
        encoder.encode_fr(&payload[tx_detail_idx::ACCOUNT_ID2], encoder.account_bits)?;
        encoder.encode_fr(&payload[tx_detail_idx::NEW_ORDER1_TOKEN_SELL], encoder.token_bits)?;
        encoder.encode_fr(&payload[tx_detail_idx::NEW_ORDER2_TOKEN_SELL], encoder.token_bits)?;
        encoder.encode_fr(&payload[tx_detail_idx::NEW_ORDER1_AMOUNT_SELL], 40)?;
        encoder.encode_fr(&payload[tx_detail_idx::NEW_ORDER1_AMOUNT_BUY], 40)?;
        encoder.encode_fr(&payload[tx_detail_idx::ORDER1_POS], encoder.order_bits)?;
        encoder.encode_fr(&payload[tx_detail_idx::NEW_ORDER1_ID], 32).ok();
        encoder.encode_fr(&payload[tx_detail_idx::NEW_ORDER2_AMOUNT_SELL], 40)?;
        encoder.encode_fr(&payload[tx_detail_idx::NEW_ORDER2_AMOUNT_BUY], 40)?;
        encoder.encode_fr(&payload[tx_detail_idx::ORDER2_POS], encoder.order_bits)?;
        encoder.encode_fr(&payload[tx_detail_idx::NEW_ORDER2_ID], 32).ok();

        Ok(())
    }
}

pub struct ForL2KeyTx<'d>(pub &'d RawTx);

impl EncodeForScheme for ForL2KeyTx<'_> {
    fn encode(self, encoder: &mut TxDataEncoder) -> Result<()> {
        let payload = &self.0.payload;
        encoder.encode_fr(&payload[tx_detail_idx::ACCOUNT_ID1], encoder.account_bits)?;
        encoder.encode_fr(&payload[tx_detail_idx::SIGN2], 1)?;
        encoder.encode_fr(&payload[tx_detail_idx::AY2], 254)?;

        Ok(())
    }
}
