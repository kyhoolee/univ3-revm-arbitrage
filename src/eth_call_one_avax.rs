use alloy::{
    primitives::U256,
    providers::{Provider, ProviderBuilder}, transports::http::reqwest::Url,
};
use source::build_tx_avalanche;
use std::sync::Arc;
pub mod source;
use anyhow::Result;
use std::ops::Div;
use alloy::eips::BlockId;
use alloy::primitives::{address, Address};

use crate::source::{
    build_tx, decode_quote_response, measure_end, measure_start, quote_calldata, ME, ONE_ETHER,
    USDC_ADDR, V3_QUOTER_ADDR, WETH_ADDR,
};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    // let rpc = std::env::var("ETH_RPC_URL").unwrap().parse()?;
    // "";
    // let rpc = "https://eth.merkle.io";
    let rpc = "https://api.avax.network/ext/bc/C/rpc";

    // let provider = ProviderBuilder::new().on_http(std::env::var("ETH_RPC_URL").unwrap().parse()?);
    let provider = ProviderBuilder::new().on_http(Url::parse(rpc)?);
    let provider = Arc::new(provider);

    let base_fee = provider.get_gas_price().await?;

    let volume = ONE_ETHER.div(U256::from(10));

    let token_in: Address = address!("B31f66AA3C1e785363F0875A1B74E27b85FD66c7"); // WAVAX on Avalanche
    let token_out: Address = address!("B97EF9Ef8734C71904D8002F8b6Bc66Dd9c48a6E"); // USDC on Avalanche
    let quoter_address: Address = address!("be0F5544EC67e9B3b2D979aaA43f18Fd87E6257F"); // Quoter on Avalanche
    // let calldata = quote_calldata(WETH_ADDR, USDC_ADDR, volume, 3000);
    let calldata = quote_calldata(token_in, token_out, volume, 3000);

    // let tx = build_tx(quoterAddress, ME, calldata, base_fee);
    let chain_id = 43114;
    let tx = build_tx_avalanche(quoter_address, ME, calldata, base_fee, Some(chain_id));
    let start = measure_start("eth_call_one");
    println!("\ntx: {:?}", tx);
    let call = provider.call(&tx).block(BlockId::latest());
    println!("\ncall: {:?}", call);
    let call = call.await?;
    println!("\ncall: {:?}", call);
    let amount_out = decode_quote_response(call)?;
    println!("{} WAVAX -> USDC {}", volume, amount_out);
    // println!("{} WETH -> USDC {}", volume, amount_out);

    measure_end(start);

    Ok(())
}
