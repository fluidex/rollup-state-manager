#![allow(clippy::upper_case_acronyms)]
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// TODO: reuse related types def in dingir-exchange

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum MarketRole {
    MAKER = 1,
    TAKER = 2,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum OrderSide {
    ASK,
    BID,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum OrderType {
    LIMIT,
    MARKET,
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub enum OrderEventType {
    PUT = 1,
    UPDATE = 2,
    FINISH = 3,
    EXPIRED = 4,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: u64,
    pub market: String,
    #[serde(rename = "type")]
    pub type_: OrderType, // enum
    pub side: OrderSide,
    pub user: u32,
    pub create_time: f64,
    pub update_time: f64,
    pub price: Decimal,
    pub amount: Decimal,
    pub taker_fee: Decimal,
    pub maker_fee: Decimal,
    pub remain: Decimal,
    pub frozen: Decimal,
    pub finished_base: Decimal,
    pub finished_quote: Decimal,
    pub finished_fee: Decimal,
    // TODO: remove Option once migration is done
    pub signature: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct OrderMessage {
    pub event: OrderEventType,
    pub order: Order,
    pub base: String,
    pub quote: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct VerboseOrderState {
    pub price: Decimal,
    pub amount: Decimal,
    pub finished_base: Decimal,
    pub finished_quote: Decimal,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct VerboseBalanceState {
    pub bid_user_base: Decimal,
    pub bid_user_quote: Decimal,
    pub ask_user_base: Decimal,
    pub ask_user_quote: Decimal,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct VerboseTradeState {
    // emit all the related state
    pub ask_order_state: VerboseOrderState,
    pub bid_order_state: VerboseOrderState,
    pub balance: VerboseBalanceState,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TradeMessage {
    pub id: u64,
    pub timestamp: f64, // unix epoch timestamp,
    pub market: String,
    pub base: String,
    pub quote: String,
    pub price: Decimal,
    pub amount: Decimal,
    pub quote_amount: Decimal,

    pub ask_user_id: u32,
    pub ask_order_id: u64,
    pub ask_role: MarketRole, // take/make
    pub ask_fee: Decimal,

    pub bid_user_id: u32,
    pub bid_order_id: u64,
    pub bid_role: MarketRole,
    pub bid_fee: Decimal,

    pub bid_order: Option<Order>,
    pub ask_order: Option<Order>,
    pub state_before: Option<VerboseTradeState>,
    pub state_after: Option<VerboseTradeState>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct BalanceMessage {
    pub timestamp: f64,
    pub user_id: u32,
    pub asset: String,
    pub business: String,
    pub change: Decimal,
    pub balance: Decimal,
    pub detail: String,
}
