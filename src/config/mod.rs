use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Settings {
    pub brokers: String,
    pub prover_cluster_db: String,
    pub rollup_state_manager_db: String,
}
