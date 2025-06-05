use std::sync::Arc;
use std::ops::Div;
use anyhow::Result;
use alloy::{
    primitives::U256,
    providers::{ProviderBuilder, RootProvider},
    transports::http::{Http, Client},
};

use crate::types::{ChainConfig, ONE_ETHER};
use crate::source::{builder::volumes, abi::*};
use crate::core::db::{init_cache_db, init_account, revm_call};
use crate::core::logger::{measure_start, measure_end};

/// Mô phỏng quote swap từ UniswapV3 bằng `REVM` (EVM giả lập offchain)
/// Dùng calldata quoteExactInputSingle() và simulate bằng REVM trên memory state
pub async fn run_eth_revm(config: &ChainConfig) -> Result<()> {
    // 1️⃣ Khởi tạo JSON-RPC provider để fetch bytecode từ chain thực
    let provider = ProviderBuilder::new()
        .on_http(config.rpc_url.parse()?);
    let provider = Arc::new(provider);

    // 2️⃣ Tạo REVM CacheDB từ provider chain thực
    let mut cache_db = init_cache_db(provider.clone());

    // 3️⃣ Địa chỉ dùng trong giao dịch (từ config)
    let from = config.addr("ME")?;
    let token_in = config.addr("WETH")?;
    let token_out = config.addr("USDC")?;
    let quoter = config.addr("QUOTER")?;

    // 4️⃣ Chuẩn bị volume swap để benchmark
    let volumes = volumes(U256::ZERO, ONE_ETHER.div(U256::from(10)), 100);

    // 5️⃣ Tải bytecode của contract quoter vào REVM memory state
    init_account(quoter, &mut cache_db, provider.clone()).await?;

    // 6️⃣ Mô phỏng lần đầu: WETH → USDC với volume đầu tiên
    let start = measure_start("revm_first");
    let calldata = quote_calldata(token_in, token_out, volumes[0], 3000);
    let response = revm_call(from, quoter, calldata, &mut cache_db)?;
    let amount_out = decode_quote_response(response)?;
    println!("{} WETH -> USDC {}", volumes[0], amount_out);
    measure_end(start);

    // 7️⃣ Mô phỏng lặp nhiều volume để benchmark hiệu suất
    let start = measure_start("revm_loop");
    for (index, volume) in volumes.into_iter().enumerate() {
        let calldata = quote_calldata(token_in, token_out, volume, 3000);
        let response = revm_call(from, quoter, calldata, &mut cache_db)?;
        let amount_out = decode_quote_response(response)?;

        if index % 20 == 0 {
            println!("{} WETH -> USDC {}", volume, amount_out);
        }
    }
    measure_end(start);

    Ok(())
}
