use std::sync::Arc;
use std::ops::{Div, Mul};
use std::str::FromStr;
use alloy::eips::BlockId;
use anyhow::Result;
use alloy::{
    primitives::{Bytes, U256},
    providers::{Provider, ProviderBuilder},
};
use revm::primitives::Bytecode;

use crate::types::{ChainConfig, ONE_ETHER};
use crate::source::{builder::volumes, abi::*, builder::build_tx};
use crate::core::db::*;
use crate::core::logger::{measure_start, measure_end};
use crate::chain::actors::ChainActors;
use crate::core::provider::MultiProvider;

/// So sánh kết quả quote từ `eth_call` và `revm`
/// Dùng custom UniV3Quoter để đảm bảo REVM phản hồi `amountOut` đúng
pub async fn run_chain_validate(config: &ChainConfig, actors: &ChainActors) -> Result<()> {
    // 1️⃣ Setup RPC và provider
    // let provider = ProviderBuilder::new()
    //     .on_http(config.rpc_url.parse()?);
    // let provider = Arc::new(provider);

    // // Tạo REVM cache database từ provider
    // let mut cache_db = init_cache_db(provider.clone());
    let multi_provider = MultiProvider::new(&config.rpc_urls);
    println!("MultiProvider with {} providers", multi_provider.len());

    // let base_fee = provider.get_gas_price().await?;
    let (provider, url) = multi_provider.next();
    let base_fee = provider.get_gas_price().await?;

    let mut cache_db = init_cache_db(&multi_provider);

    let base_fee = provider.get_gas_price().await?;
    let base_fee = base_fee.mul(110).div(100); // +10%

    // 2️⃣ Load address từ config
    let from = config.addr("ME")?;
    let token_in = config.addr(actors.native_token_key)?;
    let token_out = config.addr(actors.stable_token_key)?;
    let pool = config.addr(actors.pool_3000_key.expect("pool_3000_key required"))?;
    let quoter = config.addr(actors.quoter_key)?;
    let custom_quoter = config.addr(actors.custom_quoter_key.expect("custom_quoter_key required"))?;

    // 3️⃣ Chuẩn bị volume và mock data
    let volumes = volumes(U256::ZERO, ONE_ETHER.div(U256::from(10)), 10);

    init_account(from, &mut cache_db, &multi_provider).await?;
    init_account(pool, &mut cache_db, &multi_provider).await?;

    let mocked_erc20 = include_str!("../bytecode/generic_erc20.hex");
    let mocked_erc20 = Bytecode::new_raw(Bytes::from_str(mocked_erc20)?);
    init_account_with_bytecode(token_in, mocked_erc20.clone(), &mut cache_db)?;
    init_account_with_bytecode(token_out, mocked_erc20.clone(), &mut cache_db)?;

    let mocked_balance = U256::MAX / U256::from(2);
    insert_mapping_storage_slot(token_in, U256::ZERO, pool, mocked_balance, &mut cache_db)?;
    insert_mapping_storage_slot(token_out, U256::ZERO, pool, mocked_balance, &mut cache_db)?;

    let mocked_custom_quoter = include_str!("../bytecode/uni_v3_quoter.hex");
    let mocked_custom_quoter = Bytecode::new_raw(Bytes::from_str(mocked_custom_quoter)?);
    init_account_with_bytecode(custom_quoter, mocked_custom_quoter, &mut cache_db)?;

    // 4️⃣ So sánh từng volume
    for volume in volumes {
        // Call onchain
        let call_calldata = quote_calldata(token_in, token_out, volume, actors.default_fee);
        let tx = build_tx(quoter, from, call_calldata, base_fee);
        let call_response = provider.call(&tx).block(BlockId::latest()).await?;
        let call_amount_out = decode_quote_response(call_response)?;

        // Call REVM
        let revm_calldata = get_amount_out_calldata(pool, token_in, token_out, volume);
        let revm_response = revm_revert(from, custom_quoter, revm_calldata, &mut cache_db)?;
        let revm_amount_out = decode_get_amount_out_response(revm_response)?;

        println!(
            "{} {} -> {} | REVM: {} | ETH_CALL: {}",
            volume, actors.native_token_key, actors.stable_token_key, revm_amount_out, call_amount_out
        );

        // 5️⃣ Xác minh chính xác
        assert_eq!(revm_amount_out, call_amount_out);
    }

    measure_end(measure_start("chain_validate")); // Optional đo log nếu muốn
    Ok(())
}
