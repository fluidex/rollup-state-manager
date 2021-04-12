#![allow(dead_code)]
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::large_enum_variant)]

// use std::cmp;
// use serde_json::json;
use crate::test_utils::L2BlockSerde;
use anyhow::{anyhow, Result};
use rust_decimal::Decimal;
use serde_json::Value;
use state_keeper::state::{common, global_state};
use state_keeper::test_utils;
use state_keeper::types;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Lines, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

pub mod test_params {
    pub const NTXS: usize = 2;
    pub const BALANCELEVELS: usize = 2;
    pub const ORDERLEVELS: usize = 7;
    pub const ACCOUNTLEVELS: usize = 2;
    pub const MAXORDERNUM: usize = 2usize.pow(ORDERLEVELS as u32);
    pub const MAXACCOUNTNUM: usize = 2usize.pow(ACCOUNTLEVELS as u32);
    pub const MAXTOKENNUM: usize = 2usize.pow(BALANCELEVELS as u32);
    pub const VERBOSE: bool = false;

    // TODO: put in util
    // TODO: enum & impl
    pub fn token_id(token_name: &str) -> u32 {
        match token_name {
            "ETH" => 0,
            "USDT" => 1,
            _ => unreachable!(),
        }
    }

    // TODO: put in util
    // TODO: enum & impl
    pub fn prec(token_id: u32) -> u32 {
        match token_id {
            0 | 1 => 6,
            _ => unreachable!(),
        }
    }
}

enum WrappedMessage {
    BALANCE(types::messages::BalanceMessage),
    TRADE(types::messages::TradeMessage),
    ORDER(types::messages::OrderMessage),
}

fn parse_msg(line: String) -> Result<WrappedMessage> {
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

type PlaceOrderType = HashMap<u32, (u32, u64)>;
//index type?
#[derive(Debug)]
struct PlaceOrder(PlaceOrderType);

impl AsRef<PlaceOrderType> for PlaceOrder {
    fn as_ref(&self) -> &PlaceOrderType {
        &self.0
    }
}

impl AsMut<PlaceOrderType> for PlaceOrder {
    fn as_mut(&mut self) -> &mut PlaceOrderType {
        &mut self.0
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
    origin: &'c types::messages::VerboseOrderState,
    side: &'static str,
    token_sell: u32,
    token_buy: u32,
    total_sell: Decimal,
    total_buy: Decimal,
    filled_sell: Decimal,
    filled_buy: Decimal,

    order_id: u32,
    account_id: u32,
    role: types::messages::MarketRole,
}

struct OrderStateTag {
    id: u64,
    account_id: u32,
    role: types::messages::MarketRole,
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
        origin: &'c types::messages::VerboseOrderState,
        id_pair: TokenIdPair,
        _token_pair: TokenPair<'c>,
        side: &'static str,
        trade: &types::messages::TradeMessage,
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
    fn place_order_tx(&self) -> common::PlaceOrderTx {
        common::PlaceOrderTx {
            order_id: self.order_id,
            account_id: self.account_id,
            token_id_sell: self.token_sell,
            token_id_buy: self.token_buy,
            amount_sell: test_utils::number_to_integer(&self.total_sell, test_params::prec(self.token_sell)),
            amount_buy: test_utils::number_to_integer(&self.total_buy, test_params::prec(self.token_buy)),
        }
    }
}

impl<'c> From<OrderState<'c>> for common::Order {
    fn from(origin: OrderState<'c>) -> Self {
        common::Order {
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
    fn parse(origin: &types::messages::VerboseBalanceState, id_pair: TokenIdPair) -> Self {
        let base_id = id_pair.0;
        let quote_id = id_pair.1;

        CommonBalanceState {
            bid_user_base: test_utils::number_to_integer(&origin.bid_user_base, test_params::prec(base_id)),
            bid_user_quote: test_utils::number_to_integer(&origin.bid_user_quote, test_params::prec(quote_id)),
            ask_user_base: test_utils::number_to_integer(&origin.ask_user_base, test_params::prec(base_id)),
            ask_user_quote: test_utils::number_to_integer(&origin.ask_user_quote, test_params::prec(quote_id)),
        }
    }

    fn build_local(state: &global_state::GlobalState, bid_id: u32, ask_id: u32, id_pair: TokenIdPair) -> Self {
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
    balance_state: &types::messages::VerboseBalanceState,
    state: &global_state::GlobalState,
    bid_id: u32,
    ask_id: u32,
    id_pair: TokenIdPair,
) {
    let local_balance = CommonBalanceState::build_local(state, bid_id, ask_id, id_pair);
    let parsed_state = CommonBalanceState::parse(balance_state, id_pair);
    assert_eq!(local_balance, parsed_state);
}

impl PlaceOrder {
    fn assert_order_state<'c>(&self, state: &global_state::GlobalState, ask_order_state: OrderState<'c>, bid_order_state: OrderState<'c>) {
        let ask_order_local = state
            .get_account_order_by_id(ask_order_state.account_id, ask_order_state.order_id)
            .unwrap();
        assert_eq!(ask_order_local, common::Order::from(ask_order_state));

        let bid_order_local = state
            .get_account_order_by_id(bid_order_state.account_id, bid_order_state.order_id)
            .unwrap();
        assert_eq!(bid_order_local, common::Order::from(bid_order_state));
    }

    fn trade_into_spot_tx(&self, trade: &types::messages::TradeMessage) -> common::SpotTradeTx {
        //allow information can be obtained from trade
        let id_pair = TokenIdPair::from(TokenPair::from(trade.market.as_str()));

        match trade.ask_role {
            types::messages::MarketRole::MAKER => common::SpotTradeTx {
                order1_account_id: trade.ask_user_id,
                order2_account_id: trade.bid_user_id,
                token_id_1to2: id_pair.0,
                token_id_2to1: id_pair.1,
                amount_1to2: test_utils::number_to_integer(&trade.amount, test_params::prec(id_pair.0)),
                amount_2to1: test_utils::number_to_integer(&trade.quote_amount, test_params::prec(id_pair.1)),
                order1_id: trade.ask_order_id as u32,
                order2_id: trade.bid_order_id as u32,
            },
            types::messages::MarketRole::TAKER => common::SpotTradeTx {
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

    fn handle_trade(&mut self, state: &mut global_state::GlobalState, trade: types::messages::TradeMessage) {
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

        for order_state in put_states.into_iter() {
            if !self.as_ref().contains_key(&order_state.order_id) {
                //why the returning order id is u32?
                // in fact the GlobalState should not expose "inner idx/pos" to caller
                // we'd better handle this inside GlobalState later
                let new_order_pos = state.place_order(order_state.place_order_tx());
                self.as_mut()
                    .insert(order_state.order_id, (order_state.account_id, new_order_pos as u64));
                if test_params::VERBOSE {
                    println!(
                        "global order id {} to user order id ({},{})",
                        order_state.order_id, order_state.account_id, new_order_pos
                    );
                }
            } else if test_params::VERBOSE {
                println!("skip put order {}", order_state.order_id);
            }
        }

        assert_balance_state(
            &trade.state_before.balance,
            state,
            bid_order_state_before.account_id,
            ask_order_state_before.account_id,
            id_pair,
        );
        self.assert_order_state(state, ask_order_state_before, bid_order_state_before);

        state.spot_trade(self.trade_into_spot_tx(&trade));

        assert_balance_state(
            &trade.state_after.balance,
            state,
            bid_order_state_after.account_id,
            ask_order_state_after.account_id,
            id_pair,
        );
        self.assert_order_state(state, ask_order_state_after, bid_order_state_after);

        println!("trade {} test done", trade.id);
    }
}

fn handle_deposit(state: &mut global_state::GlobalState, deposit: types::messages::BalanceMessage) {
    //integrate the sanity check here ...
    assert!(!deposit.change.is_sign_negative(), "only support deposit now");

    let token_id = test_params::token_id(&deposit.asset);

    let balance_before = deposit.balance - deposit.change;
    assert!(!balance_before.is_sign_negative(), "invalid balance {:?}", deposit);

    let expected_balance_before = state.get_token_balance(deposit.user_id, token_id);
    assert_eq!(
        expected_balance_before,
        test_utils::number_to_integer(&balance_before, test_params::prec(token_id))
    );

    state.deposit_to_old(common::DepositToOldTx {
        token_id,
        account_id: deposit.user_id,
        amount: test_utils::number_to_integer(&deposit.change, test_params::prec(token_id)),
    });
}

fn replay_msgs(circuit_repo: &Path) -> Result<(Vec<common::L2Block>, test_utils::circuit::CircuitSource)> {
    let test_dir = circuit_repo.join("test").join("testdata");
    let file = File::open(test_dir.join("msgs_float.jsonl"))?;

    let lns: Lines<BufReader<File>> = BufReader::new(file).lines();

    let mut state = global_state::GlobalState::new(
        test_params::BALANCELEVELS,
        test_params::ORDERLEVELS,
        test_params::ACCOUNTLEVELS,
        test_params::NTXS,
        test_params::VERBOSE,
    );

    println!("genesis root {}", state.root());

    let mut place_order = PlaceOrder(PlaceOrderType::new());

    for _ in 0..test_params::MAXACCOUNTNUM {
        state.create_new_account(1);
    }

    for line in lns {
        match line.map(parse_msg)?? {
            WrappedMessage::BALANCE(balance) => {
                handle_deposit(&mut state, balance);
            }
            WrappedMessage::TRADE(trade) => {
                place_order.handle_trade(&mut state, trade);
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

fn write_input_output(dir: &Path, block: common::L2Block) -> Result<()> {
    fs::create_dir_all(dir)?;

    let input_f = File::create(dir.join("input.json"))?;
    serde_json::to_writer_pretty(input_f, &L2BlockSerde::from(block))?;

    let output_f = File::create(dir.join("output.json"))?;
    //TODO: no output?
    serde_json::to_writer_pretty(output_f, &serde_json::Value::Object(Default::default()))?;

    Ok(())
}

fn export_circuit_and_testdata(circuit_repo: &Path, blocks: Vec<common::L2Block>, source: test_utils::CircuitSource) -> Result<PathBuf> {
    let test_dir = circuit_repo.join("testdata");
    let circuit_dir = write_circuit(circuit_repo, &test_dir, &source)?;

    for (blki, blk) in blocks.into_iter().enumerate() {
        let input_dir = circuit_dir.join(format!("{:04}", blki));
        write_input(&input_dir, blk)?;
        //println!("{}", serde_json::to_string_pretty(&types::L2BlockSerde::from(blk)).unwrap());
    }

    Ok(circuit_dir)
}

fn run() -> Result<()> {
    let circuit_repo = fs::canonicalize(PathBuf::from("../circuits")).expect("invalid circuits repo path");

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
 * cargo run --bin export_circuit_test
 * npm -g install snarkit
 * npx snarkit test ../circuits/testdata/Block_2_2_7_2/
 */

fn main() {
    match run() {
        Ok(_) => println!("global_state tests generated"),
        Err(e) => panic!("{:#?}", e),
    }
}
