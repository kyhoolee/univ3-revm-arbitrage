use clap::Parser;
use univ3_revm_arbitrage::chain::actors::get_chain_actors;
use univ3_revm_arbitrage::types::{load_chain_config, ChainConfig};
use univ3_revm_arbitrage::core::{
    chain_call::run_chain_call,
    chain_anvil::run_chain_anvil,
    chain_revm::run_chain_revm,
    chain_revm_cached::run_chain_revm_cached,
    chain_revm_cached::run_chain_revm_snapshot_parallel, // ADD LINE
    chain_revm_quoter::run_chain_revm_quoter,
    chain_arbitrage::run_chain_arbitrage,
    chain_validate::run_chain_validate,
};

#[derive(Parser, Debug)]
#[command(author = "Kyhoolee", version = "1.0", about = "Simulate EVM quote/arbitrage")]
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
    let actors = get_chain_actors(&args.chain);


    // Dispatch logic dựa theo --method
    match args.method.as_str() {
        "call" => run_chain_call(&config, &actors).await?,
        "anvil" => run_chain_anvil(&config, &actors).await?,
        "revm" => run_chain_revm(&config, &actors).await?,
        "revm_cached" => run_chain_revm_cached(&config, &actors).await?,
        "revm_cached_parallel" => run_chain_revm_snapshot_parallel(&config, &actors).await?, // ADD LINE
        "revm_quoter" => run_chain_revm_quoter(&config, &actors).await?,
        "arbitrage" => run_chain_arbitrage(&config, &actors).await?,
        "validate" => run_chain_validate(&config, &actors).await?,

        _ => eprintln!("Unknown method: {}", args.method),
    }


    Ok(())
}
