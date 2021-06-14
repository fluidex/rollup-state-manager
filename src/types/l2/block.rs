use super::tx::TxType;
use crate::types::merkle_tree::MerklePath;
use crate::types::primitives::Fr;

#[derive(Clone)]
pub struct L2Block {
    pub old_root: Fr,
    pub new_root: Fr,
    pub txs_type: Vec<TxType>,
    pub encoded_txs: Vec<Vec<Fr>>,
    pub balance_path_elements: Vec<[MerklePath; 4]>,
    pub order_path_elements: Vec<[MerklePath; 2]>,
    pub account_path_elements: Vec<[MerklePath; 2]>,
    pub order_roots: Vec<[Fr; 2]>,
    pub old_account_roots: Vec<Fr>,
    pub new_account_roots: Vec<Fr>,
}
