use alloy::{
    primitives::U256,
    providers::{Provider, ProviderBuilder}, transports::http::reqwest::Url,
};
// use source::build_tx_ronin;
use std::sync::Arc;
// pub mod source;
use anyhow::Result;
use std::ops::Div;
use alloy::eips::BlockId;
use alloy::primitives::{address, Address};

use univ3_revm_arbitrage::source::*;
use univ3_revm_arbitrage::chain::ronin::*;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    // let rpc = std::env::var("ETH_RPC_URL").unwrap().parse()?;
    // "";
    // ETH_RPC_URL = 'https://ronin.gateway.tenderly.co/7dNLdBPOaPCaNqVx5pZ10s'
    // QUOTER_ADDRESS = '0x84ab2f9fdc4bf66312b0819d879437b8749efdf2'
    // let rpc = "https://eth.merkle.io";
    // let rpc = "https://api.avax.network/ext/bc/C/rpc";
    // let rpc = "http://10.0.1.50:8745";
    let rpc = "https://ronin.gateway.tenderly.co/7dNLdBPOaPCaNqVx5pZ10s";

    // let provider = ProviderBuilder::new().on_http(std::env::var("ETH_RPC_URL").unwrap().parse()?);
    let provider = ProviderBuilder::new().on_http(Url::parse(rpc)?);
    let provider = Arc::new(provider);

    let base_fee = provider.get_gas_price().await?;

    let volume = ONE_ETHER.div(U256::from(100));

    // # "tokenIn": "0xc99a6A985eD2Cac1ef41640596C5A5f9F4E19Ef5",  # Ronin WETH
    // # "tokenOut": "0xe514d9DEB7966c8BE0ca922de8a064264eA6bcd4",  # Wrapped Ronin

    let token_in: Address = address!("c99a6A985eD2Cac1ef41640596C5A5f9F4E19Ef5"); // WETH
    let token_out: Address = address!("e514d9DEB7966c8BE0ca922de8a064264eA6bcd4"); // WRON

    // let token_in: Address = address!("0b7007c13325c48911f73a2dad5fa5dcbf808adc"); // USDC
    // let token_out: Address = address!("97a9107c1793bc407d6f527b77e7fff4d812bece"); // AXS


    let quoter_address: Address = address!("84ab2f9fdc4bf66312b0819d879437b8749efdf2"); // Quoter

    // let calldata = quote_calldata(WETH_ADDR, USDC_ADDR, volume, 3000);
    // let calldata = quote_calldata(token_in, token_out, volume, 3000);
    println!("volume: {}", volume);
    let calldata = quote_exact_input_single_calldata(token_in, token_out, volume, 3000);
    println!("calldata: {:?}", calldata);


    println!("\n---------\nBlock: {}", provider.get_block_number().await?);
    // Block: 42488999
    println!("base_fee: {}", base_fee);

    // let tx = build_tx(quoterAddress, ME, calldata, base_fee);
    let chain_id = 2021;
    let wallet = address!("ebe94ab981899efbc8bb6cb2b045fc7490726ff3");
    let tx = build_tx_ronin(quoter_address, ME, calldata, base_fee, Some(chain_id));
    let start = measure_start("eth_call_one");
    println!("tx: {:?}", tx);
    let call = provider.call(&tx).block(BlockId::latest());
    println!("call: {:?}", call);
    let call = call.await?;
    println!("call: {:?}", call);   
    let amount_out = decode_quote_response(call)?;
    println!("{} WETH -> WRON {}", volume, amount_out);
    // println!("{} WETH -> USDC {}", volume, amount_out);

    measure_end(start);

    Ok(())
}
