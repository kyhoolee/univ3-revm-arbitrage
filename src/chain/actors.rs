pub struct ChainActors {
    pub native_token_key: &'static str,
    pub stable_token_key: &'static str,
    pub quoter_key: &'static str,
    pub custom_quoter_key: Option<&'static str>,
    pub pool_500_key: Option<&'static str>,
    pub pool_3000_key: Option<&'static str>,
    pub default_fee: u32,
}

pub fn get_chain_actors(chain_name: &str) -> ChainActors {
    match chain_name {
        "eth" => ChainActors {
            native_token_key: "WETH",
            stable_token_key: "USDC",
            quoter_key: "QUOTER",
            custom_quoter_key: Some("CUSTOM_QUOTER"),
            pool_500_key: Some("POOL_500"),
            pool_3000_key: Some("POOL_3000"),
            default_fee: 3000,
        },
        "avax" => ChainActors {
            native_token_key: "WAVAX",
            stable_token_key: "USDC",
            quoter_key: "QUOTER",
            custom_quoter_key: Some("CUSTOM_QUOTER"), // nếu chưa có thì để None
            pool_500_key: Some("POOL_500"),
            pool_3000_key: Some("POOL_3000"),
            default_fee: 3000, // tùy DEX
        },
        "ronin" => ChainActors {
            native_token_key: "WRON",
            stable_token_key: "USDC",
            quoter_key: "QUOTER",
            custom_quoter_key: Some("CUSTOM_QUOTER"),
            pool_500_key: Some("POOL_500"),
            pool_3000_key: Some("POOL_3000"),
            default_fee: 3000,
        },
        _ => panic!("Unknown chain"),
    }
}
