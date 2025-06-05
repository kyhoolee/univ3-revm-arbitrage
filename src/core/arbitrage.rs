use std::sync::Arc;
use std::{ops::Div, str::FromStr};

use alloy::signers::k256::elliptic_curve::consts::U2;
use anyhow::Result;
use alloy::{
    primitives::{Bytes, U256},
    providers::ProviderBuilder,
};
use revm::primitives::Bytecode;

use crate::types::{ChainConfig, ONE_ETHER};
use crate::source::{abi::*, builder::volumes};
use crate::core::db::*;

/// Mô phỏng back-and-forth arbitrage WETH -> USDC -> WETH
/// Dùng custom UniV3Quoter để quote offchain qua REVM
pub async fn run_eth_arbitrage(config: &ChainConfig) -> Result<()> {
    let provider = ProviderBuilder::new()
        .on_http(config.rpc_url.parse()?);
    let provider = Arc::new(provider);
    // Tạo REVM cache database từ provider

    let mut cache_db = init_cache_db(provider.clone());

    let from = config.addr("ME")?;
    let token_in = config.addr("WETH")?;
    let token_out = config.addr("USDC")?;
    let pool1 = config.addr("POOL_500")?;
    let pool2 = config.addr("POOL_3000")?;
    let quoter = config.addr("CUSTOM_QUOTER")?;

    let mocked_erc20 = include_str!("../bytecode/generic_erc20.hex");
    let mocked_erc20 = Bytecode::new_raw(Bytes::from_str(mocked_erc20)?);

    let mocked_quoter = include_str!("../bytecode/uni_v3_quoter.hex");
    let mocked_quoter = Bytecode::new_raw(Bytes::from_str(mocked_quoter)?);

    // Khởi tạo REVM state cho quoter, pool, token
    init_account(from, &mut cache_db, provider.clone()).await?;
    init_account(pool1, &mut cache_db, provider.clone()).await?;
    init_account(pool2, &mut cache_db, provider.clone()).await?;

    init_account_with_bytecode(token_in, mocked_erc20.clone(), &mut cache_db)?;
    init_account_with_bytecode(token_out, mocked_erc20.clone(), &mut cache_db)?;
    init_account_with_bytecode(quoter, mocked_quoter, &mut cache_db)?;

    let mocked_balance = U256::MAX.div(U256::from(2));
    for &pool in &[pool1, pool2] {
        insert_mapping_storage_slot(token_in, U256::ZERO, pool, mocked_balance, &mut cache_db)?;
        insert_mapping_storage_slot(token_out, U256::ZERO, pool, mocked_balance, &mut cache_db)?;
    }

    // Loop từng volume để kiểm tra arbitrage
    let vols = volumes(U256::ZERO, ONE_ETHER.div(U256::from(10)), 100);
    for vol in vols {
        let calldata1 = get_amount_out_calldata(pool1, token_in, token_out, vol);
        let resp1 = revm_revert(from, quoter, calldata1, &mut cache_db)?;
        let token_out_amount = decode_get_amount_out_response(resp1)?;

        let calldata2 = get_amount_out_calldata(pool2, token_out, token_in, U256::from(token_out_amount));
        let resp2 = revm_revert(from, quoter, calldata2, &mut cache_db)?;
        let token_in_back = decode_get_amount_out_response(resp2)?;
        let token_in_back = U256::from(token_in_back);

        println!(
            "{} WETH → USDC {} → WETH {}",
            vol, token_out_amount, token_in_back
        );

        if token_in_back > vol {
            let profit = token_in_back - vol;
            println!("✅ Arbitrage profit: {} WETH", profit);
        } else {
            println!("❌ No profit");
        }
    }

    Ok(())
}
