use std::sync::Arc;
use std::ops::Div;
use std::str::FromStr;
use anyhow::Result;
use alloy::{
    primitives::{Bytes, U256},
    providers::{Provider, ProviderBuilder},
};
use revm::primitives::Bytecode;

use crate::types::{ChainConfig, ONE_ETHER};
use crate::source::{builder::volumes, abi::*};
use crate::core::db::*;
use crate::core::logger::{measure_start, measure_end};
use crate::chain::actors::ChainActors;
use crate::core::provider::MultiProvider;

/// REVM mô phỏng UniswapV3 với dữ liệu cache:
/// - Gán bytecode ERC20 giả cho token
/// - Thêm balance thủ công vào REVM storage
pub async fn run_chain_revm_cached(config: &ChainConfig, actors: &ChainActors) -> Result<()> {
    // 1️⃣ Tạo JSON-RPC provider
    // let provider = ProviderBuilder::new()
    //     .on_http(config.rpc_url.parse()?);
    // let provider = Arc::new(provider);

    // // 2️⃣ Tạo cache db từ REVM memory
    // let mut cache_db = init_cache_db(provider.clone());
    // let mut cache_db = init_cache_db(provider.clone());
    let multi_provider = MultiProvider::new(&config.rpc_urls);
    println!("MultiProvider with {} providers", multi_provider.len());

    // let base_fee = provider.get_gas_price().await?;
    let (provider, url) = multi_provider.next();
    let base_fee = provider.get_gas_price().await?;

    let mut cache_db = init_cache_db(&multi_provider);

    // 3️⃣ Địa chỉ cần dùng
    let from = config.addr("ME")?;
    let token_in = config.addr(actors.native_token_key)?;
    let token_out = config.addr(actors.stable_token_key)?;
    let quoter = config.addr(actors.quoter_key)?;
    let pool = config.addr(actors.pool_3000_key.expect("Missing pool_3000_key for this chain"))?;


    println!("from={:?} token_in={:?} token_out={:?} quoter={:?} pool={:?}", from, token_in, token_out, quoter, pool);

    // 4️⃣ Chuẩn bị volume để benchmark
    let volumes = volumes(U256::ZERO, ONE_ETHER.div(U256::from(10)), 100);

    // 5️⃣ Tải bytecode thật cho quoter + pool vào memory state
    init_account(quoter, &mut cache_db, &multi_provider).await?;
    init_account(pool, &mut cache_db, &multi_provider).await?;

    // 6️⃣ Tải bytecode ERC20 giả
    let mocked_erc20 = include_str!("../bytecode/generic_erc20.hex");
    let mocked_erc20 = Bytes::from_str(mocked_erc20)?;
    let mocked_erc20 = Bytecode::new_raw(mocked_erc20);

    // 7️⃣ Gán bytecode giả cho token (WETH/WAVAX, USDC)
    init_account_with_bytecode(token_in, mocked_erc20.clone(), &mut cache_db)?;
    init_account_with_bytecode(token_out, mocked_erc20.clone(), &mut cache_db)?;

    // 8️⃣ Gán balance giả trong storage
    let mocked_balance = U256::MAX / U256::from(2);
    insert_mapping_storage_slot(token_in, U256::ZERO, pool, mocked_balance, &mut cache_db)?;
    insert_mapping_storage_slot(token_out, U256::ZERO, pool, mocked_balance, &mut cache_db)?;

    // 9️⃣ Quote lần đầu
    let start = measure_start("revm_cached_first");
    let calldata = quote_calldata(token_in, token_out, volumes[0], actors.default_fee);
    let response = revm_call(from, quoter, calldata, &mut cache_db)?;
    let amount_out = decode_quote_response(response)?;
    println!("{} {} -> {} {}", volumes[0], actors.native_token_key, actors.stable_token_key, amount_out);
    measure_end(start);

    // 🔟 Quote nhiều volume để test hiệu suất
    let start = measure_start("revm_cached_loop");
    for (index, volume) in volumes.into_iter().enumerate() {
        let calldata = quote_calldata(token_in, token_out, volume, actors.default_fee);
        let response = revm_call(from, quoter, calldata, &mut cache_db)?;
        let amount_out = decode_quote_response(response)?;
        if index % 20 == 0 {
            println!("{} {} -> {} {}", volume, actors.native_token_key, actors.stable_token_key, amount_out);
        }
    }
    measure_end(start);

    Ok(())
}
