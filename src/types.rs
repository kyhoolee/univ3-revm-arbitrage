use std::collections::HashMap;
use anyhow::{anyhow, Result};
use serde::Deserialize;

use alloy::{
    primitives::{Address, U256},
    uint,
};

pub static ONE_ETHER: U256 = uint!(1_000_000_000_000_000_000_U256);

/// Struct ánh xạ nội dung từ file `eth.toml`, `avax.toml`, ...
#[derive(Debug, Deserialize)]
pub struct ChainConfigRaw {
    pub chain_id: u64,
    pub rpc_url: String,
    pub rpc_urls: Option<Vec<String>>, // NEW
    pub gas_multiplier: f64,
    pub tokens: HashMap<String, String>,
}

/// Struct dùng trong toàn bộ codebase sau khi parse địa chỉ thành `Address`
#[derive(Debug)]
pub struct ChainConfig {
    pub chain_id: u64,
    pub rpc_url: String,
    pub rpc_urls: Vec<String>, // NEW
    pub gas_multiplier: f64,
    pub tokens: HashMap<String, Address>,
}

impl ChainConfig {
    pub fn addr(&self, key: &str) -> Result<Address> {
        self.tokens
            .get(key)
            .copied()
            .ok_or_else(|| anyhow!("Missing address for token/key: {}", key))
    }
}

/// Load + parse file TOML thành `ChainConfig`
pub fn load_chain_config(path: &str) -> Result<ChainConfig> {
    let raw_content = std::fs::read_to_string(path)?;
    let raw: ChainConfigRaw = toml::from_str(&raw_content)?;

    let mut parsed = HashMap::new();
    for (k, v) in raw.tokens.iter() {
        let addr = v.parse::<Address>()
            .map_err(|e| anyhow!("Invalid address for {}: {}", k, e))?;
        parsed.insert(k.clone(), addr);
    }

    Ok(ChainConfig {
        chain_id: raw.chain_id,
        rpc_url: raw.rpc_url.clone(),
        rpc_urls: raw.rpc_urls.unwrap_or_else(|| vec![raw.rpc_url.clone()]),
        gas_multiplier: raw.gas_multiplier,
        tokens: parsed,
    })
}
