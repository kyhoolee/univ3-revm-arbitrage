#!/bin/bash
set -e

mkdir -p doc

# File log tổng
ALL_LOG="doc/sim_all.log"
echo "" > "$ALL_LOG"  # Clear file cũ nếu có

# Cấu hình chain và method
CHAINS=("eth" "avax")
METHODS=("call" "revm" "anvil" "revm_cached" "revm_quoter" "validate")

for CHAIN in "${CHAINS[@]}"; do
    echo "=== CHAIN = $CHAIN ===" | tee -a "$ALL_LOG"
    for METHOD in "${METHODS[@]}"; do
        echo "=== METHOD = $METHOD ===" | tee -a "$ALL_LOG"
        RUSTFLAGS="-Awarnings" cargo run --bin simulate -- --method "$METHOD" --chain "$CHAIN" 2>&1 | tee -a "$ALL_LOG"
        echo "" | tee -a "$ALL_LOG"
    done
done

echo "All simulations done. Full log in $ALL_LOG"
