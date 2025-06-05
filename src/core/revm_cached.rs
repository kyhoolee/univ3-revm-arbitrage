use std::sync::Arc;
use std::ops::Div;
use std::str::FromStr;
use anyhow::Result;
use alloy::{
    primitives::{Bytes, U256},
    providers::ProviderBuilder,
};
use revm::primitives::Bytecode;

use crate::types::{ChainConfig, ONE_ETHER};
use crate::source::{builder::volumes, abi::*};
use crate::core::db::*;
use crate::core::logger::{measure_start, measure_end};

/// REVM mô phỏng UniswapV3 với dữ liệu cache:
/// - Gán bytecode ERC20 giả cho token
/// - Thêm balance thủ công vào REVM storage
pub async fn run_eth_revm_cached(config: &ChainConfig) -> Result<()> {
    // 1️⃣ Tạo JSON-RPC provider để lấy pool/quoter bytecode thật
    let provider = ProviderBuilder::new()
        .on_http(config.rpc_url.parse()?);
    let provider = Arc::new(provider);

    // 2️⃣ Tạo cache db từ REVM memory
    let mut cache_db = init_cache_db(provider.clone());

    // 3️⃣ Địa chỉ cần dùng
    let from = config.addr("ME")?;
    let token_in = config.addr("WETH")?;
    let token_out = config.addr("USDC")?;
    let quoter = config.addr("QUOTER")?;
    let pool = config.addr("POOL_3000")?;

    // 4️⃣ Chuẩn bị volume để benchmark
    let volumes = volumes(U256::ZERO, ONE_ETHER.div(U256::from(10)), 100);

    // 5️⃣ Tải bytecode thật cho quoter + pool vào memory state
    init_account(quoter, &mut cache_db, provider.clone()).await?;
    init_account(pool, &mut cache_db, provider.clone()).await?;

    // 6️⃣ Tải bytecode ERC20 giả
    let mocked_erc20 = include_str!("../bytecode/generic_erc20.hex");
    let mocked_erc20 = Bytes::from_str(mocked_erc20)?;
    let mocked_erc20 = Bytecode::new_raw(mocked_erc20);

    // 7️⃣ Gán bytecode giả cho token (WETH, USDC)
    init_account_with_bytecode(token_in, mocked_erc20.clone(), &mut cache_db)?;
    init_account_with_bytecode(token_out, mocked_erc20.clone(), &mut cache_db)?;

    // 8️⃣ Gán balance giả trong storage
    let mocked_balance = U256::MAX / U256::from(2);
    insert_mapping_storage_slot(token_in, U256::ZERO, pool, mocked_balance, &mut cache_db)?;
    insert_mapping_storage_slot(token_out, U256::ZERO, pool, mocked_balance, &mut cache_db)?;

    // 9️⃣ Quote lần đầu
    let start = measure_start("revm_cached_first");
    let calldata = quote_calldata(token_in, token_out, volumes[0], 3000);
    let response = revm_call(from, quoter, calldata, &mut cache_db)?;
    let amount_out = decode_quote_response(response)?;
    println!("{} WETH -> USDC {}", volumes[0], amount_out);
    measure_end(start);

    // 🔟 Quote nhiều volume để test hiệu suất
    let start = measure_start("revm_cached_loop");
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
