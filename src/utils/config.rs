use once_cell::sync::OnceCell;
use std::sync::Arc;
use serde::Deserialize;
use tokio::sync::Mutex;
use toml;

// Struct that represents the actual config and requirements
#[derive(Clone, Debug, Deserialize)]
pub struct StreamConfig {
    pub provider: String,
    pub url: String,
    #[serde(rename = "type")]
    pub stream_type: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Params {
    pub symbols: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub stream: StreamConfig,
    pub params: Params,
}

pub static CONFIG: OnceCell<Arc<Mutex<Config>>> = OnceCell::new();

// Load the configuration from the config.toml file
pub fn load_config(){
    let config_str = std::fs::read_to_string("config.toml").expect("Unable to read config file");
    let config: Config = toml::from_str(&config_str).expect("Unable to parse config file");

    // Set the config in the OnceCell
    CONFIG.set(Arc::new(Mutex::new(config.clone()))).expect("Failed to set the config");
}