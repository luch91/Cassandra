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

## Action requested from wallet owner

Use the owner-controlled, funded Base Sepolia wallet to register DWCS for `FRAUD_DETECTION`. Do not wait for this PR to merge. After the transaction finalizes, comment on the PR with the public binary URL or IPFS CID, SHA-256, transaction hash, registration ID, and initial registration status. Do not share private keys.

Complete and confirm each item below:

- [ ] Rebuilt the release binary from the reviewed commit.
- [ ] Recomputed and verified the SHA-256 value.
- [ ] Repeated the zero-import check with `wasm-tools print`.
- [ ] Repeated the official `go-tester` checks against that binary.
- [ ] Published the exact binary at a public HTTPS URL or IPFS CID.
- [ ] Submitted the `FRAUD_DETECTION` registration with the owner-controlled wallet.
- [ ] Recorded the registration ID, transaction hash, and initial status in the local progress log.

The registration ID, transaction hash, and public binary URL may be added after the transaction is finalized. Do not add any secret material.
