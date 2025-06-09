use std::sync::Arc;
use std::ops::Div;
use anyhow::Result;
use alloy::providers::Provider;
use alloy::{
    node_bindings::Anvil,
    primitives::U256,
    providers::{ProviderBuilder, RootProvider},
    transports::http::reqwest::Url,
};

use crate::types::{ChainConfig, ONE_ETHER};
use crate::source::{builder::volumes, abi::*, builder::build_tx};
use crate::core::logger::{measure_start, measure_end};

use crate::chain::actors::ChainActors; // cần thêm import

/// Chạy mô phỏng quote thông qua Anvil forked mainnet (multi-chain)
pub async fn run_chain_anvil(config: &ChainConfig, actors: &ChainActors) -> Result<()> {
    // 1️⃣ Parse RPC URL và khởi tạo provider thật
    let rpc_url = config.rpc_url.parse::<Url>()?;
    let provider = ProviderBuilder::new().on_http(rpc_url.clone());
    let provider = Arc::new(provider);

    // 2️⃣ Lấy base_fee và block height để tạo fork
    let base_fee = provider.get_gas_price().await?;
    let fork_block = provider.get_block_number().await?;

    // 3️⃣ Tạo instance Anvil fork từ block thực
    let anvil = Anvil::new()
        .fork(rpc_url)
        .fork_block_number(fork_block)
        .block_time(1_u64)
        .spawn();

    // 4️⃣ Tạo provider kết nối đến Anvil local fork
    let anvil_provider = ProviderBuilder::new()
        .on_http(anvil.endpoint().parse::<Url>()?);
    let anvil_provider = Arc::new(anvil_provider);

    // 5️⃣ Lấy thông tin token / quoter / from từ config + actors
    let from = config.addr("ME")?;
    let token_in = config.addr(actors.native_token_key)?;
    let token_out = config.addr(actors.stable_token_key)?;
    let quoter = config.addr(actors.quoter_key)?;

    // 6️⃣ Chuẩn bị volumes để benchmark
    let volumes = volumes(U256::ZERO, ONE_ETHER.div(U256::from(10)), 100);

    // 7️⃣ Quote lần đầu
    let start = measure_start("anvil_first");
    let calldata = quote_calldata(token_in, token_out, volumes[0], actors.default_fee);
    let tx = build_tx(quoter, from, calldata, base_fee);
    let response = anvil_provider.call(&tx).await?;
    let amount_out = decode_quote_response(response)?;
    println!(
        "{} {} -> {} {}",
        volumes[0], actors.native_token_key, actors.stable_token_key, amount_out
    );
    measure_end(start);

    // 8️⃣ Loop để benchmark nhiều volume
    let start = measure_start("anvil_loop");
    for (index, volume) in volumes.into_iter().enumerate() {
        let calldata = quote_calldata(token_in, token_out, volume, actors.default_fee);
        let tx = build_tx(quoter, from, calldata, base_fee);
        let response = anvil_provider.call(&tx).await?;
        let amount_out = decode_quote_response(response)?;
        if index % 20 == 0 {
            println!(
                "{} {} -> {} {}",
                volume, actors.native_token_key, actors.stable_token_key, amount_out
            );
        }
    }
    measure_end(start);

    drop(anvil); // cleanup anvil instance

    Ok(())
}
