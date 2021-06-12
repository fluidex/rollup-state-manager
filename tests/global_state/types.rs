#![allow(dead_code)]
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::large_enum_variant)]

use rollup_state_manager::account::Signature;
use rollup_state_manager::state::WitnessGenerator;
use rollup_state_manager::test_utils::types::{get_token_id_by_name, prec_token_id};
use rollup_state_manager::types;
use rollup_state_manager::types::fixnum;
use rollup_state_manager::types::l2::OrderSide;
use rollup_state_manager::types::primitives::fr_to_decimal;
use rust_decimal::Decimal;

#[derive(Clone, Copy)]
pub struct TokenIdPair(pub u32, pub u32);
/*
impl TokenIdPair {
    fn swap(&mut self) {
        let tmp = self.1;
        self.1 = self.0;
        self.0 = tmp;
    }
}
*/
#[derive(Clone, Copy)]
pub struct TokenPair<'c>(pub &'c str, pub &'c str);

pub struct OrderState<'c> {
    pub origin: &'c types::matchengine::messages::VerboseOrderState,
    pub side: &'static str,
    pub token_sell: u32,
    pub token_buy: u32,
    pub total_sell: Decimal,
    pub total_buy: Decimal,
    pub filled_sell: Decimal,
    pub filled_buy: Decimal,

    pub order_id: u32,
    pub account_id: u32,
    pub role: types::matchengine::messages::MarketRole,
}

struct OrderStateTag {
    id: u64,
    account_id: u32,
    role: types::matchengine::messages::MarketRole,
}

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

impl<'c> OrderState<'c> {
    pub fn parse(
        origin: &'c types::matchengine::messages::VerboseOrderState,
        id_pair: TokenIdPair,
        _token_pair: TokenPair<'c>,
        side: &'static str,
        trade: &types::matchengine::messages::TradeMessage,
    ) -> Self {
        match side {
            "ASK" => OrderState {
                origin,
                side,
                //status: 0,
                token_sell: id_pair.0,
                token_buy: id_pair.1,
                total_sell: origin.amount,
                total_buy: origin.amount * origin.price,
                filled_sell: origin.finished_base,
                filled_buy: origin.finished_quote,
                order_id: trade.ask_order_id as u32,
                account_id: trade.ask_user_id,
                role: trade.ask_role,
            },
            "BID" => OrderState {
                origin,
                side,
                //status: 0,
                token_sell: id_pair.1,
                token_buy: id_pair.0,
                total_sell: origin.amount * origin.price,
                total_buy: origin.amount,
                filled_sell: origin.finished_quote,
                filled_buy: origin.finished_base,
                order_id: trade.bid_order_id as u32,
                account_id: trade.bid_user_id,
                role: trade.bid_role,
            },
            _ => unreachable!(),
        }
    }
}

impl<'c> From<OrderState<'c>> for types::l2::Order {
    fn from(origin: OrderState<'c>) -> Self {
        types::l2::Order {
            order_id: (origin.order_id),
            //status: types::primitives::u32_to_fr(origin.status),
            tokenbuy: types::primitives::u32_to_fr(origin.token_buy),
            tokensell: types::primitives::u32_to_fr(origin.token_sell),
            filled_sell: fixnum::decimal_to_amount(&origin.filled_sell, prec_token_id(origin.token_sell)).to_fr(),
            filled_buy: fixnum::decimal_to_amount(&origin.filled_buy, prec_token_id(origin.token_buy)).to_fr(),
            total_sell: fixnum::decimal_to_amount(&origin.total_sell, prec_token_id(origin.token_sell)).to_fr(),
            total_buy: fixnum::decimal_to_amount(&origin.total_buy, prec_token_id(origin.token_buy)).to_fr(),
            sig: Signature::default(),
            account_id: origin.account_id,
            side: if origin.side.to_lowercase() == "buy" || origin.side.to_lowercase() == "bid" {
                OrderSide::Buy
            } else {
                OrderSide::Sell
            },
        }
    }
}

impl<'c> std::cmp::PartialOrd for OrderState<'c> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<'c> std::cmp::PartialEq for OrderState<'c> {
    fn eq(&self, other: &Self) -> bool {
        self.order_id == other.order_id
    }
}

impl<'c> std::cmp::Eq for OrderState<'c> {}

impl<'c> std::cmp::Ord for OrderState<'c> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.order_id.cmp(&other.order_id)
    }
}

#[derive(PartialEq, Debug)]
struct CommonBalanceState {
    /*
    bid_user_base: types::primitives::Fr,
    bid_user_quote: types::primitives::Fr,
    ask_user_base: types::primitives::Fr,
    ask_user_quote: types::primitives::Fr,
    */
    bid_user_base: Decimal,
    bid_user_quote: Decimal,
    ask_user_base: Decimal,
    ask_user_quote: Decimal,
}

impl CommonBalanceState {
    fn parse(origin: &types::matchengine::messages::VerboseBalanceState, _id_pair: TokenIdPair) -> Self {
        //let base_id = id_pair.0;
        //let quote_id = id_pair.1;

        CommonBalanceState {
            bid_user_base: origin.bid_user_base,
            bid_user_quote: origin.bid_user_quote,
            ask_user_base: origin.ask_user_base,
            ask_user_quote: origin.ask_user_quote,
            /*
            bid_user_base: fixnum::decimal_to_amount(&origin.bid_user_base, prec_token_id(base_id)).to_fr(),
            bid_user_quote: fixnum::decimal_to_amount(&origin.bid_user_quote, prec_token_id(quote_id)).to_fr(),
            ask_user_base: fixnum::decimal_to_amount(&origin.ask_user_base, prec_token_id(base_id)).to_fr(),
            ask_user_quote: fixnum::decimal_to_amount(&origin.ask_user_quote, prec_token_id(quote_id)).to_fr(),
            */
        }
    }

    fn build_local(witgen: &WitnessGenerator, bid_id: u32, ask_id: u32, id_pair: TokenIdPair) -> Self {
        let base_id = id_pair.0;
        let quote_id = id_pair.1;

        CommonBalanceState {
            bid_user_base: fr_to_decimal(&witgen.get_token_balance(bid_id, base_id), prec_token_id(base_id)),
            bid_user_quote: fr_to_decimal(&witgen.get_token_balance(bid_id, quote_id), prec_token_id(quote_id)),
            ask_user_base: fr_to_decimal(&witgen.get_token_balance(ask_id, base_id), prec_token_id(base_id)),
            ask_user_quote: fr_to_decimal(&witgen.get_token_balance(ask_id, quote_id), prec_token_id(quote_id)),
            /*
            bid_user_base: witgen.get_token_balance(bid_id, base_id),
            bid_user_quote: witgen.get_token_balance(bid_id, quote_id),
            ask_user_base: witgen.get_token_balance(ask_id, base_id),
            ask_user_quote: witgen.get_token_balance(ask_id, quote_id),
            */
        }
    }
}

pub fn assert_balance_state(
    balance_state: &types::matchengine::messages::VerboseBalanceState,
    witgen: &WitnessGenerator,
    bid_id: u32,
    ask_id: u32,
    id_pair: TokenIdPair,
) {
    let local_balance = CommonBalanceState::build_local(witgen, bid_id, ask_id, id_pair);
    let parsed_state = CommonBalanceState::parse(balance_state, id_pair);
    assert_eq!(local_balance, parsed_state);
}
