use crate::msg::msg_utils::bytes_to_sig;
use crate::state::WitnessGenerator;
use crate::test_utils::types::{get_token_id_by_name, prec_token_id};
use crate::types::fixnum::Float864;
use crate::types::l2::{self, OrderInput, OrderSide};
use crate::types::primitives::{str_to_fr, u32_to_fr, Fr};
use crate::types::{fixnum, matchengine::messages};

use babyjubjub_rs::Point;
use ff::Field;
use num::Zero;
use rust_decimal::Decimal;
use std::convert::TryInto;
use std::time::Instant;

use super::msg_utils::{check_state, exchange_order_to_rollup_order, TokenIdPair, TokenPair};

pub struct Processor {
    trade_tx_total_time: f32,
    balance_tx_total_time: f32,
    enable_check_order_sig: bool,
    enable_handle_order: bool,
}

impl Default for Processor {
    fn default() -> Self {
        Processor {
            trade_tx_total_time: 0.0,
            balance_tx_total_time: 0.0,
            enable_check_order_sig: true,
            enable_handle_order: false,
        }
    }
}

impl Processor {
    pub fn handle_user_msg(&mut self, witgen: &mut WitnessGenerator, user_info: messages::UserMessage) {
        let account_id = user_info.user_id;
        assert!(!witgen.has_account(account_id));
        let l2_pubkey: String = user_info.l2_pubkey;
        let l2_pubkey: Vec<u8> = hex::decode(l2_pubkey.trim_start_matches("0x")).unwrap();
        let bjj_compressed: [u8; 32] = l2_pubkey.try_into().unwrap();
        let l2_pubkey_point: Point = babyjubjub_rs::decompress_point(bjj_compressed).unwrap();
        let fake_token_id = 0;
        let fake_amount = Float864::from_decimal(&Decimal::zero(), prec_token_id(fake_token_id)).unwrap();
        let eth_addr = str_to_fr(&user_info.l1_address);
        let sign = if bjj_compressed[31] & 0x80 != 0x00 { Fr::one() } else { Fr::zero() };
        // TODO: remove '0x' from eth addr?
        witgen
            .deposit(l2::DepositTx {
                token_id: fake_token_id,
                account_id,
                amount: fake_amount,
                l2key: Some(l2::L2Key {
                    eth_addr,
                    sign,
                    ay: l2_pubkey_point.y,
                }),
            })
            .unwrap();
    }
    pub fn handle_balance_msg(&mut self, witgen: &mut WitnessGenerator, deposit: messages::BalanceMessage) {
        //log::debug!("handle_balance_msg {:#?}", deposit);
        assert!(!deposit.change.is_sign_negative(), "only support deposit now");
        let token_id = get_token_id_by_name(&deposit.asset);
        let account_id = deposit.user_id;
        let is_old = witgen.has_account(account_id);

        // we now use UserMessage to create new user
        assert!(is_old);

        let balance_before = deposit.balance - deposit.change;
        assert!(!balance_before.is_sign_negative(), "invalid balance {:?}", deposit);

        let expected_balance_before = witgen.get_token_balance(deposit.user_id, token_id);
        assert_eq!(
            expected_balance_before,
            fixnum::decimal_to_fr(&balance_before, prec_token_id(token_id))
        );

        let timing = Instant::now();
        let amount = fixnum::decimal_to_amount(&deposit.change, prec_token_id(token_id));

        if is_old {
            witgen
                .deposit(l2::DepositTx {
                    token_id,
                    account_id,
                    amount,
                    l2key: None,
                })
                .unwrap();
        } else {
            /*
            let account = self.accounts.entry(account_id).or_insert_with(|| {
                // create deterministic keypair for debugging
                //println!("create debug account {}", account_id);
                let mnemonic = get_mnemonic_by_account_id(account_id);
                Account::from_mnemonic::<English>(account_id, &mnemonic).unwrap()
            });

            witgen
                .deposit(l2::DepositTx {
                    token_id,
                    account_id,
                    amount,
                    l2key: Some(l2::L2Key {
                        eth_addr: account.eth_addr(),
                        sign: account.sign(),
                        ay: account.ay(),
                    }),
                })
                .unwrap();
                */
        }

        self.balance_tx_total_time += timing.elapsed().as_secs_f32();
    }

    pub fn handle_order_msg(&mut self, witgen: &mut WitnessGenerator, order: messages::OrderMessage) {
        match order.event {
            messages::OrderEventType::FINISH => {
                debug_assert_eq!(order.order.finished_base.is_zero(), order.order.finished_quote.is_zero());
                if order.order.finished_base.is_zero() {
                    debug_assert!(
                        !witgen.has_order(order.order.user, order.order.id as u32),
                        "witgen should not have empty order"
                    );
                    return;
                } else {
                    debug_assert!(
                        witgen.has_order(order.order.user, order.order.id as u32),
                        "witgen should have traded order"
                    );
                    witgen.cancel_order(order.order.user, order.order.id as u32);
                    return;
                }
            }
            messages::OrderEventType::PUT => {}
            _ => {
                log::debug!("skip order msg {:?}", order.event);
            }
        }
    }
    pub fn handle_trade_msg(&mut self, witgen: &mut WitnessGenerator, trade: messages::TradeMessage) {
        //log::debug!("handle_trade_msg {:#?}", trade);
        if let Some(state_before) = &trade.state_before {
            check_state(witgen, state_before, &trade);
        }

        let timing = Instant::now();
        let mut taker_order: Option<l2::Order> = None;
        let mut maker_order: Option<l2::Order> = None;
        if let Some(ask_order_origin) = &trade.ask_order {
            let mut ask_order_input = exchange_order_to_rollup_order(&ask_order_origin);
            if self.enable_check_order_sig {
                self.check_order_sig(&witgen, &mut ask_order_input);
            }
            assert!(!witgen.has_order(ask_order_input.account_id, ask_order_input.order_id));
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
            let mut bid_order_input = exchange_order_to_rollup_order(&bid_order_origin);
            if self.enable_check_order_sig {
                self.check_order_sig(&witgen, &mut bid_order_input);
            }
            assert!(!witgen.has_order(bid_order_input.account_id, bid_order_input.order_id));
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
        witgen.full_spot_trade(tx);
        self.trade_tx_total_time += timing.elapsed().as_secs_f32();
        if let Some(state_after) = &trade.state_after {
            check_state(witgen, state_after, &trade);
        }
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
                amount_1to2: fixnum::decimal_to_amount(&trade.amount, prec_token_id(id_pair.0)),
                amount_2to1: fixnum::decimal_to_amount(&trade.quote_amount, prec_token_id(id_pair.1)),
                order1_id: trade.ask_order_id as u32,
                order2_id: trade.bid_order_id as u32,
            },
            messages::MarketRole::TAKER => l2::SpotTradeTx {
                order1_account_id: trade.bid_user_id,
                order2_account_id: trade.ask_user_id,
                token_id_1to2: id_pair.1,
                token_id_2to1: id_pair.0,
                amount_1to2: fixnum::decimal_to_amount(&trade.quote_amount, prec_token_id(id_pair.1)),
                amount_2to1: fixnum::decimal_to_amount(&trade.amount, prec_token_id(id_pair.0)),
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
            token_sell: u32_to_fr(tokensell),
            token_buy: u32_to_fr(tokenbuy),
            total_sell: fixnum::decimal_to_fr(&total_sell, prec_token_id(tokensell)),
            total_buy: fixnum::decimal_to_fr(&total_buy, prec_token_id(tokenbuy)),
            sig: Some(bytes_to_sig(order.signature)),
            account_id: order.user,
            side: if is_ask { OrderSide::Sell } else { OrderSide::Buy },
        }
    }
    fn check_order_sig(&mut self, witgen: &WitnessGenerator, order_to_put: &mut OrderInput) {
        let msg = order_to_put.hash();
        let sig = order_to_put.sig.clone().unwrap();
        witgen
            .check_sig(order_to_put.account_id, &msg, &sig)
            .unwrap_or_else(|_| panic!("invalid sig for order {:?}", order_to_put));
    }

    pub fn take_bench(&mut self) -> (f32, f32) {
        let ret = (self.trade_tx_total_time, self.balance_tx_total_time);
        self.trade_tx_total_time = 0.0;
        self.balance_tx_total_time = 0.0;
        ret
    }
}
