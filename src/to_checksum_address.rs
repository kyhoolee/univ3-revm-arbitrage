
use anyhow::Result;
use alloy::primitives::{address, Address};

#[tokio::main]
async fn main() -> Result<()> {
    let token_in: Address = address!("c99a6A985eD2Cac1ef41640596C5A5f9F4E19Ef5"); // WETH
    let token_out: Address = address!("e514d9DEB7966c8BE0ca922de8a064264eA6bcd4"); // WRON
    let quoter_address: Address = address!("84ab2f9fdc4bf66312b0819d879437b8749efdf2"); // Quoter

    println!("token_in: {}", token_in);
    println!("token_out: {}", token_out);
    println!("quoter_address: {}", quoter_address);


    Ok(())
}
