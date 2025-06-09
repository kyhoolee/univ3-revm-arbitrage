#!/bin/bash
set -e

mkdir -p doc

echo "Running simulate --method call"
cargo run --bin simulate -- --method call > doc/sim_call.log

echo "Running simulate --method revm"
cargo run --bin simulate -- --method revm > doc/sim_revm.log

echo "Running simulate --method anvil"
cargo run --bin simulate -- --method anvil > doc/sim_anvil.log

echo "Running simulate --method revm_cached"
cargo run --bin simulate -- --method revm_cached > doc/sim_cached.log

echo "Running simulate --method revm_quoter"
cargo run --bin simulate -- --method revm_quoter > doc/sim_quoter.log

echo "Running simulate --method validate"
cargo run --bin simulate -- --method validate > doc/sim_validate.log

echo "All simulations done. Logs in doc/"
