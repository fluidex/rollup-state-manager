#![allow(clippy::upper_case_acronyms)]
use std::ops::{Deref, DerefMut};

use fluidex_common::rust_decimal::Decimal;
use fluidex_common::serde::HexArray;
use serde::{Deserialize, Serialize};

// TODO: reuse related types def in dingir-exchange
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message<T> {
    message: T,
    offset: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserMessage {
    pub user_id: u32,
    pub l1_address: String,
    pub l2_pubkey: String,
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub enum MarketRole {
    MAKER = 1,
    TAKER = 2,
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub enum OrderSide {
    ASK,
    BID,
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
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

#[derive(Clone, Serialize, Deserialize, Debug)]
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
    pub post_only: bool,

    #[serde(with = "HexArray")]
    pub signature: [u8; 64],
    // TODO: remove Option once migration is done
    //pub signature: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct OrderMessage {
    pub event: OrderEventType,
    pub order: Order,
    pub base: String,
    pub quote: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VerboseOrderState {
    pub user_id: u32,
    pub order_id: u64,
    pub order_side: OrderSide,
    pub finished_base: Decimal,
    pub finished_quote: Decimal,
    pub finished_fee: Decimal,
    //pub remain: Decimal,
    //pub frozen: Decimal,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VerboseBalanceState {
    pub user_id: u32,
    pub asset: String,
    // total = balance_available + balance_frozen
    pub balance: Decimal,
    //pub balance_available: Deimcal,
    //pub balance_frozen: Deimcal,
}

// TODO: rename this?
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct VerboseTradeState {
    // emit all the related state
    pub order_states: Vec<VerboseOrderState>,
    pub balance_states: Vec<VerboseBalanceState>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DepositMessage {
    pub timestamp: f64,
    pub user_id: u32,
    pub asset: String,
    pub business: String,
    pub change: Decimal,
    pub balance: Decimal,
    pub balance_available: Decimal,
    pub balance_frozen: Decimal,
    pub detail: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WithdrawMessage {
    pub timestamp: f64,
    pub user_id: u32,
    pub asset: String,
    pub business: String,
    pub change: Decimal,
    pub balance: Decimal,
    pub balance_available: Decimal,
    pub balance_frozen: Decimal,
    pub detail: String,
    #[serde(with = "HexArray")]
    pub signature: [u8; 64],
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TransferMessage {
    pub time: f64,
    pub user_from: u32,
    pub user_to: u32,
    pub asset: String,
    pub amount: Decimal,
    #[serde(with = "HexArray")]
    pub signature: [u8; 64],
}

pub trait TxMessage {}

impl TxMessage for DepositMessage {}
impl TxMessage for OrderMessage {}
impl TxMessage for TradeMessage {}
impl TxMessage for TransferMessage {}
impl TxMessage for UserMessage {}
impl TxMessage for WithdrawMessage {}

impl<T: TxMessage> Message<T> {
    pub fn new(message: T, offset: i64) -> Self {
        Self {
            message,
            offset: Some(offset),
        }
    }

    pub fn offset(&self) -> Option<i64> {
        self.offset
    }

    pub fn into_parts(self) -> (T, Option<i64>) {
        (self.message, self.offset)
    }
}

impl<T: TxMessage> From<T> for Message<T> {
    fn from(message: T) -> Self {
        Self { message, offset: None }
    }
}

impl<T: TxMessage> From<(T, i64)> for Message<T> {
    fn from((message, offset): (T, i64)) -> Self {
        Self {
            message,
            offset: Some(offset),
        }
    }
}

impl<T> Deref for Message<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.message
    }
}

impl<T> DerefMut for Message<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.message
    }
}
