use crate::types::matchengine::messages::{
    BalanceMessage, DepositMessage, Message, OrderMessage, TradeMessage, UserMessage, WithdrawMessage,
};
use anyhow::{anyhow, Result};
use serde_json::Value;

#[derive(Debug)]
pub enum WrappedMessage {
    BALANCE(Message<BalanceMessage>),
    DEPOSIT(Message<DepositMessage>),
    TRADE(Message<TradeMessage>),
    ORDER(Message<OrderMessage>),
    USER(Message<UserMessage>),
    WITHDRAW(Message<WithdrawMessage>),
}

pub fn parse_msg(line: String) -> Result<WrappedMessage> {
    let v: Value = serde_json::from_str(&line)?;
    if let Value::String(typestr) = &v["type"] {
        let val = v["value"].clone();

        match typestr.as_str() {
            "BalanceMessage" => {
                let data: BalanceMessage = serde_json::from_value(val).map_err(|e| anyhow!("wrong balance: {}", e))?;
                Ok(WrappedMessage::BALANCE(data.into()))
            }
            "DepositMessage" => {
                let data: DepositMessage = serde_json::from_value(val).map_err(|e| anyhow!("wrong balance: {}", e))?;
                Ok(WrappedMessage::DEPOSIT(data.into()))
            }
            "OrderMessage" => {
                let data: OrderMessage = serde_json::from_value(val).map_err(|e| anyhow!("wrong order: {}", e))?;
                Ok(WrappedMessage::ORDER(data.into()))
            }
            "TradeMessage" => {
                let data: TradeMessage = serde_json::from_value(val).map_err(|e| anyhow!("wrong trade: {}", e))?;
                Ok(WrappedMessage::TRADE(data.into()))
            }
            "UserMessage" => {
                let data: UserMessage = serde_json::from_value(val).map_err(|e| anyhow!("wrong user: {}", e))?;
                Ok(WrappedMessage::USER(data.into()))
            }
            "WithdrawMessage" => {
                let data: WithdrawMessage = serde_json::from_value(val).map_err(|e| anyhow!("wrong balance: {}", e))?;
                Ok(WrappedMessage::WITHDRAW(data.into()))
            }
            other => Err(anyhow!("unrecognized type field {}", other)),
        }
    } else {
        Err(anyhow!("missed or unexpected type field: {}", line))
    }
}
