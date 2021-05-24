#![allow(dead_code)]
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::large_enum_variant)]

use std::collections::HashMap;

use rollup_state_manager::state::WitnessGenerator;
use rollup_state_manager::types;
use rollup_state_manager::types::fixnum;
use rollup_state_manager::types::l2::Order;
use rollup_state_manager::types::primitives::{fr_to_decimal, u32_to_fr};
use rust_decimal::Decimal;
use std::ops::{Deref, DerefMut};
use std::time::Instant;
// TODO: use ENV

// TODO: move most of these to test_utils

pub mod test_params {
    pub const NTXS: usize = 2;

    pub const BALANCELEVELS: usize = 2;
    pub const ORDERLEVELS: usize = 7;
    pub const ACCOUNTLEVELS: usize = 2;
    /*

          pub const BALANCELEVELS: usize = 20;
          pub const ORDERLEVELS: usize = 20;
          pub const ACCOUNTLEVELS: usize = 20;
    */
    pub const MAXORDERNUM: usize = 2usize.pow(ORDERLEVELS as u32);
    pub const MAXACCOUNTNUM: usize = 2usize.pow(ACCOUNTLEVELS as u32);
    pub const MAXTOKENNUM: usize = 2usize.pow(BALANCELEVELS as u32);
    pub const VERBOSE: bool = false;

    // TODO: enum & impl
    pub fn token_id(token_name: &str) -> u32 {
        match token_name {
            "ETH" => 0,
            "USDT" => 1,
            _ => unreachable!(),
        }
    }

    // TODO: enum & impl
    pub fn prec(token_id: u32) -> u32 {
        match token_id {
            0 | 1 => 6,
            _ => unreachable!(),
        }
    }
}

type OrdersType = HashMap<u32, (u32, u64)>;
//index type?
#[derive(Debug)]
pub struct Orders {
    place_bench: f32,
    spot_bench: f32,
}

impl Default for Orders {
    fn default() -> Self {
        Orders {
            place_bench: 0.0,
            spot_bench: 0.0,
        }
    }
}

impl Orders {
    pub fn take_bench(&mut self) -> (f32, f32) {
        let ret = (self.place_bench, self.spot_bench);
        self.place_bench = 0.0;
        self.spot_bench = 0.0;
        ret
    }

    fn assert_order_state<'c>(&self, witgen: &WitnessGenerator, ask_order_state: OrderState<'c>, bid_order_state: OrderState<'c>) {
        let ask_order_local = witgen.get_account_order_by_id(ask_order_state.account_id, ask_order_state.order_id);
        assert_eq!(ask_order_local, types::l2::Order::from(ask_order_state));

        let bid_order_local = witgen.get_account_order_by_id(bid_order_state.account_id, bid_order_state.order_id);
        assert_eq!(bid_order_local, types::l2::Order::from(bid_order_state));
    }

    fn trade_into_spot_tx(&self, trade: &types::matchengine::messages::TradeMessage) -> types::l2::SpotTradeTx {
        //allow information can be obtained from trade
        let id_pair = TokenIdPair::from(TokenPair::from(trade.market.as_str()));

        match trade.ask_role {
            types::matchengine::messages::MarketRole::MAKER => types::l2::SpotTradeTx {
                order1_account_id: trade.ask_user_id,
                order2_account_id: trade.bid_user_id,
                token_id_1to2: id_pair.0,
                token_id_2to1: id_pair.1,
                amount_1to2: fixnum::decimal_to_amount(&trade.amount, test_params::prec(id_pair.0)),
                amount_2to1: fixnum::decimal_to_amount(&trade.quote_amount, test_params::prec(id_pair.1)),
                order1_id: trade.ask_order_id as u32,
                order2_id: trade.bid_order_id as u32,
            },
            types::matchengine::messages::MarketRole::TAKER => types::l2::SpotTradeTx {
                order1_account_id: trade.bid_user_id,
                order2_account_id: trade.ask_user_id,
                token_id_1to2: id_pair.1,
                token_id_2to1: id_pair.0,
                amount_1to2: fixnum::decimal_to_amount(&trade.quote_amount, test_params::prec(id_pair.1)),
                amount_2to1: fixnum::decimal_to_amount(&trade.amount, test_params::prec(id_pair.0)),
                order1_id: trade.bid_order_id as u32,
                order2_id: trade.ask_order_id as u32,
            },
        }
    }

    fn check_global_state_knows_order(&self, witgen: &mut WitnessGenerator, order_state: &OrderState) {
        let is_new_order =
            order_state.origin.finished_base == Decimal::new(0, 0) && order_state.origin.finished_quote == Decimal::new(0, 0);
        let order_id = order_state.order_id;
        if is_new_order {
            assert!(!witgen.has_order(order_state.account_id, order_id), "invalid new order");
            let order_to_put = Order {
                order_id: u32_to_fr(order_id),
                tokensell: u32_to_fr(order_state.token_sell),
                tokenbuy: u32_to_fr(order_state.token_buy),
                filled_sell: u32_to_fr(0),
                filled_buy: u32_to_fr(0),
                total_sell: fixnum::decimal_to_amount(&order_state.total_sell, test_params::prec(order_state.token_sell)).to_fr(),
                total_buy: fixnum::decimal_to_amount(&order_state.total_buy, test_params::prec(order_state.token_buy)).to_fr(),
            };
            witgen.update_order_state(order_state.account_id, order_to_put);
        } else {
            assert!(
                witgen.has_order(order_state.account_id, order_id),
                "invalid old order, too many open orders?"
            );
        }
    }

    pub fn handle_trade(&mut self, witgen: &mut WitnessGenerator, trade: types::matchengine::messages::TradeMessage) {
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

        self.check_global_state_knows_order(witgen, &ask_order_state_before);
        self.check_global_state_knows_order(witgen, &bid_order_state_before);

        assert_balance_state(
            &trade.state_before.balance,
            witgen,
            bid_order_state_before.account_id,
            ask_order_state_before.account_id,
            id_pair,
        );
        self.assert_order_state(witgen, ask_order_state_before, bid_order_state_before);

        let timing = Instant::now();
        witgen.spot_trade(self.trade_into_spot_tx(&trade));
        self.spot_bench += timing.elapsed().as_secs_f32();

        assert_balance_state(
            &trade.state_after.balance,
            witgen,
            bid_order_state_after.account_id,
            ask_order_state_after.account_id,
            id_pair,
        );
        self.assert_order_state(witgen, ask_order_state_after, bid_order_state_after);
    }
}

type AccountsType = HashMap<u32, u32>;
//index type?
#[derive(Debug)]
pub struct Accounts {
    accountmapping: AccountsType,
    balance_bench: f32,
}

impl Deref for Accounts {
    type Target = AccountsType;
    fn deref(&self) -> &Self::Target {
        &self.accountmapping
    }
}

impl DerefMut for Accounts {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.accountmapping
    }
}

impl Default for Accounts {
    fn default() -> Self {
        Accounts {
            accountmapping: AccountsType::new(),
            balance_bench: 0.0,
        }
    }
}

//make ad-hoc transform in account_id
impl Accounts {
    fn userid_to_treeindex(&mut self, witgen: &mut WitnessGenerator, account_id: u32) -> u32 {
        match self.get(&account_id) {
            Some(idx) => *idx,
            None => {
                let uid = witgen.create_new_account(1);
                self.insert(account_id, uid);
                if test_params::VERBOSE {
                    println!("global account index {} to user account id {}", uid, account_id);
                }
                uid
            }
        }
    }

    pub fn transform_trade(
        &mut self,
        witgen: &mut WitnessGenerator,
        mut trade: types::matchengine::messages::TradeMessage,
    ) -> types::matchengine::messages::TradeMessage {
        trade.ask_user_id = self.userid_to_treeindex(witgen, trade.ask_user_id);
        trade.bid_user_id = self.userid_to_treeindex(witgen, trade.bid_user_id);

        trade
    }

    pub fn handle_deposit(&mut self, witgen: &mut WitnessGenerator, mut deposit: types::matchengine::messages::BalanceMessage) {
        //integrate the sanity check here ...
        deposit.user_id = self.userid_to_treeindex(witgen, deposit.user_id);

        assert!(!deposit.change.is_sign_negative(), "only support deposit now");

        let token_id = test_params::token_id(&deposit.asset);

        let balance_before = deposit.balance - deposit.change;
        assert!(!balance_before.is_sign_negative(), "invalid balance {:?}", deposit);

        let expected_balance_before = witgen.get_token_balance(deposit.user_id, token_id);
        assert_eq!(
            expected_balance_before,
            fixnum::decimal_to_amount(&balance_before, test_params::prec(token_id)).to_fr()
        );

        let timing = Instant::now();

        witgen.deposit_to_old(types::l2::DepositToOldTx {
            token_id,
            account_id: deposit.user_id,
            amount: fixnum::decimal_to_amount(&deposit.change, test_params::prec(token_id)),
        });

        self.balance_bench += timing.elapsed().as_secs_f32();
    }

    pub fn take_bench(&mut self) -> (f32, f32) {
        let ret = (self.balance_bench, 0.0);
        self.balance_bench = 0.0;
        ret
    }
}

#[derive(Clone, Copy)]
struct TokenIdPair(u32, u32);
/*
impl TokenIdPair {
    fn swap(&mut self) {
        let tmp = self.1;
        self.1 = self.0;
        self.0 = tmp;
    }
}
*/
#[derive(Clone, Copy)]
struct TokenPair<'c>(&'c str, &'c str);

struct OrderState<'c> {
    origin: &'c types::matchengine::messages::VerboseOrderState,
    side: &'static str,
    token_sell: u32,
    token_buy: u32,
    total_sell: Decimal,
    total_buy: Decimal,
    filled_sell: Decimal,
    filled_buy: Decimal,

    order_id: u32,
    account_id: u32,
    role: types::matchengine::messages::MarketRole,
}

struct OrderStateTag {
    id: u64,
    account_id: u32,
    role: types::matchengine::messages::MarketRole,
}

impl<'c> From<&'c str> for TokenPair<'c> {
    fn from(origin: &'c str) -> Self {
        let mut assets = origin.split('_');
        let asset_1 = assets.next().unwrap();
        let asset_2 = assets.next().unwrap();
        TokenPair(asset_1, asset_2)
    }
}

impl<'c> From<TokenPair<'c>> for TokenIdPair {
    fn from(origin: TokenPair<'c>) -> Self {
        TokenIdPair(test_params::token_id(origin.0), test_params::token_id(origin.1))
    }
}

impl<'c> OrderState<'c> {
    fn parse(
        origin: &'c types::matchengine::messages::VerboseOrderState,
        id_pair: TokenIdPair,
        _token_pair: TokenPair<'c>,
        side: &'static str,
        trade: &types::matchengine::messages::TradeMessage,
    ) -> Self {
        match side {
            "ASK" => OrderState {
                origin,
                side,
                //status: 0,
                token_sell: id_pair.0,
                token_buy: id_pair.1,
                total_sell: origin.amount,
                total_buy: origin.amount * origin.price,
                filled_sell: origin.finished_base,
                filled_buy: origin.finished_quote,
                order_id: trade.ask_order_id as u32,
                account_id: trade.ask_user_id,
                role: trade.ask_role,
            },
            "BID" => OrderState {
                origin,
                side,
                //status: 0,
                token_sell: id_pair.1,
                token_buy: id_pair.0,
                total_sell: origin.amount * origin.price,
                total_buy: origin.amount,
                filled_sell: origin.finished_quote,
                filled_buy: origin.finished_base,
                order_id: trade.bid_order_id as u32,
                account_id: trade.bid_user_id,
                role: trade.bid_role,
            },
            _ => unreachable!(),
        }
    }
}

impl<'c> From<OrderState<'c>> for types::l2::Order {
    fn from(origin: OrderState<'c>) -> Self {
        types::l2::Order {
            order_id: types::primitives::u32_to_fr(origin.order_id),
            //status: types::primitives::u32_to_fr(origin.status),
            tokenbuy: types::primitives::u32_to_fr(origin.token_buy),
            tokensell: types::primitives::u32_to_fr(origin.token_sell),
            filled_sell: fixnum::decimal_to_amount(&origin.filled_sell, test_params::prec(origin.token_sell)).to_fr(),
            filled_buy: fixnum::decimal_to_amount(&origin.filled_buy, test_params::prec(origin.token_buy)).to_fr(),
            total_sell: fixnum::decimal_to_amount(&origin.total_sell, test_params::prec(origin.token_sell)).to_fr(),
            total_buy: fixnum::decimal_to_amount(&origin.total_buy, test_params::prec(origin.token_buy)).to_fr(),
        }
    }
}

impl<'c> std::cmp::PartialOrd for OrderState<'c> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<'c> std::cmp::PartialEq for OrderState<'c> {
    fn eq(&self, other: &Self) -> bool {
        self.order_id == other.order_id
    }
}

impl<'c> std::cmp::Eq for OrderState<'c> {}

impl<'c> std::cmp::Ord for OrderState<'c> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.order_id.cmp(&other.order_id)
    }
}

#[derive(PartialEq, Debug)]
struct CommonBalanceState {
    /*
    bid_user_base: types::primitives::Fr,
    bid_user_quote: types::primitives::Fr,
    ask_user_base: types::primitives::Fr,
    ask_user_quote: types::primitives::Fr,
    */
    bid_user_base: Decimal,
    bid_user_quote: Decimal,
    ask_user_base: Decimal,
    ask_user_quote: Decimal,
}

impl CommonBalanceState {
    fn parse(origin: &types::matchengine::messages::VerboseBalanceState, _id_pair: TokenIdPair) -> Self {
        //let base_id = id_pair.0;
        //let quote_id = id_pair.1;

        CommonBalanceState {
            bid_user_base: origin.bid_user_base,
            bid_user_quote: origin.bid_user_quote,
            ask_user_base: origin.ask_user_base,
            ask_user_quote: origin.ask_user_quote,
            /*
            bid_user_base: fixnum::decimal_to_amount(&origin.bid_user_base, test_params::prec(base_id)).to_fr(),
            bid_user_quote: fixnum::decimal_to_amount(&origin.bid_user_quote, test_params::prec(quote_id)).to_fr(),
            ask_user_base: fixnum::decimal_to_amount(&origin.ask_user_base, test_params::prec(base_id)).to_fr(),
            ask_user_quote: fixnum::decimal_to_amount(&origin.ask_user_quote, test_params::prec(quote_id)).to_fr(),
            */
        }
    }

    fn build_local(witgen: &WitnessGenerator, bid_id: u32, ask_id: u32, id_pair: TokenIdPair) -> Self {
        let base_id = id_pair.0;
        let quote_id = id_pair.1;

        CommonBalanceState {
            bid_user_base: fr_to_decimal(&witgen.get_token_balance(bid_id, base_id), test_params::prec(base_id)),
            bid_user_quote: fr_to_decimal(&witgen.get_token_balance(bid_id, quote_id), test_params::prec(quote_id)),
            ask_user_base: fr_to_decimal(&witgen.get_token_balance(ask_id, base_id), test_params::prec(base_id)),
            ask_user_quote: fr_to_decimal(&witgen.get_token_balance(ask_id, quote_id), test_params::prec(quote_id)),
            /*
            bid_user_base: witgen.get_token_balance(bid_id, base_id),
            bid_user_quote: witgen.get_token_balance(bid_id, quote_id),
            ask_user_base: witgen.get_token_balance(ask_id, base_id),
            ask_user_quote: witgen.get_token_balance(ask_id, quote_id),
            */
        }
    }
}

fn assert_balance_state(
    balance_state: &types::matchengine::messages::VerboseBalanceState,
    witgen: &WitnessGenerator,
    bid_id: u32,
    ask_id: u32,
    id_pair: TokenIdPair,
) {
    let local_balance = CommonBalanceState::build_local(witgen, bid_id, ask_id, id_pair);
    let parsed_state = CommonBalanceState::parse(balance_state, id_pair);
    assert_eq!(local_balance, parsed_state);
}
