#!/bin/bash

mkdir -p src/bin
mkdir -p src/core
mkdir -p src/config
mkdir -p src/source
mkdir -p src/chain
mkdir -p src/bytecode

# Move CLI entrypoint
mv src/bin/eth_call_one.rs src/bin/simulate.rs

# Core logic
mv src/bin/eth_call.rs          src/core/call.rs
mv src/bin/eth_revm.rs          src/core/revm.rs
mv src/bin/eth_revm_cached.rs   src/core/revm_cached.rs
mv src/bin/eth_revm_quoter.rs   src/core/revm_quoter.rs
mv src/bin/eth_validate.rs      src/core/validate.rs
mv src/bin/eth_arbitrage.rs     src/core/arbitrage.rs
mv src/bin/eth_anvil.rs         src/core/anvil.rs

# Move AVAX logic if needed
mv src/bin/avax_call.rs         src/core/avax_call.rs
mv src/bin/ronin_call.rs        src/core/ronin_call.rs

# Chain config
mv src/chain/*.rs               src/chain/
mv src/chain/mod.rs             src/chain/mod.rs

# Source ABI + helpers
mv src/source/abi.rs            src/source/abi.rs
mv src/source/helpers.rs        src/source/builder.rs

# Bytecode
mv src/bytecode/generic_erc20.hex src/bytecode/
mv src/bytecode/uni_v3_quoter.hex src/bytecode/

# Contracts (optional move to test folder or leave)
mkdir -p src/contracts
mv src/contracts/uni_v3_quoter.sol src/contracts/

# Update lib
echo "pub mod core;" >> src/lib.rs
echo "pub mod config;" >> src/lib.rs
