#![allow(dead_code)]
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::large_enum_variant)]

use crate::types::l2::Order;
use crate::test_utils::messages::{parse_msg, WrappedMessage};
use crate::test_utils::L2BlockSerde;
use crate::types::primitives::{u32_to_fr};
use anyhow::Result;
use rust_decimal::Decimal;
use state_keeper::state::GlobalState;
use state_keeper::test_utils;
use state_keeper::types;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Lines, Write};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::time::Instant;

// TODO: use ENV
pub mod test_params {
    pub const NTXS: usize = 2;
    pub const BALANCELEVELS: usize = 2;
    pub const ORDERLEVELS: usize = 7;
    pub const ACCOUNTLEVELS: usize = 2;
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

// TODO: move most of these to test_utils

type OrdersType = HashMap<u32, (u32, u64)>;
//index type?
#[derive(Debug)]
struct Orders {
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

type AccountsType = HashMap<u32, u32>;
//index type?
#[derive(Debug)]
struct Accounts {
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
    fn userid_to_treeindex(&mut self, state: &mut GlobalState, account_id: u32) -> u32 {
        match self.get(&account_id) {
            Some(idx) => *idx,
            None => {
                let uid = state.create_new_account(1);
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
        state: &mut GlobalState,
        mut trade: types::matchengine::messages::TradeMessage,
    ) -> types::matchengine::messages::TradeMessage {
        trade.ask_user_id = self.userid_to_treeindex(state, trade.ask_user_id);
        trade.bid_user_id = self.userid_to_treeindex(state, trade.bid_user_id);

        trade
    }

    pub fn handle_deposit(&mut self, state: &mut GlobalState, mut deposit: types::matchengine::messages::BalanceMessage) {
        //integrate the sanity check here ...
        deposit.user_id = self.userid_to_treeindex(state, deposit.user_id);

        assert!(!deposit.change.is_sign_negative(), "only support deposit now");

        let token_id = test_params::token_id(&deposit.asset);

        let balance_before = deposit.balance - deposit.change;
        assert!(!balance_before.is_sign_negative(), "invalid balance {:?}", deposit);

        let expected_balance_before = state.get_token_balance(deposit.user_id, token_id);
        assert_eq!(
            expected_balance_before,
            test_utils::number_to_integer(&balance_before, test_params::prec(token_id))
        );

        let timing = Instant::now();

        state.deposit_to_old(types::l2::DepositToOldTx {
            token_id,
            account_id: deposit.user_id,
            amount: test_utils::number_to_integer(&deposit.change, test_params::prec(token_id)),
        });

        self.balance_bench += timing.elapsed().as_secs_f32();
    }

    fn take_bench(&mut self) -> (f32, f32) {
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
    fn place_order_tx(&self) -> types::l2::PlaceOrderTx {
        types::l2::PlaceOrderTx {
            order_id: self.order_id,
            account_id: self.account_id,
            token_id_sell: self.token_sell,
            token_id_buy: self.token_buy,
            amount_sell: test_utils::number_to_integer(&self.total_sell, test_params::prec(self.token_sell)),
            amount_buy: test_utils::number_to_integer(&self.total_buy, test_params::prec(self.token_buy)),
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
            filled_sell: test_utils::number_to_integer(&origin.filled_sell, test_params::prec(origin.token_sell)),
            filled_buy: test_utils::number_to_integer(&origin.filled_buy, test_params::prec(origin.token_buy)),
            total_sell: test_utils::number_to_integer(&origin.total_sell, test_params::prec(origin.token_sell)),
            total_buy: test_utils::number_to_integer(&origin.total_buy, test_params::prec(origin.token_buy)),
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
    bid_user_base: types::primitives::Fr,
    bid_user_quote: types::primitives::Fr,
    ask_user_base: types::primitives::Fr,
    ask_user_quote: types::primitives::Fr,
}

impl CommonBalanceState {
    fn parse(origin: &types::matchengine::messages::VerboseBalanceState, id_pair: TokenIdPair) -> Self {
        let base_id = id_pair.0;
        let quote_id = id_pair.1;

        CommonBalanceState {
            bid_user_base: test_utils::number_to_integer(&origin.bid_user_base, test_params::prec(base_id)),
            bid_user_quote: test_utils::number_to_integer(&origin.bid_user_quote, test_params::prec(quote_id)),
            ask_user_base: test_utils::number_to_integer(&origin.ask_user_base, test_params::prec(base_id)),
            ask_user_quote: test_utils::number_to_integer(&origin.ask_user_quote, test_params::prec(quote_id)),
        }
    }

    fn build_local(state: &GlobalState, bid_id: u32, ask_id: u32, id_pair: TokenIdPair) -> Self {
        let base_id = id_pair.0;
        let quote_id = id_pair.1;

        CommonBalanceState {
            bid_user_base: state.get_token_balance(bid_id, base_id),
            bid_user_quote: state.get_token_balance(bid_id, quote_id),
            ask_user_base: state.get_token_balance(ask_id, base_id),
            ask_user_quote: state.get_token_balance(ask_id, quote_id),
        }
    }
}

fn assert_balance_state(
    balance_state: &types::matchengine::messages::VerboseBalanceState,
    state: &GlobalState,
    bid_id: u32,
    ask_id: u32,
    id_pair: TokenIdPair,
) {
    let local_balance = CommonBalanceState::build_local(state, bid_id, ask_id, id_pair);
    let parsed_state = CommonBalanceState::parse(balance_state, id_pair);
    assert_eq!(local_balance, parsed_state);
}

impl Orders {
    fn take_bench(&mut self) -> (f32, f32) {
        let ret = (self.place_bench, self.spot_bench);
        self.place_bench = 0.0;
        self.spot_bench = 0.0;
        ret
    }

    fn assert_order_state<'c>(&self, state: &GlobalState, ask_order_state: OrderState<'c>, bid_order_state: OrderState<'c>) {
        let ask_order_local = state
            .get_account_order_by_id(ask_order_state.account_id, ask_order_state.order_id);
        assert_eq!(ask_order_local, types::l2::Order::from(ask_order_state));

        let bid_order_local = state
            .get_account_order_by_id(bid_order_state.account_id, bid_order_state.order_id);
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
                amount_1to2: test_utils::number_to_integer(&trade.amount, test_params::prec(id_pair.0)),
                amount_2to1: test_utils::number_to_integer(&trade.quote_amount, test_params::prec(id_pair.1)),
                order1_id: trade.ask_order_id as u32,
                order2_id: trade.bid_order_id as u32,
            },
            types::matchengine::messages::MarketRole::TAKER => types::l2::SpotTradeTx {
                order1_account_id: trade.bid_user_id,
                order2_account_id: trade.ask_user_id,
                token_id_1to2: id_pair.1,
                token_id_2to1: id_pair.0,
                amount_1to2: test_utils::number_to_integer(&trade.quote_amount, test_params::prec(id_pair.1)),
                amount_2to1: test_utils::number_to_integer(&trade.amount, test_params::prec(id_pair.0)),
                order1_id: trade.bid_order_id as u32,
                order2_id: trade.ask_order_id as u32,
            },
        }
    }

    fn check_global_state_knows_order(&self, state: &mut GlobalState, order_state: &OrderState) {
        let is_new_order = order_state.origin.finished_base == Decimal::new(0, 0) && order_state.origin.finished_quote == Decimal::new(0, 0);
        let order_id = order_state.order_id;
        if is_new_order {
            assert!(!state.has_order(order_state.account_id, order_id), "invalid new order");
            let order_to_put = Order {
                order_id: u32_to_fr(order_id),
                tokensell: u32_to_fr(order_state.token_sell),
                tokenbuy: u32_to_fr(order_state.token_buy),
                filled_sell: u32_to_fr(0),
                filled_buy: u32_to_fr(0),
                total_sell: test_utils::number_to_integer(&order_state.total_sell, test_params::prec(order_state.token_sell)),
                total_buy: test_utils::number_to_integer(&order_state.total_buy, test_params::prec(order_state.token_buy)),
            };
            state.update_order_state(order_state.account_id, order_to_put);
        } else {
            assert!(state.has_order(order_state.account_id, order_id), "invalid old order, too many open orders?");
        }
    }

    fn handle_trade(&mut self, state: &mut GlobalState, trade: types::matchengine::messages::TradeMessage) {
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

        self.check_global_state_knows_order(state, &ask_order_state_before);
        self.check_global_state_knows_order(state, &bid_order_state_before);

        assert_balance_state(
            &trade.state_before.balance,
            state,
            bid_order_state_before.account_id,
            ask_order_state_before.account_id,
            id_pair,
        );
        self.assert_order_state(state, ask_order_state_before, bid_order_state_before);

        let timing = Instant::now();
        state.spot_trade(self.trade_into_spot_tx(&trade));
        self.spot_bench += timing.elapsed().as_secs_f32();

        assert_balance_state(
            &trade.state_after.balance,
            state,
            bid_order_state_after.account_id,
            ask_order_state_after.account_id,
            id_pair,
        );
        self.assert_order_state(state, ask_order_state_after, bid_order_state_after);
    }
}

//if we use nightly build, we are able to use bench test ...
fn bench_global_state(circuit_repo: &Path) -> Result<Vec<types::l2::L2Block>> {
    let test_dir = circuit_repo.join("test").join("testdata");
    let file = File::open(test_dir.join("msgs_float.jsonl"))?;

    let messages: Vec<WrappedMessage> = BufReader::new(file)
        .lines()
        .map(Result::unwrap)
        .map(parse_msg)
        .map(Result::unwrap)
        .filter(|msg| matches!(msg, WrappedMessage::BALANCE(_) | WrappedMessage::TRADE(_)))
        .collect();

    println!("prepare bench: {} records", messages.len());

    GlobalState::print_config();
    // TODO: use ENV
    //use custom states
    let mut state = GlobalState::new(
        10, //test_params::BALANCELEVELS,
        10, //test_params::ORDERLEVELS,
        10, //test_params::ACCOUNTLEVELS,
        test_params::NTXS,
        false,
    );

    //amplify the records: in each iter we run records on a group of new accounts
    let mut timing = Instant::now();
    let mut orders = Orders::default();
    let mut accounts = Accounts::default();
    for i in 1..51 {
        for msg in messages.iter() {
            match msg {
                WrappedMessage::BALANCE(balance) => {
                    accounts.handle_deposit(&mut state, balance.clone());
                }
                WrappedMessage::TRADE(trade) => {
                    let trade = accounts.transform_trade(&mut state, trade.clone());
                    orders.handle_trade(&mut state, trade);
                }
                _ => unreachable!(),
            }
        }

        accounts.clear();

        if i % 10 == 0 {
            let total = timing.elapsed().as_secs_f32();
            let (balance_t, _) = accounts.take_bench();
            let (plact_t, spot_t) = orders.take_bench();
            println!(
                "{}th 10 iters in {:.5}s: balance {:.3}%, place {:.3}%, spot {:.3}%",
                i / 10,
                total,
                balance_t * 100.0 / total,
                plact_t * 100.0 / total,
                spot_t * 100.0 / total
            );
            timing = Instant::now();
        }
    }

    Ok(state.take_blocks())
}

fn replay_msgs(circuit_repo: &Path) -> Result<(Vec<types::l2::L2Block>, test_utils::circuit::CircuitSource)> {
    let test_dir = circuit_repo.join("test").join("testdata");
    let file = File::open(test_dir.join("msgs_float.jsonl"))?;

    let lns: Lines<BufReader<File>> = BufReader::new(file).lines();

    let mut state = GlobalState::new(
        test_params::BALANCELEVELS,
        test_params::ORDERLEVELS,
        test_params::ACCOUNTLEVELS,
        test_params::NTXS,
        test_params::VERBOSE,
    );

    println!("genesis root {}", state.root());

    let mut orders = Orders::default();
    let mut accounts = Accounts::default();
    /*
        for _ in 0..test_const::MAXACCOUNTNUM {
            state.create_new_account(1);
        }
    */

    for line in lns {
        match line.map(parse_msg)?? {
            WrappedMessage::BALANCE(balance) => {
                accounts.handle_deposit(&mut state, balance);
            }
            WrappedMessage::TRADE(trade) => {
                let trade = accounts.transform_trade(&mut state, trade);
                let trade_id = trade.id;
                orders.handle_trade(&mut state, trade);
                println!("trade {} test done", trade_id);
            }
            _ => {
                //other msg is omitted
            }
        }
    }

    state.flush_with_nop();

    let component = test_utils::circuit::CircuitSource {
        src: String::from("src/block.circom"),
        main: format!(
            "Block({}, {}, {}, {})",
            test_params::NTXS,
            test_params::BALANCELEVELS,
            test_params::ORDERLEVELS,
            test_params::ACCOUNTLEVELS
        ),
    };

    Ok((state.take_blocks(), component))
}

//just grap from export_circuit_test.rs ...
fn write_circuit(circuit_repo: &Path, test_dir: &Path, source: &test_utils::CircuitSource) -> Result<PathBuf> {
    let circuit_name = test_utils::format_circuit_name(source.main.as_str());
    let circuit_dir = test_dir.join(circuit_name);

    fs::create_dir_all(circuit_dir.clone())?;

    let circuit_file = circuit_dir.join("circuit.circom");

    // on other OS than UNIX the slash in source wolud not be considerred as separator
    //so we need to convert them explicity
    let src_path: PathBuf = source.src.split('/').collect();

    let file_content = format!(
        "include \"{}\";\ncomponent main = {}",
        circuit_repo.join(src_path).to_str().unwrap(),
        source.main
    );
    let mut f = File::create(circuit_file)?;
    f.write_all(&file_content.as_bytes())?;
    Ok(circuit_dir)
}

fn write_input_output(dir: &Path, block: types::l2::L2Block) -> Result<()> {
    fs::create_dir_all(dir)?;

    let input_f = File::create(dir.join("input.json"))?;
    serde_json::to_writer_pretty(input_f, &L2BlockSerde::from(block))?;

    let output_f = File::create(dir.join("output.json"))?;
    //TODO: no output?
    serde_json::to_writer_pretty(output_f, &serde_json::Value::Object(Default::default()))?;

    Ok(())
}

fn export_circuit_and_testdata(circuit_repo: &Path, blocks: Vec<types::l2::L2Block>, source: test_utils::CircuitSource) -> Result<PathBuf> {
    let test_dir = circuit_repo.join("testdata");
    let circuit_dir = write_circuit(circuit_repo, &test_dir, &source)?;

    for (blki, blk) in blocks.into_iter().enumerate() {
        let dir = circuit_dir.join(format!("{:04}", blki));
        write_input_output(&dir, blk)?;
        //println!("{}", serde_json::to_string_pretty(&types::L2BlockSerde::from(blk)).unwrap());
    }

    Ok(circuit_dir)
}

fn run_bench() -> Result<()> {
    let circuit_repo = fs::canonicalize(PathBuf::from("../circuits")).expect("invalid circuits repo path");

    let timing = Instant::now();
    let blocks = bench_global_state(&circuit_repo)?;
    println!(
        "bench for {} blocks (TPS: {})",
        blocks.len(),
        (test_params::NTXS * blocks.len()) as f32 / timing.elapsed().as_secs_f32()
    );

    Ok(())
}

fn run() -> Result<()> {
    let circuit_repo = fs::canonicalize(PathBuf::from("circuits")).expect("invalid circuits repo path");

    let timing = Instant::now();
    let (blocks, components) = replay_msgs(&circuit_repo)?;
    println!(
        "genesis {} blocks (TPS: {})",
        blocks.len(),
        (test_params::NTXS * blocks.len()) as f32 / timing.elapsed().as_secs_f32()
    );

    let circuit_dir = export_circuit_and_testdata(&circuit_repo, blocks, components)?;

    println!("test circuit dir {}", circuit_dir.to_str().unwrap());

    Ok(())
}

/*
 * have a look at scripts/global_state_test.sh
 */

fn main() {
    match run() {
        Ok(_) => println!("global_state tests generated"),
        Err(e) => panic!("{:#?}", e),
    }

    #[cfg(feature = "bench_global_state")]
    run_bench().expect("bench ok");
}
