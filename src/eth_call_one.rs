use alloy::{
    primitives::U256,
    providers::{Provider, ProviderBuilder},
    transports::http::reqwest::Url,
};
use std::sync::Arc;
pub mod source;
use anyhow::Result;
use std::ops::Div;
use alloy::eips::BlockId;
use crate::source::{
    build_tx, decode_quote_response, measure_end, measure_start, quote_calldata, ME, ONE_ETHER,
    USDC_ADDR, V3_QUOTER_ADDR, WETH_ADDR,
};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let rpc = "https://eth.merkle.io";

    let provider = ProviderBuilder::new().on_http(Url::parse(rpc)?);
    let provider = Arc::new(provider);

    let base_fee = provider.get_gas_price().await?;

    let volume = ONE_ETHER.div(U256::from(10));
    let calldata = quote_calldata(WETH_ADDR, USDC_ADDR, volume, 3000);

    let tx = build_tx(V3_QUOTER_ADDR, ME, calldata, base_fee);
    println!("tx: {:?}", tx);

    let start = measure_start("eth_call_one");
    let call = provider.call(&tx).block(BlockId::latest()).await?;
    println!("call: {:?}", call);   
    
    let amount_out = decode_quote_response(call)?;
    println!("{} WETH -> USDC {}", volume, amount_out);

    measure_end(start);

    Ok(())
}
