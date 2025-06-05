use clap::Parser;
use univ3_revm_arbitrage::types::{load_chain_config, ChainConfig};
use univ3_revm_arbitrage::core::{
    call::run_eth_call,
    revm::run_eth_revm,
    anvil::run_eth_anvil,            
    revm_cached::run_eth_revm_cached,
    revm_quoter::run_eth_revm_quoter,
    arbitrage::run_eth_arbitrage,
    validate::run_eth_validate,
};

#[derive(Parser, Debug)]
#[command(author = "Your Name", version = "1.0", about = "Simulate EVM quote/arbitrage")]
struct Args {
    /// Tên chain (eth, avax, ronin)
    #[arg(long, default_value = "eth")]
    chain: String,

    /// Logic cần chạy (call, revm, anvil, arbitrage, validate)
    #[arg(long, default_value = "call")]
    method: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Load config từ file src/config/<chain>.toml
    let config_path = format!("src/config/{}.toml", args.chain);
    let config: ChainConfig = load_chain_config(&config_path)?;

    // Dispatch logic dựa theo --method
    match args.method.as_str() {
        "call" => run_eth_call(&config).await?,
        "revm" => run_eth_revm(&config).await?,
        "anvil" => run_eth_anvil(&config).await?,
        "revm_cached" => run_eth_revm_cached(&config).await?, 
        "revm_quoter" => run_eth_revm_quoter(&config).await?,
        "arbitrage" => run_eth_arbitrage(&config).await?,
        "validate" => run_eth_validate(&config).await?,
        _ => eprintln!("Unknown method: {}", args.method),
    }

    Ok(())
}
