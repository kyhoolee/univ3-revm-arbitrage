use std::sync::Arc;
use std::ops::Div;
use anyhow::Result;
use alloy::{
    primitives::U256,
    providers::{Provider, ProviderBuilder},
};
use crate::types::ChainConfig;
use crate::source::{builder::*, abi::*};
use crate::core::logger::{measure_start, measure_end};

pub async fn run_eth_call(config: &ChainConfig) -> Result<()> {
    // Setup provider
    let provider = ProviderBuilder::new()
        .on_http(config.rpc_url.parse()?);
    let provider = Arc::new(provider);

    // Lấy base fee
    let base_fee = provider.get_gas_price().await?;
    let from = config.addr("ME")?;
    let token_in = config.addr("WETH")?;
    let token_out = config.addr("USDC")?;
    let quoter = config.addr("QUOTER")?;

    // Chuẩn bị volumes
    let volumes = volumes(U256::ZERO, crate::types::ONE_ETHER.div(U256::from(10)), 100);

    let start = measure_start("eth_call");

    for (index, volume) in volumes.into_iter().enumerate() {
        let calldata = quote_calldata(token_in, token_out, volume, 3000);
        let tx = build_tx(quoter, from, calldata, base_fee);
        let response = provider.call(&tx).await?;
        let amount_out = decode_quote_response(response)?;

        if index % 20 == 0 {
            println!("{} WETH -> USDC {}", volume, amount_out);
        }
    }

    measure_end(start);
    Ok(())
}

