use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};
use fnv::FnvHashMap;
use rollup_state_manager::r#const::sled_db::{ACCOUNTSTATES_KEY, ACCOUNTTREE_KEY, BALANCETREES_KEY, ORDERTREES_KEY};
use rollup_state_manager::state::AccountState;
use rollup_state_manager::types::merkle_tree::Tree;
use rollup_state_manager::types::primitives::{FrWrapper, fr_to_string};

fn main() -> Result<()> {
    let sled_path: PathBuf = env::var("SLED_DB_PATH")
        .unwrap_or_else(|_| "/tmp/rollup-sled.db".to_string())
        .parse()?;
    let dump_path: PathBuf = env::var("SLED_DUMP_PATH")
        .unwrap_or_else(|_| "circuits/testdata/dump".to_string())
        .parse()?;
    fs::create_dir_all(&dump_path).context("Failed to create dump directory")?;

    let db = sled::open(&sled_path).context("Failed to open sled")?;

    let account_tree: Tree = db
        .get(ACCOUNTTREE_KEY)?
        .and_then(|v| bincode::deserialize(v.as_ref()).ok())
        .unwrap();
    serde_json::to_writer_pretty(&mut fs::File::create(&dump_path.join("account_tree.json"))?, &account_tree)?;

    let account_states = db.open_tree(ACCOUNTSTATES_KEY).unwrap();
    let loaded_account_states: FnvHashMap<u32, AccountState> = account_tree
        .iter()
        .map(|(id, hash)| {
            println!("{} {}", id, fr_to_string(hash));
            let v = account_states
                .get(bincode::serialize(&FrWrapper::from(hash)).unwrap())
                .ok()
                .flatten()
                .unwrap();
            let (stored_id, state): (u32, AccountState) = bincode::deserialize(v.as_ref()).expect("Failed to deserialize");
            assert_eq!(id, stored_id);
            (stored_id, state)
        })
        .collect();

    {
        let mut account_states_json = fs::File::create(&dump_path.join("account_states.jsonl"))?;
        for (idx, state) in loaded_account_states.iter() {
            serde_json::to_writer(&mut account_states_json, &(idx, state))?;
            account_states_json.write_all(b"\n")?;
        }
    }

    let balance_trees = db.open_tree(BALANCETREES_KEY).unwrap();
    let loaded_balance_trees: FnvHashMap<u32, Tree> = loaded_account_states
        .iter()
        .map(|(id, _)| {
            let tree = balance_trees
                .get(bincode::serialize(&id).unwrap())
                .ok()
                .flatten()
                .and_then(|v| bincode::deserialize(v.as_ref()).ok())
                .unwrap();
            (*id, tree)
        })
        .collect();

    {
        let mut balance_trees_json = fs::File::create(&dump_path.join("balance_trees.jsonl"))?;
        for (idx, tree) in loaded_balance_trees.iter() {
            serde_json::to_writer(&mut balance_trees_json, &(idx, tree))?;
            balance_trees_json.write_all(b"\n")?;
        }
    }

    let order_trees = db.open_tree(ORDERTREES_KEY).unwrap();

    let loaded_order_trees: FnvHashMap<u32, Tree> = loaded_account_states
        .iter()
        .map(|(id, _)| {
            let tree = order_trees
                .get(bincode::serialize(&id).unwrap())
                .ok()
                .flatten()
                .and_then(|v| bincode::deserialize(v.as_ref()).ok())
                .unwrap();
            (*id, tree)
        })
        .collect();

    {
        let mut order_trees_json = fs::File::create(&dump_path.join("order_trees.jsonl"))?;
        for (idx, tree) in loaded_order_trees.iter() {
            serde_json::to_writer(&mut order_trees_json, &(idx, tree))?;
            order_trees_json.write_all(b"\n")?;
        }
    }

    Ok(())
}
