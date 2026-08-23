#!/usr/bin/env bash
# Builds, checks, and prepares DWCS for registration on Telegraph.
# Does NOT submit the on-chain transaction itself, that's a deliberate
# manual step (registerWasm via integrate.telegraphprotocol.com or a
# direct contract call), registration costs a real transaction and should
# never happen from an unattended script.

set -euo pipefail

cd "$(dirname "$0")/../dwcs/rust-module"

echo "Running host-side unit tests first..."
cargo test

echo ""
echo "Building release WASM binary..."
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown

WASM_FILE="target/wasm32-unknown-unknown/release/dwcs_scoring_module.wasm"

if [ ! -f "$WASM_FILE" ]; then
  echo "Build did not produce $WASM_FILE, check cargo output above."
  exit 1
fi

echo ""
echo "Checking for zero imports (required, a non-zero count means this is not a valid Telegraph scoring module)..."
if ! command -v wasm-tools &> /dev/null; then
  echo "wasm-tools not found. Install it (cargo install wasm-tools) before proceeding."
  exit 1
fi

IMPORT_COUNT=$(wasm-tools print "$WASM_FILE" | grep -c '(import' || true)
echo "Import count: $IMPORT_COUNT"
if [ "$IMPORT_COUNT" -ne 0 ]; then
  echo "FAILED: expected 0 imports, got $IMPORT_COUNT. Do not register this binary."
  echo "Check you built wasm32-unknown-unknown, not wasm32-wasip1."
  exit 1
fi

echo ""
echo "Build OK. File: $(pwd)/$WASM_FILE"
echo ""
echo "Next steps (manual, not automated by this script):"
echo "  1. Test against Telegraph's go-tester harness, see rust-module/README.md."
echo "  2. Host this file at a public https:// or ipfs:// URL."
echo "  3. Register via https://integrate.telegraphprotocol.com, or call"
echo "     registerWasm(wasmHash, wasmUrl, intent) directly."
