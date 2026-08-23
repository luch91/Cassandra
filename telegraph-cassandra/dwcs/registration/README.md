# DWCS registration handoff

The DWCS scoring module is ready for owner-controlled registration. This handoff deliberately excludes wallet credentials and does not submit a transaction.

## Validated binary

Build command:

```text
cargo build --release --target wasm32-unknown-unknown
```

Binary path:

```text
dwcs/rust-module/target/wasm32-unknown-unknown/release/dwcs_scoring_module.wasm
```

SHA-256:

```text
B9FDA517680949B74A81E43839D42452F83E15A8B7150C6411C55AD4DC7F2A53
```

Validation completed on Aug 23:

- Rust unit tests: 9 passed.
- Target: `wasm32-unknown-unknown`.
- `wasm-tools print` import count: 0.
- Required exports: `alloc`, `dealloc`, `rank_answer`.
- Telegraph's official `go-tester` passed exact, wrong, empty, reworded, quality-ranked, Unicode, and long-input cases.

## Owner actions

1. Rebuild the binary and verify its SHA-256 matches the value above.
2. Host that exact binary at a public HTTPS URL or IPFS CID.
3. Connect the wallet that will own the registration, funded with Base Sepolia test ETH for gas.
4. Register `FRAUD_DETECTION` through `https://integrate.telegraphprotocol.com`.
5. Record the returned registration ID and final status in the local progress log.

Do not commit private keys, wallet files, or any canary dataset to this repository.
