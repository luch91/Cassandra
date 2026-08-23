#!/usr/bin/env bash
# Runs every local test: the real Rust module's own tests, the TS
# prototype's tests, and the application's tests. No live Telegraph calls,
# no on-chain interaction, safe to run any time.

set -euo pipefail

echo "=== Rust module tests (the real, deployable scoring logic) ==="
(cd "$(dirname "$0")/../dwcs/rust-module" && cargo test)

echo ""
echo "=== TS prototype tests (mirrors the Rust logic, for fast iteration) ==="
npm run test:dwcs

echo ""
echo "=== Application (Sentinel) tests ==="
npm run test:app

echo ""
echo "All local tests passed. This does not validate live behavior against"
echo "Telegraph's actual Stage 2 benchmark or real x402 payments, those"
echo "require the go-tester harness and a funded testnet wallet respectively."
