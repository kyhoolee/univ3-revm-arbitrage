use alloy::{
    network::{TransactionBuilder},
    primitives::{Address, Bytes, U256},
    rpc::types::TransactionRequest,
};
use std::time::Duration;
use tokio::time::Instant;

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
        .max_priority_fee_per_gas(base_fee * 2)
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
