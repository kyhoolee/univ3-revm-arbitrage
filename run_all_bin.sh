#!/bin/bash
set -e  # Stop if any command fails
source .env

echo "🚀 Running all bin/*.rs targets in release mode"

for BIN in $(ls src/bin | sed 's/\.rs$//'); do
    echo "👉 Running $BIN ..."
    cargo run --bin "$BIN" --release
    echo "✅ Finished $BIN"
    echo "-----------------------------"
done

echo "🎉 All binaries ran successfully!"
