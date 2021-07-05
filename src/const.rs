#[cfg(feature = "persist_sled")]
pub mod sled_db {
    pub const KAFKA_OFFSET_KEY: &str = "kafka_offset";
    pub const ACCOUNTTREE_KEY: &str = "account_tree";
    pub const ACCOUNTSTATES_KEY: &str = "account_states";
    pub const BALANCETREES_KEY: &str = "balance_trees";
    pub const ORDERTREES_KEY: &str = "order_trees";
    pub const ORDERSTATES_KEY: &str = "order_states";
}
