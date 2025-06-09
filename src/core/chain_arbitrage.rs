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
use crate::source::{abi::*, builder::volumes};
use crate::core::db::*;
use crate::core::logger::{measure_start, measure_end};
use crate::chain::actors::ChainActors;

/// Mô phỏng back-and-forth arbitrage Native -> Stable -> Native
/// Dùng custom UniV3Quoter để quote offchain qua REVM
pub async fn run_chain_arbitrage(config: &ChainConfig, actors: &ChainActors) -> Result<()> {
    let provider = ProviderBuilder::new()
        .on_http(config.rpc_url.parse()?);
    let provider = Arc::new(provider);

    let mut cache_db = init_cache_db(provider.clone());

    let from = config.addr("ME")?;
    let token_in = config.addr(actors.native_token_key)?;
    let token_out = config.addr(actors.stable_token_key)?;
    let pool1 = config.addr(actors.pool_500_key.expect("pool_500_key required"))?;
    let pool2 = config.addr(actors.pool_3000_key.expect("pool_3000_key required"))?;
    let quoter = config.addr(actors.custom_quoter_key.expect("custom_quoter_key required"))?;

    // Load bytecode
    let mocked_erc20 = include_str!("../bytecode/generic_erc20.hex");
    let mocked_erc20 = Bytecode::new_raw(Bytes::from_str(mocked_erc20)?);

    let mocked_quoter = include_str!("../bytecode/uni_v3_quoter.hex");
    let mocked_quoter = Bytecode::new_raw(Bytes::from_str(mocked_quoter)?);

    // Init accounts
    init_account(from, &mut cache_db, provider.clone()).await?;
    init_account(pool1, &mut cache_db, provider.clone()).await?;
    init_account(pool2, &mut cache_db, provider.clone()).await?;

    init_account_with_bytecode(token_in, mocked_erc20.clone(), &mut cache_db)?;
    init_account_with_bytecode(token_out, mocked_erc20.clone(), &mut cache_db)?;
    init_account_with_bytecode(quoter, mocked_quoter, &mut cache_db)?;

    // Insert fake balances
    let mocked_balance = U256::MAX / U256::from(2);
    for &pool in &[pool1, pool2] {
        insert_mapping_storage_slot(token_in, U256::ZERO, pool, mocked_balance, &mut cache_db)?;
        insert_mapping_storage_slot(token_out, U256::ZERO, pool, mocked_balance, &mut cache_db)?;
    }

    // Arbitrage loop
    let vols = volumes(U256::ZERO, ONE_ETHER.div(U256::from(10)), 100);
    measure_start("chain_arbitrage"); // Optional: đo log

    for vol in vols {
        let calldata1 = get_amount_out_calldata(pool1, token_in, token_out, vol);
        let resp1 = revm_revert(from, quoter, calldata1, &mut cache_db)?;
        let token_out_amount = decode_get_amount_out_response(resp1)?;

        let calldata2 = get_amount_out_calldata(pool2, token_out, token_in, U256::from(token_out_amount));
        let resp2 = revm_revert(from, quoter, calldata2, &mut cache_db)?;
        let token_in_back = decode_get_amount_out_response(resp2)?;
        let token_in_back = U256::from(token_in_back);

        println!(
            "{} {} → {} {} → {} {}",
            vol, actors.native_token_key,
            token_out_amount, actors.stable_token_key,
            token_in_back, actors.native_token_key
        );

        if token_in_back > vol {
            let profit = token_in_back - vol;
            println!("✅ Arbitrage profit: {} {}", profit, actors.native_token_key);
        } else {
            println!("❌ No profit");
        }
    }

    measure_end(measure_start("chain_arbitrage")); // Optional đo log
    Ok(())
}
