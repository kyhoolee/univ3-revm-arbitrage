use alloy::{
    network::{Ethereum, TransactionBuilder},
    primitives::{Address, Bytes, U256},
    providers::{Provider, RootProvider},
    rpc::types::TransactionRequest,
    sol_types::SolValue,
    transports::http::{Client, Http},
    uint,
};



use anyhow::{anyhow, Result};
use revm::primitives::{keccak256, AccountInfo, Bytecode};
use revm::{
    db::{AlloyDB, CacheDB},
    primitives::{ExecutionResult, Output, TransactTo},
    Evm,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;


// use web3::types::H160;
// use web3::helpers::to_checksum;

// /// Converts a lowercase Ethereum address to a checksummed address (EIP-55).
// pub fn to_checksum_address(address: &str) -> Result<String, String> {
//     // Parse the input string into an H160 Ethereum address
//     match address.parse::<H160>() {
//         Ok(h160_address) => Ok(to_checksum(&h160_address, None)),
//         Err(_) => Err("Invalid Ethereum address format".to_string()),
//     }
// }

pub fn measure_start(label: &str) -> (String, Instant) {
    (label.to_string(), Instant::now())
}

pub fn measure_end(start: (String, Instant)) -> Duration {
    let elapsed = start.1.elapsed();
    println!("Elapsed: {:.2?} for '{}'", elapsed, start.0);
    elapsed
}

pub fn volumes(from: U256, to: U256, count: usize) -> Vec<U256> {
    let start = U256::ZERO;
    let mut volumes = Vec::new();
    let distance = to - from;
    let step = distance / U256::from(count);

    for i in 1..(count + 1) {
        let current = start + step * U256::from(i);
        volumes.push(current);
    }

    volumes.reverse();
    volumes
}

pub fn build_tx(to: Address, from: Address, calldata: Bytes, base_fee: u128) -> TransactionRequest {
    TransactionRequest::default()
        .to(to)
        .from(from)
        .with_input(calldata)
        .nonce(0)
        .gas_limit(1000000)
        .max_fee_per_gas(base_fee * 12 / 10)
        .max_priority_fee_per_gas(base_fee / 10)
        .build_unsigned()
        .unwrap()
        .into()
}

pub fn build_tx_avalanche(
    to: Address, 
    from: Address, 
    calldata: Bytes, 
    base_fee: u128, 
    chain_id: Option<u64> // Use Option for default handling
) -> TransactionRequest {
    // Use chain_id or fallback to Avalanche's default chain ID
    let chain_id = chain_id.unwrap_or(43114); // Avalanche C-Chain ID
    
    TransactionRequest::default()
        .to(to)
        .from(from)
        .with_input(calldata)
        .nonce(0)
        .gas_limit(1000000)
        .max_fee_per_gas(10*base_fee)
        .max_priority_fee_per_gas(0)
        .with_chain_id(chain_id) // Use Avalanche's chain ID (43114)
        .build_unsigned()
        .unwrap()
        .into()
}

pub fn build_tx_ronin(
    to: Address, 
    from: Address, 
    calldata: Bytes, 
    base_fee: u128, 
    chain_id: Option<u64> // Use Option for default handling
) -> TransactionRequest {
    // Use chain_id or fallback to Avalanche's default chain ID
    // let chain_id = chain_id.unwrap_or(2020); // Ronin
    
    TransactionRequest::default()
        .to(to)
        .from(from)
        .with_input(calldata)
        .nonce(0)
        .gas_limit(1_000_000)
        .max_fee_per_gas(1*base_fee)
        .max_priority_fee_per_gas(0)
        // .with_chain_id(chain_id) // Ronin
        .build_unsigned()
        .unwrap()
        .into()
}

// pub type AlloyCacheDB = CacheDB<AlloyDB<Http<Client>, Ethereum, Arc<RootProvider<Http<Client>>>>>;

// pub fn revm_call(
//     from: Address,
//     to: Address,
//     calldata: Bytes,
//     cache_db: &mut AlloyCacheDB,
// ) -> Result<Bytes> {
//     let mut evm = Evm::builder()
//         .with_db(cache_db)
//         .modify_tx_env(|tx| {
//             tx.caller = from;
//             tx.transact_to = TransactTo::Call(to);
//             tx.data = calldata;
//             tx.value = U256::ZERO;
//         })
//         .build();

//     let ref_tx = evm.transact().unwrap();
//     let result = ref_tx.result;

//     let value = match result {
//         ExecutionResult::Success {
//             output: Output::Call(value),
//             ..
//         } => value,
//         result => {
//             return Err(anyhow!("execution failed: {result:?}"));
//         }
//     };

//     Ok(value)
// }

// pub fn revm_revert(
//     from: Address,
//     to: Address,
//     calldata: Bytes,
//     cache_db: &mut AlloyCacheDB,
// ) -> Result<Bytes> {
//     let mut evm = Evm::builder()
//         .with_db(cache_db)
//         .modify_tx_env(|tx| {
//             tx.caller = from;
//             tx.transact_to = TransactTo::Call(to);
//             tx.data = calldata;
//             tx.value = U256::ZERO;
//         })
//         .build();
//     let ref_tx = evm.transact().unwrap();
//     let result = ref_tx.result;

//     let value = match result {
//         ExecutionResult::Revert { output: value, .. } => value,
//         _ => {
//             panic!("It should never happen!");
//         }
//     };

//     Ok(value)
// }

// pub fn init_cache_db(provider: Arc<RootProvider<Http<Client>>>) -> AlloyCacheDB {
//     CacheDB::new(AlloyDB::new(provider, Default::default()).unwrap())
// }

// pub async fn init_account(
//     address: Address,
//     cache_db: &mut AlloyCacheDB,
//     provider: Arc<RootProvider<Http<Client>>>,
// ) -> Result<()> {
//     let cache_key = format!("bytecode-{:?}", address);
//     let bytecode = match cacache::read(&cache_dir(), cache_key.clone()).await {
//         Ok(bytecode) => {
//             let bytecode = Bytes::from(bytecode);
//             Bytecode::new_raw(bytecode)
//         }
//         Err(_e) => {
//             let bytecode = provider.get_code_at(address).await?;
//             let bytecode_result = Bytecode::new_raw(bytecode.clone());
//             let bytecode = bytecode.to_vec();
//             cacache::write(&cache_dir(), cache_key, bytecode.clone()).await?;
//             bytecode_result
//         }
//     };
//     let code_hash = bytecode.hash_slow();
//     let acc_info = AccountInfo {
//         balance: U256::ZERO,
//         nonce: 0_u64,
//         code: Some(bytecode),
//         code_hash,
//     };
//     cache_db.insert_account_info(address, acc_info);
//     Ok(())
// }

// pub fn init_account_with_bytecode(
//     address: Address,
//     bytecode: Bytecode,
//     cache_db: &mut AlloyCacheDB,
// ) -> Result<()> {
//     let code_hash = bytecode.hash_slow();
//     let acc_info = AccountInfo {
//         balance: U256::ZERO,
//         nonce: 0_u64,
//         code: Some(bytecode),
//         code_hash,
//     };

//     cache_db.insert_account_info(address, acc_info);
//     Ok(())
// }

// pub fn insert_mapping_storage_slot(
//     contract: Address,
//     slot: U256,
//     slot_address: Address,
//     value: U256,
//     cache_db: &mut AlloyCacheDB,
// ) -> Result<()> {
//     let hashed_balance_slot = keccak256((slot_address, slot).abi_encode());

//     cache_db.insert_account_storage(contract, hashed_balance_slot.into(), value)?;
//     Ok(())
// }

// fn cache_dir() -> String {
//     ".evm_cache".to_string()
// }
