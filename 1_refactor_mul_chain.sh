mkdir -p src/bin

mv src/eth_call.rs src/bin/eth_call.rs
mv src/eth_call_one.rs src/bin/eth_call_one.rs
mv src/eth_call_one_avax.rs src/bin/avax_call.rs
mv src/eth_call_one_ronin.rs src/bin/ronin_call.rs
mv src/anvil.rs src/bin/eth_anvil.rs
mv src/revm.rs src/bin/eth_revm.rs
mv src/revm_cached.rs src/bin/eth_revm_cached.rs
mv src/revm_quoter.rs src/bin/eth_revm_quoter.rs
mv src/revm_validate.rs src/bin/eth_validate.rs
mv src/revm_arbitrage.rs src/bin/eth_arbitrage.rs
mv src/to_checksum_address.rs src/bin/sample_checksum.rs
