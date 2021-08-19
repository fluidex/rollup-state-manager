use crate::account::SignatureBJJ;
use crate::msg::msg_utils::bytes_to_sig;
use crate::state::ManagerWrapper;
use crate::test_utils::types::{get_token_id_by_name, prec_token_id};
use crate::types::l2::{self, OrderInput, OrderSide};
use crate::types::matchengine::messages;

use fluidex_common::babyjubjub_rs::{self, Point};
use fluidex_common::ff::Field;
use fluidex_common::rust_decimal::Decimal;
use fluidex_common::types::{DecimalExt, Float864, FrExt};
use fluidex_common::Fr;
use num::Zero;
use std::convert::TryInto;
use std::time::Instant;

use super::msg_utils::{check_state, exchange_order_to_rollup_order, TokenIdPair, TokenPair};

pub struct Processor {
    pub enable_check_sig: bool,
    pub balance_tx_total_time: f32,
    pub trade_tx_total_time: f32,
    pub transfer_tx_total_time: f32,
}

impl Default for Processor {
    fn default() -> Self {
        Processor {
            enable_check_sig: true,
            balance_tx_total_time: 0.0,
            trade_tx_total_time: 0.0,
            transfer_tx_total_time: 0.0,
        }
    }
}

impl Processor {
    pub fn handle_user_msg(&mut self, manager: &mut ManagerWrapper, message: messages::Message<messages::UserMessage>) {
        let (user_info, offset) = message.into_parts();
        //println!("handle_user_msg {:#?}", user_info);
        let account_id = user_info.user_id;
        assert!(!manager.has_account(account_id));
        let l2_pubkey: String = user_info.l2_pubkey;
        let l2_pubkey: Vec<u8> = hex::decode(l2_pubkey.trim_start_matches("0x")).unwrap();
        let bjj_compressed: [u8; 32] = l2_pubkey.try_into().unwrap();
        let l2_pubkey_point: Point = babyjubjub_rs::decompress_point(bjj_compressed).unwrap();
        let fake_token_id = 0;
        let fake_amount = Float864::from_decimal(&Decimal::zero(), prec_token_id(fake_token_id)).unwrap();
        let eth_addr = Fr::from_str(&user_info.l1_address);
        let sign = if bjj_compressed[31] & 0x80 != 0x00 { Fr::one() } else { Fr::zero() };
        // TODO: remove '0x' from eth addr?
        manager
            .deposit(
                l2::DepositTx {
                    token_id: fake_token_id,
                    account_id,
                    amount: fake_amount,
                    l2key: Some(l2::L2Key {
                        eth_addr,
                        sign,
                        ay: l2_pubkey_point.y,
                    }),
                },
                offset,
            )
            .unwrap();
    }
    pub fn handle_deposit_msg(&mut self, manager: &mut ManagerWrapper, message: messages::Message<messages::DepositMessage>) {
        let (deposit, offset) = message.into_parts();
        assert!(!deposit.change.is_sign_negative(), "should be a deposit");

        let token_id = get_token_id_by_name(&deposit.asset);
        let account_id = deposit.user_id;

        let balance_before = deposit.balance - deposit.change;
        assert!(!balance_before.is_sign_negative(), "invalid balance {:?}", deposit);

        let expected_balance_before = manager.get_token_balance(deposit.user_id, token_id);
        assert_eq!(expected_balance_before, balance_before.to_fr(prec_token_id(token_id)));

        let timing = Instant::now();
        let amount = deposit.change.to_amount(prec_token_id(token_id));

        manager
            .deposit(
                l2::DepositTx {
                    token_id,
                    account_id,
                    amount,
                    l2key: None,
                },
                offset,
            )
            .unwrap();

        self.balance_tx_total_time += timing.elapsed().as_secs_f32();
    }
    pub fn handle_withdraw_msg(&mut self, _manager: &mut ManagerWrapper, _message: messages::Message<messages::WithdrawMessage>) {
        // TODO: Handles Withdraw messages.
        unimplemented!()
    }
    pub fn handle_order_msg(&mut self, manager: &mut ManagerWrapper, message: messages::Message<messages::OrderMessage>) {
        let (order, _) = message.into_parts();
        match order.event {
            messages::OrderEventType::FINISH => {
                debug_assert_eq!(order.order.finished_base.is_zero(), order.order.finished_quote.is_zero());
                if order.order.finished_base.is_zero() {
                    debug_assert!(
                        !manager.has_order(order.order.user, order.order.id as u32),
                        "manager should not have empty order"
                    );
                } else {
                    debug_assert!(
                        manager.has_order(order.order.user, order.order.id as u32),
                        "manager should have traded order"
                    );
                    manager.cancel_order(order.order.user, order.order.id as u32);
                }
            }
            messages::OrderEventType::PUT => {}
            _ => {
                log::debug!("skip order msg {:?}", order.event);
            }
        }
    }
    pub fn handle_trade_msg(&mut self, manager: &mut ManagerWrapper, message: messages::Message<messages::TradeMessage>) {
        let (trade, offset) = message.into_parts();
        //log::debug!("handle_trade_msg {:#?}", trade);
        if let Some(state_before) = &trade.state_before {
            check_state(manager, state_before, &trade);
        }

        let timing = Instant::now();
        let mut taker_order: Option<l2::Order> = None;
        let mut maker_order: Option<l2::Order> = None;
        if let Some(ask_order_origin) = &trade.ask_order {
            let ask_order_input = exchange_order_to_rollup_order(&ask_order_origin);
            if self.enable_check_sig {
                self.check_order_sig(&manager, &ask_order_input);
            }
            assert!(!manager.has_order(ask_order_input.account_id, ask_order_input.order_id));
            let ask_order = l2::Order::from(ask_order_input);
            match trade.ask_role {
                messages::MarketRole::MAKER => {
                    maker_order = Some(ask_order);
                }
                messages::MarketRole::TAKER => {
                    taker_order = Some(ask_order);
                }
            };
        }
        if let Some(bid_order_origin) = &trade.bid_order {
            let bid_order_input = exchange_order_to_rollup_order(&bid_order_origin);
            if self.enable_check_sig {
                self.check_order_sig(&manager, &bid_order_input);
            }
            assert!(!manager.has_order(bid_order_input.account_id, bid_order_input.order_id));
            let bid_order = l2::Order::from(bid_order_input);
            match trade.bid_role {
                messages::MarketRole::MAKER => {
                    maker_order = Some(bid_order);
                }
                messages::MarketRole::TAKER => {
                    taker_order = Some(bid_order);
                }
            };
        }
        let tx = l2::FullSpotTradeTx {
            trade: self.trade_into_spot_tx(&trade),
            taker_order,
            maker_order,
        };
        manager.full_spot_trade(tx, offset);
        self.trade_tx_total_time += timing.elapsed().as_secs_f32();
        if let Some(state_after) = &trade.state_after {
            check_state(manager, state_after, &trade);
        }
    }
    pub fn handle_transfer_msg(&mut self, manager: &mut ManagerWrapper, message: messages::Message<messages::TransferMessage>) {
        let (transfer, offset) = message.into_parts();
        let amount = transfer.amount;
        assert!(!amount.is_sign_negative(), "Transfer amount must not be negative");

        let token_id = get_token_id_by_name(&transfer.asset);
        let from = transfer.user_from;
        let from_balance = manager.get_token_balance(from, token_id).to_decimal(prec_token_id(token_id));
        assert!(from_balance >= amount, "From user must have sufficient balance");

        let to = transfer.user_to;
        let amount = amount.to_amount(prec_token_id(token_id));

        let timing = Instant::now();
        let raw_sig = bytes_to_sig(transfer.signature);
        let mut transfer_tx = l2::TransferTx::new(from, to, token_id, amount);
        transfer_tx.sig = crate::account::Signature::from_raw(transfer_tx.hash(), &raw_sig);
        if self.enable_check_sig {
            self.check_transfer_sig(&manager, &transfer_tx, &raw_sig);
        }
        manager.transfer(transfer_tx, offset);
        self.transfer_tx_total_time += timing.elapsed().as_secs_f32();
    }
    fn trade_into_spot_tx(&self, trade: &messages::TradeMessage) -> l2::SpotTradeTx {
        //allow information can be obtained from trade
        let id_pair = TokenIdPair::from(TokenPair::from(trade.market.as_str()));

        match trade.ask_role {
            messages::MarketRole::MAKER => l2::SpotTradeTx {
                order1_account_id: trade.ask_user_id,
                order2_account_id: trade.bid_user_id,
                token_id_1to2: id_pair.0,
                token_id_2to1: id_pair.1,
                amount_1to2: trade.amount.to_amount(prec_token_id(id_pair.0)),
                amount_2to1: trade.quote_amount.to_amount(prec_token_id(id_pair.1)),
                order1_id: trade.ask_order_id as u32,
                order2_id: trade.bid_order_id as u32,
            },
            messages::MarketRole::TAKER => l2::SpotTradeTx {
                order1_account_id: trade.bid_user_id,
                order2_account_id: trade.ask_user_id,
                token_id_1to2: id_pair.1,
                token_id_2to1: id_pair.0,
                amount_1to2: trade.quote_amount.to_amount(prec_token_id(id_pair.1)),
                amount_2to1: trade.amount.to_amount(prec_token_id(id_pair.0)),
                order1_id: trade.bid_order_id as u32,
                order2_id: trade.ask_order_id as u32,
            },
        }
    }
    fn parse_order_from_msg(order_msg: &messages::OrderMessage) -> OrderInput {
        let order: &messages::Order = &order_msg.order;
        let base_token_id = get_token_id_by_name(&order_msg.base);
        let quote_token_id = get_token_id_by_name(&order_msg.quote);
        let base_amount = order.amount;
        assert_ne!(order.price, Decimal::zero());
        let quote_amount = order.amount * order.price;
        let is_ask = matches!(order.side, messages::OrderSide::ASK);
        let (tokensell, tokenbuy) = if is_ask {
            (base_token_id, quote_token_id)
        } else {
            (quote_token_id, base_token_id)
        };
        let (total_sell, total_buy) = if is_ask {
            (base_amount, quote_amount)
        } else {
            (quote_amount, base_amount)
        };

        OrderInput {
            order_id: order.id as u32,
            token_sell: Fr::from_u32(tokensell),
            token_buy: Fr::from_u32(tokenbuy),
            total_sell: total_sell.to_fr(prec_token_id(tokensell)),
            total_buy: total_buy.to_fr(prec_token_id(tokenbuy)),
            sig: Some(bytes_to_sig(order.signature)),
            account_id: order.user,
            side: if is_ask { OrderSide::Sell } else { OrderSide::Buy },
        }
    }
    fn check_order_sig(&mut self, manager: &ManagerWrapper, order_to_put: &OrderInput) {
        let msg = order_to_put.hash();
        let sig = order_to_put.sig.clone().unwrap();
        manager
            .check_sig(order_to_put.account_id, &msg, &sig)
            .unwrap_or_else(|_| panic!("invalid sig for order {:?}", order_to_put));
    }

    fn check_transfer_sig(&mut self, manager: &ManagerWrapper, transfer: &l2::TransferTx, sig: &SignatureBJJ) {
        let msg = transfer.hash();
        manager
            .check_sig(transfer.from, &msg, &sig)
            .unwrap_or_else(|_| panic!("invalid sig for transfer {:?}", transfer));
    }

    pub fn take_bench(&mut self) -> (f32, f32) {
        let ret = (self.trade_tx_total_time, self.balance_tx_total_time);
        self.trade_tx_total_time = 0.0;
        self.balance_tx_total_time = 0.0;
        ret
    }
}
