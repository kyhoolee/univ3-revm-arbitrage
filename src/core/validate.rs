use std::sync::Arc;
use std::ops::{Div, Mul};
use std::str::FromStr;
use anyhow::Result;
use alloy::providers::Provider;
use alloy::{
    primitives::{Bytes, U256},
    providers::ProviderBuilder,
};
use revm::primitives::Bytecode;

use crate::types::{ChainConfig, ONE_ETHER};
use crate::source::{builder::volumes, abi::*, builder::build_tx};
use crate::core::db::*;

/// So sánh kết quả quote từ `eth_call` và `revm`
/// Dùng custom UniV3Quoter để đảm bảo REVM phản hồi `amountOut` đúng
pub async fn run_eth_validate(config: &ChainConfig) -> Result<()> {
    // 1️⃣ Setup RPC và provider
    let provider = ProviderBuilder::new()
        .on_http(config.rpc_url.parse()?);
    let provider = Arc::new(provider);
    // Tạo REVM cache database từ provider

    let mut cache_db = init_cache_db(provider.clone());

    let base_fee = provider.get_gas_price().await?;
    let base_fee = base_fee.mul(110).div(100); // +10% để đảm bảo an toàn fee

    // 2️⃣ Load address từ config
    let from = config.addr("ME")?;
    let token_in = config.addr("WETH")?;
    let token_out = config.addr("USDC")?;
    let pool = config.addr("POOL_3000")?;
    let quoter = config.addr("QUOTER")?;
    let custom_quoter = config.addr("CUSTOM_QUOTER")?;

    // 3️⃣ Chuẩn bị volume và mock data
    let volumes = volumes(U256::ZERO, ONE_ETHER.div(U256::from(10)), 10);

    init_account(from, &mut cache_db, provider.clone()).await?;
    init_account(pool, &mut cache_db, provider.clone()).await?;

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
        let call_calldata = quote_calldata(token_in, token_out, volume, 3000);
        let tx = build_tx(quoter, from, call_calldata, base_fee);
        let call_response = provider.call(&tx).await?;
        let call_amount_out = decode_quote_response(call_response)?;

        let revm_calldata = get_amount_out_calldata(pool, token_in, token_out, volume);
        let revm_response = revm_revert(from, custom_quoter, revm_calldata, &mut cache_db)?;
        let revm_amount_out = decode_get_amount_out_response(revm_response)?;

        println!(
            "{} WETH -> USDC | REVM: {} | ETH_CALL: {}",
            volume, revm_amount_out, call_amount_out
        );

        // 5️⃣ Xác minh chính xác
        assert_eq!(revm_amount_out, call_amount_out);
    }

    Ok(())
}
