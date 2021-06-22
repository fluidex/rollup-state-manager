use crate::account::{random_mnemonic_with_rng, Account, Signature, SignatureBJJ};
use crate::state::WitnessGenerator;
use crate::test_utils::types::{get_token_id_by_name, prec_token_id};
use crate::types::l2::{self, OrderInput, OrderSide};
use crate::types::primitives::{fr_to_bigint, u32_to_fr, Fr};
use crate::types::{fixnum, matchengine::messages};
use babyjubjub_rs::Point;
use ethers::core::rand::SeedableRng;
use ethers::prelude::coins_bip39::English;
use num::Zero;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::time::Instant;

use super::msg_utils::{
    assert_balance_state, assert_order_state, exchange_order_to_rollup_order, trade_to_order_state, TokenIdPair, TokenPair,
};

// Preprocessor is used to attach order_sig for each order
// it is only useful in development system
// it should not be used in prod system
pub struct Processor {
    trade_tx_total_time: f32,
    balance_tx_total_time: f32,
    accounts: HashMap<u32, Account>,

    // (order_hash, bjj_key) -> sig
    // only useful in debug mode
    order_sig_cache: HashMap<(Fr, String), Signature>,

    // FIXME: we cache the order twice? here and inside witgen
    // (uid, order_id) -> Order
    order_cache: HashMap<(u32, u32), OrderInput>,

    enable_check_order_sig: bool,
    enable_handle_order: bool,
}

impl Default for Processor {
    fn default() -> Self {
        Processor {
            trade_tx_total_time: 0.0,
            balance_tx_total_time: 0.0,
            accounts: Default::default(),
            order_sig_cache: Default::default(),
            order_cache: Default::default(),
            enable_check_order_sig: false,
            enable_handle_order: false,
        }
    }
}

impl Processor {
    pub fn take_bench(&mut self) -> (f32, f32) {
        let ret = (self.trade_tx_total_time, self.balance_tx_total_time);
        self.trade_tx_total_time = 0.0;
        self.balance_tx_total_time = 0.0;
        ret
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
        let account = self.accounts.entry(account_id).or_insert_with(|| {
            // create deterministic keypair for debugging
            let mut r = ethers::core::rand::rngs::StdRng::seed_from_u64(account_id as u64);
            let mnemonic = random_mnemonic_with_rng(&mut r);
            //println!("mnemonic for account {} is {}", account_id, mnemonic.to_phrase().unwrap());
            Account::from_mnemonic::<English>(account_id, &mnemonic).unwrap()
        });

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

    pub fn handle_order_msg(&mut self, _witgen: &mut WitnessGenerator, order: messages::OrderMessage) {
        if !self.enable_handle_order {
            // in this case, we will reconstruct order from trade state
            return;
        }
        match order.event {
            messages::OrderEventType::FINISH => {
                self.order_cache.remove(&(order.order.user, order.order.id as u32));
            }
            messages::OrderEventType::PUT => {
                let is_new_order = order.order.finished_base == Decimal::zero() && order.order.finished_quote == Decimal::zero();
                debug_assert!(is_new_order);
                let mut order_input = Self::parse_order_from_msg(&order);
                self.cache_order(&mut order_input);
            }
            _ => {
                log::debug!("skip order msg {:?}", order.event);
            }
        }
    }
    pub fn handle_trade_msg(&mut self, witgen: &mut WitnessGenerator, trade: messages::TradeMessage) {
        self.check_state(witgen, &trade.state_before, &trade);

        let timing = Instant::now();
        let mut taker_order: Option<l2::Order> = None;
        let mut maker_order: Option<l2::Order> = None;
        if let Some(ask_order) = &trade.ask_order {
            let mut order = exchange_order_to_rollup_order(&ask_order);
            self.check_order_sig(&mut order);
            assert!(!witgen.has_order(order.account_id, order.order_id));
            let ask_order = l2::order::Order::from_order_input(&order);
            match trade.ask_role {
                messages::MarketRole::MAKER => {
                    maker_order = Some(ask_order);
                }
                messages::MarketRole::TAKER => {
                    taker_order = Some(ask_order);
                }
            };
        }
        if let Some(bid_order) = &trade.bid_order {
            let mut bid_order = exchange_order_to_rollup_order(&bid_order);
            self.check_order_sig(&mut bid_order);
            assert!(!witgen.has_order(bid_order.account_id, bid_order.order_id));
            let bid_order = l2::order::Order::from_order_input(&bid_order);
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
        self.check_state(witgen, &trade.state_after, &trade);
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
            total_sell: fixnum::decimal_to_amount(&total_sell, prec_token_id(tokensell)).to_fr(),
            total_buy: fixnum::decimal_to_amount(&total_buy, prec_token_id(tokenbuy)).to_fr(),
            // TODO:
            sig: SignatureBJJ::default(),
            account_id: order.user,
            side: if is_ask { OrderSide::Sell } else { OrderSide::Buy },
        }
    }
    fn check_order_sig(&mut self, order_to_put: &mut OrderInput) {
        if self.enable_check_order_sig {
            // TODO: if order has no sig, deny
        } else if *crate::params::OVERWRITE_SIGNATURE {
            // overwrite order sig
            let order_hash = order_to_put.hash();
            let account = self.accounts.get(&order_to_put.account_id).unwrap();
            //println!("hash {} {} {} {}", account_id, order_state.order_id, order_hash, account.bjj_pub_key());
            let sig = *self.order_sig_cache.entry((order_hash, account.bjj_pub_key())).or_insert_with(|| {
                //println!("sign order");
                account.sign_hash(order_hash).unwrap()
            });
            order_to_put.sig = SignatureBJJ {
                s: fr_to_bigint(&sig.s),
                r_b8: Point { x: sig.r8x, y: sig.r8y },
            };
        } else {
            // use original signature, do nothing
        }
    }

    //fn check_global_state_knows_order(&self, witgen: &mut WitnessGenerator, account_id: u32, order_id: u32) {
    //    if !witgen.has_order(account_id, order_id) {
    //        //println!("check_global_state_knows_order {} {}", account_id, order_id);
    //        let order = self.order_cache.get(&(account_id, order_id)).expect("no order found");
    //        let order = l2::order::Order::from_order_input(&order);
    //        witgen.update_order_state(order.account_id, order);
    //    }
    //}
    fn cache_order(&mut self, order_input: &mut OrderInput) {
        self.check_order_sig(order_input);
        self.order_cache
            .insert((order_input.account_id, order_input.order_id), order_input.clone());
        //println!("store order {} {}", order_input.account_id, order_input.order_id);
    }
    fn check_state(&self, witgen: &WitnessGenerator, trade_state: &Option<messages::VerboseTradeState>, trade: &messages::TradeMessage) {
        let token_pair = TokenPair::from(trade.market.as_str());
        let id_pair = TokenIdPair::from(token_pair);
        if let Some(state) = trade_state {
            assert_balance_state(&state.balance, witgen, trade.bid_user_id, trade.ask_user_id, id_pair);
            let (ask_order_state, bid_order_state) = trade_to_order_state(&state, &trade);
            assert_order_state(witgen, ask_order_state);
            assert_order_state(witgen, bid_order_state);
        }
    }
    pub fn sign_orders(&mut self, trade: messages::TradeMessage) {
        let (ask, bid) = trade_to_order_state(&trade.state_before.clone().unwrap(), &trade);
        self.check_order_sig(&mut OrderInput::from(ask));
        self.check_order_sig(&mut OrderInput::from(bid));
    }
}
