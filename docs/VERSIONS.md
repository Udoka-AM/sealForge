# Pinned versions

Filled in during Milestone 1, task 1.2 (PRD open question Q1). Each row records the pin, where it is set, the reason, and when it was last verified.

"Verified" means: against NU7 testnet or the local NU6.3 regtest chain on the date given. "Tests" means the workspace suites (`cargo test --workspace`, `pnpm test`) passed with this pin on the date given, which re-checks the build, not the network behaviour.

| Component | Version | Set in | Verified on | Tests pass on | Notes |
|---|---|---|---|---|---|
| Rust toolchain | 1.99.0 | `rust-toolchain.toml` | 2026-10-15 | 2026-11-13 | Bumped from 1.89 for sqlx 0.9 (needs ≥ 1.94) |
| Node | 22 LTS | `.nvmrc` | 2026-10-08 | 2026-11-13 (local run on Node 24) | |
| Next.js | 16.4.2 | `apps/web/package.json` | 2026-10-08 (as 16.4.0) | 2026-11-13 | |
| Zebra (`zebrad`) | 7.0.0-rc.0 (`zfnd/zebra`) | `scripts/regtest.sh` | 2026-10-15 | n/a | NU7 on testnet. Local regtest chain: `scripts/regtest.sh` |
| Zaino | 0.10.2 (`zingodevops/zaino:0.10.2-no-tls`, amd64) | `scripts/regtest.sh` | 2026-10-15 | n/a | Knows Ironwood (NU6.3) but not NU7's branch ID yet, so regtest activates up to NU6.3 |
| `zcash_client_backend` / `zcash_client_sqlite` | =0.25.0-pre.2 / =0.23.0-pre.2 | `Cargo.toml` | 2026-10-17 (testnet auctions) | 2026-11-13 | Escrow wallets (`sealforge-chain`). Scanning Ironwood notes on regtest and testnet |
| `zcash_keys` / `zcash_protocol` | =0.17.0-pre.2 / =0.11.0-pre.1 | `Cargo.toml` | 2026-10-14 | 2026-11-13 | `zcash_keys` with `orchard` + `unstable-frost`. `zcash_protocol` knows NU7 |
| `zcash_primitives` / `zcash_address` | =0.31.0-pre.2 / =0.14.0-pre.2 | `crates/sealforge-wallet/Cargo.toml` | 2026-10-17 | 2026-11-13 | In-browser wallet (librustzcash compiled to WASM) |
| `pczt` | =0.10.0-pre.2 | `Cargo.toml` | 2026-10-17 (testnet refunds and payouts) | 2026-11-13 | Has `shielded_sighash` and `apply_ironwood_signature` |
| `orchard` | 0.16.0 (lockfile) | `Cargo.toml` (`0.16`) | 2026-10-17 | 2026-11-13 | Ironwood value pool, `PostNu6_3` circuit |
| `zcash_note_encryption` | 0.5.1 (lockfile) | `crates/sealforge-wallet/Cargo.toml`, `sealforge-scan` | 2026-10-17 | 2026-11-13 | Compact and full note decryption |
| FROST | `reddsa =0.5.2` (`frost` feature), `frost-rerandomized =3.0.0` | `Cargo.toml` | 2026-10-14 | 2026-11-13 | The FROST(Pallas, BLAKE2b-512) ciphersuite isn't in reddsa 0.6. See docs/spikes/frost-pczt.md |
| `reddsa` | 0.6.1 | `Cargo.toml` | 2026-10-14 | 2026-11-13 | Orchard's RedPallas; used to check FROST output verifies as a consensus signature |

The browser wallet is `sealforge-wallet`, librustzcash compiled to WebAssembly. Earlier plans named WebZjs and `zcash-devtool`; neither is used (see [01-PRD.md](01-PRD.md) Q1 and [PRODUCT.md](PRODUCT.md) for that design history).
