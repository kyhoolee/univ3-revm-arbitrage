use alloy::{
    primitives::{aliases::U24, Address, Bytes, U160, U256},
    sol,
    sol_types::{SolCall, SolValue},
};

use anyhow::Result;

sol! {
    struct QuoteExactInputSingleParams {
        address tokenIn;
        address tokenOut;
        uint256 amountIn;
        uint24 fee;
        uint160 sqrtPriceLimitX96;
    }

    function quoteExactInputSingle(QuoteExactInputSingleParams memory params)
    public
    override
    returns (
        uint256 amountOut,
        uint160 sqrtPriceX96After,
        uint32 initializedTicksCrossed,
        uint256 gasEstimate
    );

}

sol! {
    function getAmountOut(
        address pool,
        bool zeroForOne,
        uint256 amountIn
    ) external;
}

pub fn decode_quote_response(response: Bytes) -> Result<u128> {
    let (amount_out, _, _, _) = <(u128, u128, u32, u128)>::abi_decode(&response, false)?;
    Ok(amount_out)
}

pub fn decode_get_amount_out_response(response: Bytes) -> Result<u128> {
    let value = response.to_vec();
    let last_64_bytes = &value[value.len() - 64..];

    let (a, b) = match <(i128, i128)>::abi_decode(last_64_bytes, false) {
        Ok((a, b)) => (a, b),
        Err(e) => return Err(anyhow::anyhow!("'getAmountOut' decode failed: {:?}", e)),
    };
    let value_out = std::cmp::min(a, b);
    let value_out = -value_out;
    Ok(value_out as u128)
}

pub fn get_amount_out_calldata(
    pool: Address,
    token_in: Address,
    token_out: Address,
    amount_in: U256,
) -> Bytes {
    Bytes::from(
        getAmountOutCall {
            pool,
            zeroForOne: token_in < token_out,
            amountIn: amount_in,
        }
        .abi_encode(),
    )
}

pub fn quote_calldata(token_in: Address, token_out: Address, amount_in: U256, fee: u32) -> Bytes {
    let zero_for_one = token_in < token_out;

    let sqrt_price_limit_x96: U160 = if zero_for_one {
        "4295128749".parse().unwrap()
    } else {
        "1461446703485210103287273052203988822378723970341"
            .parse()
            .unwrap()
    };

    let params = QuoteExactInputSingleParams {
        tokenIn: token_in,
        tokenOut: token_out,
        amountIn: amount_in,
        fee: U24::from(fee),
        sqrtPriceLimitX96: sqrt_price_limit_x96,
    };

    Bytes::from(quoteExactInputSingleCall { params }.abi_encode())
}




sol! {
    function quoteExactInput(
        bytes path,
        uint256 amountIn
    )
    public
    returns (
        uint256 amountOut,
        uint160[] memory sqrtPriceX96AfterList,
        uint32[] memory initializedTicksCrossedList,
        uint256 gasEstimate
    );
}

pub fn encode_path(tokens: &[Address], fees: &[U24]) -> Bytes {
    assert!(tokens.len() == fees.len() + 1, "Path length mismatch between tokens and fees");

    let mut path = Vec::new();
    for i in 0..fees.len() {
        path.extend_from_slice(tokens[i].as_ref());
        let fee_bytes: [u8; 3] = fees[i].to_be_bytes::<3>(); // Explicitly specifying size
        path.extend_from_slice(&fee_bytes);
    }
    path.extend_from_slice(tokens.last().unwrap().as_ref());
    Bytes::from(path)
}


use std::fmt;

pub struct PrettyBytes(pub Bytes);

impl fmt::Display for PrettyBytes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for byte in self.0.iter() {
            write!(f, "{:02x} ", byte)?;
        }
        Ok(())
    }
}

pub fn quote_exact_input_calldata(tokens: &[Address], fees: &[U24], amount_in: U256) -> Bytes {
    let path = encode_path(tokens, fees);
    Bytes::from(quoteExactInputCall {
        path,
        amountIn: amount_in, }.abi_encode())
}

pub fn quote_exact_input_single_calldata(
    token_in: Address,
    token_out: Address,
    amount_in: U256,
    fee: u32,
) -> Bytes {
    let path = encode_path(&[token_in, token_out], &[U24::from(fee)]);
    let pretty = PrettyBytes(path.clone());
    println!("{}", pretty);
    let encoded = quoteExactInputCall {
        path,
        amountIn: amount_in }.abi_encode();
    println!("encoded: {:?}", encoded.clone());
    Bytes::from(encoded)
}
