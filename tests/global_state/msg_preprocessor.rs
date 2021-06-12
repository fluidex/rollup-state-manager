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
}
impl Preprocessor {
    pub fn take_bench(&mut self) -> (f32, f32) {
        let ret = (self.trade_tx_total_time, self.balance_tx_total_time);
        self.trade_tx_total_time = 0.0;
        self.balance_tx_total_time = 0.0;
        ret
    }

    fn assert_order_state<'c>(&self, witgen: &WitnessGenerator, ask_order_state: OrderState<'c>, bid_order_state: OrderState<'c>) {
        // TODO: compares the order field sig. The field sig is set to the default value of Signature for now.
        let mut ask_order_local = witgen.get_account_order_by_id(ask_order_state.account_id, ask_order_state.order_id);
        ask_order_local.sig = Signature::default();
        assert_eq!(ask_order_local, l2::Order::from(ask_order_state));

        let mut bid_order_local = witgen.get_account_order_by_id(bid_order_state.account_id, bid_order_state.order_id);
        bid_order_local.sig = Signature::default();
        assert_eq!(bid_order_local, l2::Order::from(bid_order_state));
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
    fn parse_order(order_state: &OrderState) -> OrderInput {
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
    fn sign_order_using_cache(&mut self, order_state: &OrderState) -> OrderInput {
        let account_id = order_state.account_id;
        let mut order_to_put = Self::parse_order(order_state);
        let order_hash = order_to_put.hash();
        let account = self.accounts.get(&account_id).unwrap();
        //println!("hash {} {} {} {}", account_id, order_state.order_id, order_hash, account.bjj_pub_key());
        let sig = *self.order_sig_cache.entry((order_hash, account.bjj_pub_key())).or_insert_with(|| {
            //println!("sign order");
            account.sign_hash(order_hash).unwrap()
        });
        order_to_put.sig = sig;
        order_to_put
    }

    fn check_global_state_knows_order(&mut self, witgen: &mut WitnessGenerator, order_state: &OrderState) {
        let is_new_order =
            order_state.origin.finished_base == Decimal::new(0, 0) && order_state.origin.finished_quote == Decimal::new(0, 0);
        //let account_id = order_state.account_id;
        let order_id = order_state.order_id;
        if is_new_order {
            assert!(!witgen.has_order(order_state.account_id, order_id), "invalid new order");
            let order_to_put = self.sign_order_using_cache(order_state);
            let order_state = l2::order::Order::from_order_input(&order_to_put);
            witgen.update_order_state(order_state.account_id, order_state);
        } else {
            assert!(
                witgen.has_order(order_state.account_id, order_id),
                "invalid old order, too many open orders?"
            );
        }
    }

    pub fn sign_orders(&mut self, trade: messages::TradeMessage) {
        let token_pair = TokenPair::from(trade.market.as_str());
        let id_pair = TokenIdPair::from(token_pair);
        let ask_order_state_before: OrderState = OrderState::parse(&trade.state_before.ask_order_state, id_pair, token_pair, "ASK", &trade);
        let bid_order_state_before: OrderState = OrderState::parse(&trade.state_before.bid_order_state, id_pair, token_pair, "BID", &trade);
        self.sign_order_using_cache(&ask_order_state_before);
        self.sign_order_using_cache(&bid_order_state_before);
    }

    pub fn handle_trade(&mut self, witgen: &mut WitnessGenerator, trade: messages::TradeMessage) {
        let token_pair = TokenPair::from(trade.market.as_str());
        let id_pair = TokenIdPair::from(token_pair);

        let ask_order_state_before: OrderState = OrderState::parse(&trade.state_before.ask_order_state, id_pair, token_pair, "ASK", &trade);

        let bid_order_state_before: OrderState = OrderState::parse(&trade.state_before.bid_order_state, id_pair, token_pair, "BID", &trade);

        //this field is not used yet ...
        let ask_order_state_after: OrderState = OrderState::parse(&trade.state_after.ask_order_state, id_pair, token_pair, "ASK", &trade);

        let bid_order_state_after: OrderState = OrderState::parse(&trade.state_after.bid_order_state, id_pair, token_pair, "BID", &trade);

        //seems we do not need to use map/zip liket the ts code because the suitable order_id has been embedded
        //into the tag.id field
        let mut put_states = vec![&ask_order_state_before, &bid_order_state_before];
        put_states.sort();

        let test_use_full_spot_trade: bool = true;

        if test_use_full_spot_trade {
            // pass
        } else {
            self.check_global_state_knows_order(witgen, &ask_order_state_before);
            self.check_global_state_knows_order(witgen, &bid_order_state_before);
        }

        assert_balance_state(
            &trade.state_before.balance,
            witgen,
            bid_order_state_before.account_id,
            ask_order_state_before.account_id,
            id_pair,
        );

        let timing = Instant::now();
        let trade_tx = self.trade_into_spot_tx(&trade);
        if test_use_full_spot_trade {
            let ask_order = self.sign_order_using_cache(&ask_order_state_before);
            let bid_order = self.sign_order_using_cache(&bid_order_state_before);

            let full_trade_tx = match trade.ask_role {
                messages::MarketRole::MAKER => l2::FullSpotTradeTx {
                    trade: trade_tx,
                    maker_order: l2::order::Order::from_order_input(&ask_order),
                    taker_order: l2::order::Order::from_order_input(&bid_order),
                },
                messages::MarketRole::TAKER => l2::FullSpotTradeTx {
                    trade: trade_tx,
                    maker_order: l2::order::Order::from_order_input(&bid_order),
                    taker_order: l2::order::Order::from_order_input(&ask_order),
                },
            };
            //self.assert_order_state(witgen, ask_order_state_before, bid_order_state_before);
            witgen.full_spot_trade(full_trade_tx);
        } else {
            self.assert_order_state(witgen, ask_order_state_before, bid_order_state_before);
            witgen.spot_trade(trade_tx);
        }
        self.trade_tx_total_time += timing.elapsed().as_secs_f32();

        assert_balance_state(
            &trade.state_after.balance,
            witgen,
            bid_order_state_after.account_id,
            ask_order_state_after.account_id,
            id_pair,
        );
        self.assert_order_state(witgen, ask_order_state_after, bid_order_state_after);
    }
    pub fn set_account(&mut self, account_id: u32, account: Account) {
        //println!("set account {} {}", account_id, account. bjj_pub_key());
        self.accounts.insert(account_id, account);
    }
    pub fn handle_deposit(&mut self, witgen: &mut WitnessGenerator, deposit: messages::BalanceMessage) {
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
