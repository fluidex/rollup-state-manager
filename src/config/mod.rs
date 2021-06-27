use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub brokers: String,
    pub prover_cluster_db: String,
    pub rollup_state_manager_db: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            brokers: "127.0.0.1:9092".to_string(),
            prover_cluster_db: Default::default(),
            rollup_state_manager_db: Default::default(),
        }
    }
}
