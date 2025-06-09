use std::sync::Arc;
use std::ops::Div;
use anyhow::Result;
use alloy::{
    eips::BlockId,
    primitives::U256,
    providers::{Provider, ProviderBuilder},
};
use crate::{source::build_tx_avalanche, types::{ChainConfig, ONE_ETHER}};
use crate::source::{builder::volumes, abi::quote_calldata, builder::build_tx};
use crate::core::logger::{measure_start, measure_end};
use crate::chain::actors::ChainActors;

/// Mô phỏng quote swap bằng eth_call (multi-chain)
pub async fn run_chain_call(config: &ChainConfig, actors: &ChainActors) -> Result<()> {
    let provider = ProviderBuilder::new()
        .on_http(config.rpc_url.parse()?);
    let provider = Arc::new(provider);

    let base_fee = provider.get_gas_price().await?;
    let from = config.addr("ME")?;
    let token_in = config.addr(actors.native_token_key)?;
    let token_out = config.addr(actors.stable_token_key)?;
    let quoter = config.addr(actors.quoter_key)?;

    // print address 
    // println!("From address: {}", from);
    // println!("Token in address: {}", token_in);
    // println!("Token out address: {}", token_out);
    // println!("Quoter address: {}", quoter);

    let volumes = volumes(U256::ZERO, ONE_ETHER.div(U256::from(10)), 100);

    let start = measure_start("chain_call");

    for (index, volume) in volumes.into_iter().enumerate() {
        let calldata = quote_calldata(token_in, token_out, volume, actors.default_fee);
        
        let tx = build_tx(quoter, from, calldata, base_fee);

        // println!("\nTransaction: {:?}", tx);
        // println!("Calling provider with transaction...");

        // let response = provider.call(&tx).await?;
        let response = provider.call(&tx).block(BlockId::latest()).await?;

        let amount_out = crate::source::abi::decode_quote_response(response)?;

        // println!("Response: {:?}", response);
        // println!("Amount out: {}", amount_out);

        if index % 20 == 0 {
            println!(
                "{} {} -> {} {}",
                volume, actors.native_token_key, actors.stable_token_key, amount_out
            );
        }
    }

    measure_end(start);
    Ok(())
}
