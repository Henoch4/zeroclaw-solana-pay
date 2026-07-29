#!/usr/bin/env bash
# Build script for solana-wallet plugin
# Run from the project root: ./scripts/build.sh
set -euo pipefail

echo "=== Solana Wallet Plugin Build ==="

# 1. Check wasm32-wasip2 target
if ! rustup target list --installed | grep -q "wasm32-wasip2"; then
    echo "Adding wasm32-wasip2 target..."
    rustup target add wasm32-wasip2
fi

# 2. Run host tests
echo -e "\n=== Running host tests ==="
cd plugins/solana-wallet
cargo test
echo "All tests passed!"

# 3. Build WASM component
echo -e "\n=== Building WASM component ==="
cargo build --target wasm32-wasip2 --release
echo "WASM build successful!"

# 4. Show output
WASM_PATH="target/wasm32-wasip2/release/solana_wallet.wasm"
if [ -f "$WASM_PATH" ]; then
    SIZE=$(stat -f%z "$WASM_PATH" 2>/dev/null || stat -c%s "$WASM_PATH" 2>/dev/null)
    echo -e "\n=== Output ==="
    echo "Plugin: $WASM_PATH"
    echo "Size: $((SIZE / 1024)) KB"
fi

cd ../..
echo -e "\n=== Build complete ==="
