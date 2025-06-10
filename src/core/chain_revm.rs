use std::sync::Arc;
use std::ops::Div;
use anyhow::Result;
use alloy::{
    primitives::U256,
    providers::{Provider, ProviderBuilder},
    transports::http::{Http, Client},
};

use crate::types::{ChainConfig, ONE_ETHER};
use crate::source::{builder::volumes, abi::*};
use crate::core::db::{init_cache_db, init_account, revm_call};
use crate::core::logger::{measure_start, measure_end};
use crate::chain::actors::ChainActors; // cần thêm import
use crate::core::provider::MultiProvider;

/// Mô phỏng quote swap từ UniswapV3 bằng `REVM` (multi-chain)
pub async fn run_chain_revm(config: &ChainConfig, actors: &ChainActors) -> Result<()> {
    // 1️⃣ Khởi tạo JSON-RPC provider để fetch bytecode từ chain thực
    // let provider = ProviderBuilder::new()
    //     .on_http(config.rpc_url.parse()?);
    // let provider = Arc::new(provider);

    // // 2️⃣ Tạo REVM CacheDB từ provider chain thực
    // let mut cache_db = init_cache_db(provider.clone());
    let multi_provider = MultiProvider::new(&config.rpc_urls);
    println!("MultiProvider with {} providers", multi_provider.len());

    // let base_fee = provider.get_gas_price().await?;
    let (provider, url) = multi_provider.next();
    let base_fee = provider.get_gas_price().await?;

    let mut cache_db = init_cache_db(&multi_provider);

    // 3️⃣ Địa chỉ dùng trong giao dịch (từ config + actors)
    let from = config.addr("ME")?;
    let token_in = config.addr(actors.native_token_key)?;
    let token_out = config.addr(actors.stable_token_key)?;
    let quoter = config.addr(actors.quoter_key)?;

    // 4️⃣ Chuẩn bị volume swap để benchmark
    let volumes = volumes(U256::ZERO, ONE_ETHER.div(U256::from(10)), 100);

    // 5️⃣ Tải bytecode của contract quoter vào REVM memory state
    init_account(quoter, &mut cache_db, &multi_provider).await?;

    // 6️⃣ Mô phỏng lần đầu
    let start = measure_start("revm_first");
    let calldata = quote_calldata(token_in, token_out, volumes[0], actors.default_fee);
    let response = revm_call(from, quoter, calldata, &mut cache_db)?;
    let amount_out = decode_quote_response(response)?;
    println!(
        "{} {} -> {} {}",
        volumes[0], actors.native_token_key, actors.stable_token_key, amount_out
    );
    measure_end(start);

    // 7️⃣ Mô phỏng lặp nhiều volume để benchmark
    let start = measure_start("revm_loop");
    for (index, volume) in volumes.into_iter().enumerate() {
        let calldata = quote_calldata(token_in, token_out, volume, actors.default_fee);
        let response = revm_call(from, quoter, calldata, &mut cache_db)?;
        let amount_out = decode_quote_response(response)?;

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
