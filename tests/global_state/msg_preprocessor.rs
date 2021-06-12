use num::Zero;
use rollup_state_manager::account::{Account, Signature};
use rollup_state_manager::state::WitnessGenerator;
use rollup_state_manager::test_utils::types::{get_token_id_by_name, prec_token_id};
use rollup_state_manager::types::l2::{self, OrderInput, OrderSide};
use rollup_state_manager::types::primitives::{u32_to_fr, Fr};
use rollup_state_manager::types::{fixnum, matchengine::messages};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::time::Instant;

use super::types::{assert_balance_state, OrderState, TokenIdPair, TokenPair};

// Preprocessor is used to attach order_sig for each order
// it is only useful in development system
// it should not be used in prod system
#[derive(Default)]
pub struct Preprocessor {
    trade_tx_total_time: f32,
    balance_tx_total_time: f32,
    accounts: HashMap<u32, Account>,
    // (order_hash, bjj_key) -> sig
    order_sig_cache: HashMap<(Fr, String), Signature>,
    // FIXME: we cache the order twice? here and inside witgen
    // (uid, order_id) -> Order
    order_cache: HashMap<(u32, u32), OrderInput>,
    check_order_sig: bool,
}
impl Preprocessor {
    pub fn take_bench(&mut self) -> (f32, f32) {
        let ret = (self.trade_tx_total_time, self.balance_tx_total_time);
        self.trade_tx_total_time = 0.0;
        self.balance_tx_total_time = 0.0;
        ret
    }

    fn assert_order_state<'c>(&self, witgen: &WitnessGenerator, order_state: OrderState<'c>) {
        if witgen.has_order(order_state.account_id, order_state.order_id) {
            let order_local = witgen.get_account_order_by_id(order_state.account_id, order_state.order_id);
            // TODO: compares the order field sig. The field sig is set to the default value of Signature for now.
            //ask_order_local.sig = Signature::default();
            assert_eq!(order_local, l2::Order::from(order_state));
        } else {
            // the only possible path reaching here, is that the order has not been put into witgen
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
    fn parse_order(order_msg: &messages::OrderMessage) -> OrderInput {
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
            tokensell: u32_to_fr(tokensell),
            tokenbuy: u32_to_fr(tokenbuy),
            total_sell: fixnum::decimal_to_amount(&total_sell, prec_token_id(tokensell)).to_fr(),
            total_buy: fixnum::decimal_to_amount(&total_buy, prec_token_id(tokenbuy)).to_fr(),
            sig: Signature::default(),
            account_id: order.user,
            side: if is_ask { OrderSide::Sell } else { OrderSide::Buy },
        }
    }
    fn parse_order_from_state(order_state: &OrderState) -> OrderInput {
        OrderInput {
            order_id: (order_state.order_id),
            tokensell: u32_to_fr(order_state.token_sell),
            tokenbuy: u32_to_fr(order_state.token_buy),
            //filled_sell: u32_to_fr(0),
            //filled_buy: u32_to_fr(0),
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
    fn sign_order_using_cache(&mut self, order_to_put: &mut OrderInput) {
        let order_hash = order_to_put.hash();
        let account = self.accounts.get(&order_to_put.account_id).unwrap();
        //println!("hash {} {} {} {}", account_id, order_state.order_id, order_hash, account.bjj_pub_key());
        let sig = *self.order_sig_cache.entry((order_hash, account.bjj_pub_key())).or_insert_with(|| {
            //println!("sign order");
            account.sign_hash(order_hash).unwrap()
        });
        order_to_put.sig = sig;
    }
    fn sign_order_state_using_cache(&mut self, order_state: &OrderState) -> OrderInput {
        let mut order_to_put = Self::parse_order_from_state(order_state);
        self.sign_order_using_cache(&mut order_to_put);
        order_to_put
    }

    fn check_global_state_knows_order(&self, witgen: &mut WitnessGenerator, account_id: u32, order_id: u32) {
        if !witgen.has_order(account_id, order_id) {
            println!("check_global_state_knows_order {} {}", account_id, order_id);
            let order = self.order_cache.get(&(account_id, order_id)).unwrap();
            let order_state = l2::order::Order::from_order_input(&order);
            witgen.update_order_state(order_state.account_id, order_state);
        }
    }
    /*
    pub fn sign_orders(&mut self, trade: messages::TradeMessage) {
        let token_pair = TokenPair::from(trade.market.as_str());
        let id_pair = TokenIdPair::from(token_pair);
        let ask_order_state_before: OrderState = OrderState::parse(&trade.state_before.ask_order_state, id_pair, token_pair, "ASK", &trade);
        let bid_order_state_before: OrderState = OrderState::parse(&trade.state_before.bid_order_state, id_pair, token_pair, "BID", &trade);
        self.sign_order_state_using_cache(&ask_order_state_before);
        self.sign_order_state_using_cache(&bid_order_state_before);
    }
    */
    pub fn handle_order_msg(&mut self, _witgen: &mut WitnessGenerator, order: messages::OrderMessage) {
        match order.event {
            messages::OrderEventType::FINISH => {
                self.order_cache.remove(&(order.order.user, order.order.id as u32));
            }
            messages::OrderEventType::PUT => {
                let is_new_order = order.order.finished_base == Decimal::zero() && order.order.finished_quote == Decimal::zero();
                debug_assert!(is_new_order);
                let mut order_input = Self::parse_order(&order);
                if self.check_order_sig {
                    // TODO: if order has no sig, deny
                } else {
                    // if order has no sig, auto fill a sig
                    self.sign_order_using_cache(&mut order_input);
                }
                self.order_cache.insert((order_input.account_id, order_input.order_id), order_input);
                println!("store order {} {}", order_input.account_id, order_input.order_id);
            }
            _ => {
                log::debug!("skip order msg {:?}", order.event);
            }
        }
    }
    pub fn handle_trade_msg(&mut self, witgen: &mut WitnessGenerator, trade: messages::TradeMessage) {
        self.check_state(witgen, &trade.state_before, &trade);

        let timing = Instant::now();
        self.check_global_state_knows_order(witgen, trade.ask_user_id, trade.ask_order_id as u32);
        self.check_global_state_knows_order(witgen, trade.bid_user_id, trade.bid_order_id as u32);
        let trade_tx = self.trade_into_spot_tx(&trade);
        witgen.spot_trade(trade_tx);
        self.trade_tx_total_time += timing.elapsed().as_secs_f32();
        self.check_state(witgen, &trade.state_after, &trade);
    }
    fn check_state(&self, witgen: &WitnessGenerator, trade_state: &Option<messages::VerboseTradeState>, trade: &messages::TradeMessage) {
        let token_pair = TokenPair::from(trade.market.as_str());
        let id_pair = TokenIdPair::from(token_pair);
        if let Some(state) = trade_state {
            assert_balance_state(&state.balance, witgen, trade.bid_user_id, trade.ask_user_id, id_pair);

            let ask_order_state: OrderState = OrderState::parse(&state.ask_order_state, id_pair, token_pair, "ASK", &trade);

            let bid_order_state: OrderState = OrderState::parse(&state.bid_order_state, id_pair, token_pair, "BID", &trade);

            self.assert_order_state(witgen, ask_order_state);
            self.assert_order_state(witgen, bid_order_state);
        }
    }
    pub fn set_account(&mut self, account_id: u32, account: Account) {
        //println!("set account {} {}", account_id, account. bjj_pub_key());
        self.accounts.insert(account_id, account);
    }
    pub fn handle_balance_msg(&mut self, witgen: &mut WitnessGenerator, deposit: messages::BalanceMessage) {
        assert!(!deposit.change.is_sign_negative(), "only support deposit now");
        let token_id = get_token_id_by_name(&deposit.asset);
        let account_id = deposit.user_id;
        let is_old = witgen.has_account(account_id);
        let account = self.accounts.entry(account_id).or_insert_with(|| Account::new(account_id));

        let balance_before = deposit.balance - deposit.change;
        assert!(!balance_before.is_sign_negative(), "invalid balance {:?}", deposit);

        let expected_balance_before = witgen.get_token_balance(deposit.user_id, token_id);
        assert_eq!(
            expected_balance_before,
            fixnum::decimal_to_amount(&balance_before, prec_token_id(token_id)).to_fr()
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
        }

        self.balance_tx_total_time += timing.elapsed().as_secs_f32();
    }
}
