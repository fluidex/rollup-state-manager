use fluidex_common::non_blocking_tracing;
use normpath::PathExt;
use rollup_state_manager::test_utils::circuit;
use std::path::PathBuf;

mod test_l2_block;
mod test_merkletree;

use rollup_state_manager::config::Settings;
use test_l2_block::get_l2_block_test_case;
use test_merkletree::get_merkle_tree_test_case;

fn run() -> anyhow::Result<()> {
    let circuit_repo = PathBuf::from("circuits")
        .as_path()
        .normalize()
        .expect("invalid circuits repo path")
        .into_path_buf();
    let test_dir = circuit_repo.join("testdata");
    circuit::write_test_case(&circuit_repo, &test_dir, get_merkle_tree_test_case())?;
    circuit::write_test_case(&circuit_repo, &test_dir, get_l2_block_test_case())?;
    Ok(())
}

fn main() {
    dotenv::dotenv().ok();
    let _guard = non_blocking_tracing::setup();
    Settings::init_default();
    log::debug!("{:?}", Settings::get());
    match run() {
        Ok(_) => println!("export_circuit test_case generated"),
        Err(e) => panic!("{:#?}", e),
    }
}
