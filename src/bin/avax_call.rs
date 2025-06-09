use std::sync::Arc;
use std::ops::Div;

use anyhow::Result;
use alloy::{
    eips::BlockId,
    primitives::U256,
    providers::{Provider, ProviderBuilder},
    transports::http::reqwest::Url,
};

use univ3_revm_arbitrage::{chain::avax::*, types::ONE_ETHER};
use univ3_revm_arbitrage::source::*;


#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let rpc = "https://api.avax.network/ext/bc/C/rpc";
    let provider = ProviderBuilder::new().on_http(Url::parse(rpc)?);
    let provider = Arc::new(provider);

    let base_fee = provider.get_gas_price().await?;

    let volume = ONE_ETHER.div(U256::from(10));
    let calldata = quote_calldata(WAVAX_ADDR, USDC_ADDR, volume, 3000);
    let tx = build_tx_avalanche(V3_QUOTER_ADDR, ME, calldata, base_fee, Some(CHAIN_ID));

    let start = measure_start("avax_call");
    println!("\ntx: {:?}", tx);

    let call = provider.call(&tx).block(BlockId::latest());
    println!("\ncall: {:?}", call);
    let call = call.await?;
    println!("\nresponse: {:?}", call);

    let amount_out = decode_quote_response(call)?;
    println!("{} WAVAX -> USDC {}", volume, amount_out);

    measure_end(start);
    Ok(())
}
