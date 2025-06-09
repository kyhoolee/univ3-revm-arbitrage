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
use crate::chain::actors::ChainActors;

/// REVM chạy quote bằng custom UniV3Quoter contract (trả kết quả qua revert), multi-chain version
pub async fn run_chain_revm_quoter(config: &ChainConfig, actors: &ChainActors) -> Result<()> {
    // 1️⃣ Setup provider và cache db
    let provider = ProviderBuilder::new()
        .on_http(config.rpc_url.parse()?);
    let provider = Arc::new(provider);

    let mut cache_db = init_cache_db(provider.clone());

    // 2️⃣ Đọc address từ config
    let from = config.addr("ME")?;
    let token_in = config.addr(actors.native_token_key)?;
    let token_out = config.addr(actors.stable_token_key)?;
    let pool = config.addr(actors.pool_3000_key.expect("Missing pool_3000_key"))?;
    let quoter = config.addr(actors.custom_quoter_key.expect("Missing custom_quoter_key"))?;

    // 3️⃣ Chuẩn bị volume để benchmark
    let volumes = volumes(U256::ZERO, ONE_ETHER.div(U256::from(10)), 100);

    // 4️⃣ Load bytecode thật và giả vào REVM
    init_account(from, &mut cache_db, provider.clone()).await?;
    init_account(pool, &mut cache_db, provider.clone()).await?;

    let mocked_erc20 = include_str!("../bytecode/generic_erc20.hex");
    let mocked_erc20 = Bytecode::new_raw(Bytes::from_str(mocked_erc20)?);
    init_account_with_bytecode(token_in, mocked_erc20.clone(), &mut cache_db)?;
    init_account_with_bytecode(token_out, mocked_erc20.clone(), &mut cache_db)?;

    // 5️⃣ Insert fake balances vào REVM storage
    let mocked_balance = U256::MAX / U256::from(2);
    insert_mapping_storage_slot(token_in, U256::ZERO, pool, mocked_balance, &mut cache_db)?;
    insert_mapping_storage_slot(token_out, U256::ZERO, pool, mocked_balance, &mut cache_db)?;

    // 6️⃣ Load custom quoter bytecode
    let mocked_custom_quoter = include_str!("../bytecode/uni_v3_quoter.hex");
    let mocked_custom_quoter = Bytecode::new_raw(Bytes::from_str(mocked_custom_quoter)?);
    init_account_with_bytecode(quoter, mocked_custom_quoter, &mut cache_db)?;

    // 7️⃣ Quote đầu tiên
    let start = measure_start("chain_revm_quoter_first");
    let calldata = get_amount_out_calldata(pool, token_in, token_out, volumes[0]);
    let response = revm_revert(from, quoter, calldata, &mut cache_db)?;
    let amount_out = decode_get_amount_out_response(response)?;
    println!("{} {} -> {} {}", volumes[0], actors.native_token_key, actors.stable_token_key, amount_out);
    measure_end(start);

    // 8️⃣ Loop benchmark các volume còn lại
    let start = measure_start("chain_revm_quoter_loop");
    for (index, volume) in volumes.into_iter().enumerate() {
        let calldata = get_amount_out_calldata(pool, token_in, token_out, volume);
        let response = revm_revert(from, quoter, calldata, &mut cache_db)?;
        let amount_out = decode_get_amount_out_response(response)?;
        if index % 20 == 0 {
            println!("{} {} -> {} {}", volume, actors.native_token_key, actors.stable_token_key, amount_out);
        }
    }
    measure_end(start);

    Ok(())
}
