use crate::types::matchengine::messages::{
    DepositMessage, Message, OrderMessage, TradeMessage, TransferMessage, UserMessage, WithdrawMessage,
};
use anyhow::{anyhow, Result};
use serde_json::Value;

#[derive(Debug)]
pub enum WrappedMessage {
    DEPOSIT(Message<DepositMessage>),
    ORDER(Message<OrderMessage>),
    TRADE(Message<TradeMessage>),
    TRANSFER(Message<TransferMessage>),
    USER(Message<UserMessage>),
    WITHDRAW(Message<WithdrawMessage>),
}

pub fn parse_msg(line: String) -> Result<WrappedMessage> {
    let v: Value = serde_json::from_str(&line)?;
    if let Value::String(typestr) = &v["type"] {
        let val = v["value"].clone();

        match typestr.as_str() {
            "DepositMessage" => {
                let data: DepositMessage = serde_json::from_value(val).map_err(|e| anyhow!("wrong deposit: {}", e))?;
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
            "TransferMessage" => {
                let data: TransferMessage = serde_json::from_value(val).map_err(|e| anyhow!("wrong transfer: {}", e))?;
                Ok(WrappedMessage::TRANSFER(data.into()))
            }
            "UserMessage" => {
                let data: UserMessage = serde_json::from_value(val).map_err(|e| anyhow!("wrong user: {}", e))?;
                Ok(WrappedMessage::USER(data.into()))
            }
            "WithdrawMessage" => {
                let data: WithdrawMessage = serde_json::from_value(val).map_err(|e| anyhow!("wrong withdraw: {}", e))?;
                Ok(WrappedMessage::WITHDRAW(data.into()))
            }
            other => Err(anyhow!("unrecognized type field {}", other)),
        }
    } else {
        Err(anyhow!("missed or unexpected type field: {}", line))
    }
}
