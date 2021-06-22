use crate::account::Signature;
use crate::state::WitnessGenerator;
use crate::test_utils::types::{get_token_id_by_name, prec_token_id};
use crate::types::l2::{self, OrderSide};
use crate::types::primitives::{fr_to_decimal, str_to_fr, u32_to_fr};
use crate::types::{self, fixnum, matchengine};
use num::Zero;
use rust_decimal::Decimal;

#[derive(Clone, Copy)]
pub struct TokenIdPair(pub u32, pub u32);
#[derive(Clone, Copy)]
pub struct TokenPair<'c>(pub &'c str, pub &'c str);

pub struct OrderState {
    pub side: &'static str,
    pub token_sell: u32,
    pub token_buy: u32,
    pub total_sell: Decimal,
    pub total_buy: Decimal,
    pub filled_sell: Decimal,
    pub filled_buy: Decimal,

    pub order_id: u32,
    pub account_id: u32,
    pub role: matchengine::messages::MarketRole,
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

impl From<String> for TokenIdPair {
    fn from(origin: String) -> Self {
        let mut assets = origin.split('_');
        let base = assets.next().unwrap();
        let quote = assets.next().unwrap();
        TokenIdPair(get_token_id_by_name(base), get_token_id_by_name(quote))
    }
}

fn hash_order(order: &crate::types::matchengine::messages::Order) -> String {
    unimplemented!()
}

// TODO: opt lifetime?
use std::convert::TryInto;
impl<'c> From<&'c matchengine::messages::Order> for crate::account::Signature {
    fn from(order: &'c matchengine::messages::Order) -> Self {
        let order_hash = hash_order(order);

        let sig_packed_vec = hex::decode(&order.signature).unwrap();
        let sig_unpacked = babyjubjub_rs::decompress_signature(&sig_packed_vec.try_into().unwrap()).unwrap();


        // safe
        // let b = self.priv_key.sign(fr_to_bigint(&hash))?.compress();
        // let r_b8_bytes: [u8; 32] = *array_ref!(b[..32], 0, 32);
        // let s = bigint_to_fr(BigInt::from_bytes_le(num_bigint::Sign::Plus, &b[32..]));
        // let r_b8 = decompress_point(r_b8_bytes);
        // match r_b8 {
        //     Result::Err(err) => Err(err),
        //     Result::Ok(Point { x: r8x, y: r8y }) => Ok(Signature { hash, s, r8x, r8y }),
        // }

        // unsafe
        // let sig_orig: babyjubjub_rs::Signature = self.priv_key.sign(fr_to_bigint(&hash))?;
        // let sig: SignatureBJJ = unsafe { std::mem::transmute::<babyjubjub_rs::Signature, SignatureBJJ>(sig_orig) };
        // let s = bigint_to_fr(sig.s);
        // Ok(Signature {
        //     hash,
        //     s,
        //     r8x: sig.r_b8.x,
        //     r8y: sig.r_b8.y,
        // })



        Self {
            hash: str_to_fr(&order_hash),
        }
    }
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
                token_buy: types::primitives::u32_to_fr(quote_token_id),
                token_sell: types::primitives::u32_to_fr(base_token_id),
                //filled_sell: fixnum::decimal_to_amount(&origin.finished_base, base_token_id).to_fr(),
                //filled_buy: fixnum::decimal_to_amount(&origin.finished_quote, quote_token_id).to_fr(),
                total_sell: fixnum::decimal_to_amount(&origin.amount, base_prec).to_fr(),
                total_buy: fixnum::decimal_to_amount(&(origin.amount * origin.price), quote_prec).to_fr(),
                // sig: Signature::default(),
                sig: origin.into(),
                account_id: origin.user,
                side: OrderSide::Sell,
            }
        }
        matchengine::messages::OrderSide::BID => {
            l2::OrderInput {
                order_id: origin.id as u32,
                token_buy: types::primitives::u32_to_fr(base_token_id),
                token_sell: types::primitives::u32_to_fr(quote_token_id),
                //filled_sell: fixnum::decimal_to_amount(&origin.finished_quote, quote_token_id).to_fr(),
                //filled_buy: fixnum::decimal_to_amount(&origin.finished_base, base_token_id).to_fr(),
                total_sell: fixnum::decimal_to_amount(&(origin.amount * origin.price), quote_prec).to_fr(),
                total_buy: fixnum::decimal_to_amount(&origin.amount, base_prec).to_fr(),
                // sig: Signature::default(),
                sig: origin.into(),
                account_id: origin.user,
                side: OrderSide::Buy,
            }
        }
    }
}

pub fn trade_to_order_state(
    state: &matchengine::messages::VerboseTradeState,
    trade: &matchengine::messages::TradeMessage,
) -> (OrderState, OrderState) {
    // ASK, BID
    let ask = &state.ask_order_state;
    let bid = &state.bid_order_state;
    let id_pair = TokenIdPair::from(TokenPair::from(trade.market.as_str()));
    (
        OrderState {
            side: "ASK",
            token_sell: id_pair.0,
            token_buy: id_pair.1,
            total_sell: ask.amount,
            total_buy: ask.amount * ask.price,
            filled_sell: ask.finished_base,
            filled_buy: ask.finished_quote,
            order_id: trade.ask_order_id as u32,
            account_id: trade.ask_user_id,
            role: trade.ask_role,
        },
        OrderState {
            side: "BID",
            token_sell: id_pair.1,
            token_buy: id_pair.0,
            total_sell: bid.amount * bid.price,
            total_buy: bid.amount,
            filled_sell: bid.finished_quote,
            filled_buy: bid.finished_base,
            order_id: trade.bid_order_id as u32,
            account_id: trade.bid_user_id,
            role: trade.bid_role,
        },
    )
}

impl OrderState {
    pub fn is_empty(&self) -> bool {
        // they should be both zero or both non-zero
        self.filled_buy.is_zero() && self.filled_sell.is_zero()
    }
    pub fn parse(
        origin: &matchengine::messages::VerboseOrderState,
        id_pair: TokenIdPair,
        side: &'static str,
        trade: &matchengine::messages::TradeMessage,
    ) -> Self {
        match side {
            "ASK" => OrderState {
                //origin,
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
                //origin,
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

impl From<OrderState> for l2::Order {
    fn from(origin: OrderState) -> Self {
        l2::Order {
            order_id: origin.order_id,
            //status: types::primitives::u32_to_fr(origin.status),
            token_buy: types::primitives::u32_to_fr(origin.token_buy),
            token_sell: types::primitives::u32_to_fr(origin.token_sell),
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

impl From<OrderState> for l2::OrderInput {
    fn from(order_state: OrderState) -> Self {
        l2::OrderInput {
            order_id: order_state.order_id,
            token_sell: u32_to_fr(order_state.token_sell),
            token_buy: u32_to_fr(order_state.token_buy),
            total_sell: fixnum::decimal_to_amount(&order_state.total_sell, prec_token_id(order_state.token_sell)).to_fr(),
            total_buy: fixnum::decimal_to_amount(&order_state.total_buy, prec_token_id(order_state.token_buy)).to_fr(),
            sig: Signature::default(),
            account_id: order_state.account_id,
            side: if order_state.side.to_lowercase() == "buy" || order_state.side.to_lowercase() == "bid" {
                OrderSide::Buy
            } else {
                OrderSide::Sell
            },
        }
    }
}

impl std::cmp::PartialOrd for OrderState {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::cmp::PartialEq for OrderState {
    fn eq(&self, other: &Self) -> bool {
        self.order_id == other.order_id
    }
}

impl std::cmp::Eq for OrderState {}

impl std::cmp::Ord for OrderState {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.order_id.cmp(&other.order_id)
    }
}

#[derive(PartialEq, Debug)]
struct CommonBalanceState {
    bid_user_base: Decimal,
    bid_user_quote: Decimal,
    ask_user_base: Decimal,
    ask_user_quote: Decimal,
}

impl CommonBalanceState {
    fn parse(origin: &matchengine::messages::VerboseBalanceState, _id_pair: TokenIdPair) -> Self {
        CommonBalanceState {
            bid_user_base: origin.bid_user_base,
            bid_user_quote: origin.bid_user_quote,
            ask_user_base: origin.ask_user_base,
            ask_user_quote: origin.ask_user_quote,
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
        }
    }
}

pub fn assert_balance_state(
    balance_state: &matchengine::messages::VerboseBalanceState,
    witgen: &WitnessGenerator,
    bid_id: u32,
    ask_id: u32,
    id_pair: TokenIdPair,
) {
    let local_balance = CommonBalanceState::build_local(witgen, bid_id, ask_id, id_pair);
    let parsed_state = CommonBalanceState::parse(balance_state, id_pair);
    assert_eq!(local_balance, parsed_state);
}

pub fn assert_order_state(witgen: &WitnessGenerator, order_state: OrderState) {
    if witgen.has_order(order_state.account_id, order_state.order_id) {
        let mut order_local = witgen.get_account_order_by_id(order_state.account_id, order_state.order_id);
        // TODO: compares the order field sig. The field sig is set to the default value of Signature for now.
        order_local.sig = Signature::default();
        assert_eq!(order_local, l2::Order::from(order_state));
    } else {
        // the only possible path reaching here, is that the order has not been put into witgen
    }
}
