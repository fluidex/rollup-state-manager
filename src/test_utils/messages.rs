use crate::types;
use anyhow::{anyhow, Result};
use serde_json::Value;

pub enum WrappedMessage {
    BALANCE(types::matchengine::messages::BalanceMessage),
    TRADE(types::matchengine::messages::TradeMessage),
    ORDER(types::matchengine::messages::OrderMessage),
}

pub fn parse_msg(line: String) -> Result<WrappedMessage> {
    let v: Value = serde_json::from_str(&line)?;
    if let Value::String(typestr) = &v["type"] {
        let val = v["value"].clone();

        match typestr.as_str() {
            "BalanceMessage" => {
                let data = serde_json::from_value(val).map_err(|e| anyhow!("wrong balance: {}", e))?;
                Ok(WrappedMessage::BALANCE(data))
            }
            "OrderMessage" => {
                let data = serde_json::from_value(val).map_err(|e| anyhow!("wrong balance: {}", e))?;
                Ok(WrappedMessage::ORDER(data))
            }
            "TradeMessage" => {
                let data = serde_json::from_value(val).map_err(|e| anyhow!("wrong balance: {}", e))?;
                Ok(WrappedMessage::TRADE(data))
            }
            other => Err(anyhow!("unrecognized type field {}", other)),
        }
    } else {
        Err(anyhow!("missed or unexpected type field: {}", line))
    }
}
