use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub brokers: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            brokers: "127.0.0.1:9092".to_string(),
        }
    }
}
