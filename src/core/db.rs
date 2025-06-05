use alloy::{
    network::Ethereum,
    primitives::{keccak256, Address, Bytes, U256},
    providers::{Provider, RootProvider},
    sol_types::SolValue,
    transports::http::{Client, Http},
};
use anyhow::{anyhow, Result};
use revm::{
    db::{AlloyDB, CacheDB},
    primitives::{AccountInfo, Bytecode, ExecutionResult, Output, TransactTo},
    Evm,
};
use std::sync::Arc;

pub type AlloyCacheDB =
    CacheDB<AlloyDB<Http<Client>, Ethereum, Arc<RootProvider<Http<Client>>>>>;

pub fn init_cache_db(provider: Arc<RootProvider<Http<Client>>>) -> AlloyCacheDB {
    CacheDB::new(AlloyDB::new(provider, Default::default()).unwrap())
}

pub async fn init_account(
    address: Address,
    cache_db: &mut AlloyCacheDB,
    provider: Arc<RootProvider<Http<Client>>>,
) -> Result<()> {
    let cache_key = format!("bytecode-{:?}", address);
    let bytecode = match cacache::read(&cache_dir(), cache_key.clone()).await {
        Ok(bytecode) => {
            let bytecode = Bytes::from(bytecode);
            Bytecode::new_raw(bytecode)
        }
        Err(_) => {
            let bytecode = provider.get_code_at(address).await?;
            let bytecode_result = Bytecode::new_raw(bytecode.clone());
            let bytecode_vec = bytecode.to_vec();
            cacache::write(&cache_dir(), cache_key, bytecode_vec).await?;
            bytecode_result
        }
    };
    let code_hash = bytecode.hash_slow();
    let acc_info = AccountInfo {
        balance: U256::ZERO,
        nonce: 0_u64,
        code: Some(bytecode),
        code_hash,
    };
    cache_db.insert_account_info(address, acc_info);
    Ok(())
}

pub fn init_account_with_bytecode(
    address: Address,
    bytecode: Bytecode,
    cache_db: &mut AlloyCacheDB,
) -> Result<()> {
    let code_hash = bytecode.hash_slow();
    let acc_info = AccountInfo {
        balance: U256::ZERO,
        nonce: 0_u64,
        code: Some(bytecode),
        code_hash,
    };
    cache_db.insert_account_info(address, acc_info);
    Ok(())
}

pub fn insert_mapping_storage_slot(
    contract: Address,
    slot: U256,
    slot_address: Address,
    value: U256,
    cache_db: &mut AlloyCacheDB,
) -> Result<()> {
    let hashed_slot = keccak256((slot_address, slot).abi_encode());
    cache_db.insert_account_storage(contract, hashed_slot.into(), value)?;
    Ok(())
}

pub fn revm_call(
    from: Address,
    to: Address,
    calldata: Bytes,
    cache_db: &mut AlloyCacheDB,
) -> Result<Bytes> {
    let mut evm = Evm::builder()
        .with_db(cache_db)
        .modify_tx_env(|tx| {
            tx.caller = from;
            tx.transact_to = TransactTo::Call(to);
            tx.data = calldata;
            tx.value = U256::ZERO;
        })
        .build();

    let result = evm.transact()?.result;

    let value = match result {
        ExecutionResult::Success {
            output: Output::Call(value),
            ..
        } => value,
        result => {
            return Err(anyhow!("execution failed: {result:?}"));
        }
    };

    Ok(value)
}

pub fn revm_revert(
    from: Address,
    to: Address,
    calldata: Bytes,
    cache_db: &mut AlloyCacheDB,
) -> Result<Bytes> {
    let mut evm = Evm::builder()
        .with_db(cache_db)
        .modify_tx_env(|tx| {
            tx.caller = from;
            tx.transact_to = TransactTo::Call(to);
            tx.data = calldata;
            tx.value = U256::ZERO;
        })
        .build();

    let result = evm.transact()?.result;

    match result {
        ExecutionResult::Revert { output, .. } => Ok(output),
        _ => Err(anyhow!("Expected revert result")),
    }
}

fn cache_dir() -> String {
    ".evm_cache".to_string()
}
