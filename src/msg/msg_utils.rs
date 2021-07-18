#![allow(clippy::let_and_return)]
use crate::account::SignatureBJJ;
use crate::state::WitnessGenerator;
use crate::test_utils::types::{get_token_id_by_name, prec_token_id};
use crate::types::l2::{self, OrderSide};
use crate::types::matchengine::{self, messages};
use fluidex_common::babyjubjub_rs;
use fluidex_common::rust_decimal::Decimal;
use fluidex_common::types::{DecimalExt, FrExt};
use fluidex_common::Fr;
use num::Zero;
use std::convert::TryInto;

#[derive(Clone, Copy)]
pub struct TokenIdPair(pub u32, pub u32);
#[derive(Clone, Copy)]
pub struct TokenPair<'c>(pub &'c str, pub &'c str);

impl<'c> From<&'c str> for TokenPair<'c> {
    fn from(origin: &'c str) -> Self {
        let mut assets = origin.split('_');
        let asset_1 = assets.next().unwrap();
        let asset_2 = assets.next().unwrap();
        TokenPair(asset_1, asset_2)
    }
}

impl<'c> From<TokenPair<'c>> for TokenIdPair {
    fn from(origin: TokenPair<'c>) -> Self {
        TokenIdPair(get_token_id_by_name(origin.0), get_token_id_by_name(origin.1))
    }
}

impl From<String> for TokenIdPair {
    fn from(origin: String) -> Self {
        let mut assets = origin.split('_');
        let base = assets.next().unwrap();
        let quote = assets.next().unwrap();
        TokenIdPair(get_token_id_by_name(base), get_token_id_by_name(quote))
    }
}

pub fn string_to_sig(signature: String) -> SignatureBJJ {
    if signature.is_empty() {
        panic!("empty signature");
    }

    let sig_packed_vec = hex::decode(&signature).unwrap();
    babyjubjub_rs::decompress_signature(&sig_packed_vec.try_into().unwrap()).unwrap()
}

pub fn bytes_to_sig(signature: [u8; 64]) -> SignatureBJJ {
    if signature == [0; 64] {
        panic!("empty signature");
    }
    //println!("SignatureBJJ {:?}", signature);
    babyjubjub_rs::decompress_signature(&signature).unwrap()
}

pub fn exchange_order_to_rollup_order(origin: &matchengine::messages::Order) -> l2::OrderInput {
    assert!(origin.finished_base.is_zero());
    assert!(origin.finished_quote.is_zero());
    let TokenIdPair(base_token_id, quote_token_id) = origin.market.clone().into();
    let base_prec = prec_token_id(base_token_id);
    let quote_prec = prec_token_id(quote_token_id);
    match origin.side {
        matchengine::messages::OrderSide::ASK => {
            l2::OrderInput {
                order_id: origin.id as u32,
                token_buy: Fr::from_u32(quote_token_id),
                token_sell: Fr::from_u32(base_token_id),
                //filled_sell: fixnum::decimal_to_fr(&origin.finished_base, base_token_id),
                //filled_buy: fixnum::decimal_to_fr(&origin.finished_quote, quote_token_id),
                total_sell: origin.amount.to_fr(base_prec),
                total_buy: (origin.amount * origin.price).to_fr(quote_prec),
                sig: Some(bytes_to_sig(origin.signature)),
                account_id: origin.user,
                side: OrderSide::Sell,
            }
        }
        matchengine::messages::OrderSide::BID => {
            l2::OrderInput {
                order_id: origin.id as u32,
                token_buy: Fr::from_u32(base_token_id),
                token_sell: Fr::from_u32(quote_token_id),
                //filled_sell: fixnum::decimal_to_fr(&origin.finished_quote, quote_token_id),
                //filled_buy: fixnum::decimal_to_fr(&origin.finished_base, base_token_id),
                total_sell: (origin.amount * origin.price).to_fr(quote_prec),
                total_buy: origin.amount.to_fr(base_prec),
                sig: Some(bytes_to_sig(origin.signature)),
                account_id: origin.user,
                side: OrderSide::Buy,
            }
        }
    }
}
pub fn check_state(witgen: &WitnessGenerator, state: &messages::VerboseTradeState, trade: &messages::TradeMessage) {
    let token_pair = TokenPair::from(trade.market.as_str());
    let TokenIdPair(base_token_id, quote_token_id) = TokenIdPair::from(token_pair);
    for balance_state in &state.balance_states {
        // assert_balance_state(&state.balance, witgen, trade.bid_user_id, trade.ask_user_id, id_pair);
        let balance_remote = balance_state.balance;
        let token_id = get_token_id_by_name(&balance_state.asset);
        let balance_local = witgen
            .get_token_balance(balance_state.user_id, token_id)
            .to_decimal(prec_token_id(token_id));
        assert_eq!(
            balance_remote, balance_local,
            "uid {} token {} remote balance {} local balance {}",
            balance_state.user_id, token_id, balance_remote, balance_local
        );
    }
    for order_state in &state.order_states {
        let account_id = order_state.user_id;
        let order_id = order_state.order_id as u32;
        if witgen.has_order(account_id, order_id) {
            let order_local = witgen.get_account_order_by_id(account_id, order_id);
            match order_state.order_side {
                messages::OrderSide::BID => {
                    let remote_filled_buy = order_state.finished_base;
                    let remote_filled_sell = order_state.finished_quote;
                    let local_filled_buy = order_local.filled_buy.to_decimal(prec_token_id(base_token_id));
                    let local_filled_sell = order_local.filled_sell.to_decimal(prec_token_id(quote_token_id));
                    assert_eq!(remote_filled_buy, local_filled_buy);
                    assert_eq!(remote_filled_sell, local_filled_sell);
                }
                messages::OrderSide::ASK => {
                    let remote_filled_buy = order_state.finished_quote;
                    let remote_filled_sell = order_state.finished_base;
                    let local_filled_buy = order_local.filled_buy.to_decimal(prec_token_id(quote_token_id));
                    let local_filled_sell = order_local.filled_sell.to_decimal(prec_token_id(base_token_id));
                    assert_eq!(remote_filled_buy, local_filled_buy);
                    assert_eq!(remote_filled_sell, local_filled_sell);
                }
            }
        } else {
            // the only possible path reaching here, is that the order is a new order in 'state_before'
            // so it is unknown for witgen
            assert_eq!(order_state.finished_base, Decimal::zero(), "{:?}", order_state);
            assert_eq!(order_state.finished_quote, Decimal::zero(), "{:?}", order_state);
        }
    }
}
