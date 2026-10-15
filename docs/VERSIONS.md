# Pinned versions

Filled in during Milestone 1, task 1.2 (PRD open question Q1). Record each pin with the reason and the date it was verified against NU7 testnet.

| Component | Version | Verified on | Notes |
|---|---|---|---|
| Rust toolchain | 1.99.0 | 2026-10-15 | `rust-toolchain.toml`. Bumped from 1.89 for sqlx 0.9 (needs ≥ 1.94) |
| Node | 22 LTS | 2026-10-08 | `.nvmrc` |
| Next.js | 16.4.0 | 2026-10-08 | `apps/web` |
| Zebra (`zebrad`) | 7.0.0-rc.0 (`zfnd/zebra`) | 2026-10-15 | NU7 on testnet. Local regtest chain: `scripts/regtest.sh` |
| Zaino | 0.10.2 (`zingodevops/zaino:0.10.2-no-tls`, amd64) | 2026-10-15 | Knows Ironwood (NU6.3) but not NU7's branch ID yet, so regtest activates up to NU6.3 |
| `zcash_client_backend` / `zcash_client_sqlite` | =0.25.0-pre.2 / =0.23.0-pre.2 | 2026-10-15 | Escrow wallets (`sealforge-chain`). Verified scanning Ironwood notes on regtest |
| sqlx | 0.9 | 2026-10-15 | 0.8 clashed with rusqlite over `libsqlite3-sys` (Cargo `links`) |
| `zcash_keys` / `zcash_protocol` | =0.17.0-pre.2 / =0.11.0-pre.1 | 2026-10-14 | `zcash_keys` with `orchard` + `unstable-frost`. `zcash_protocol` knows NU7 |
| `pczt` | =0.10.0-pre.2 | 2026-10-14 | Has `shielded_sighash` and `apply_ironwood_signature` |
| FROST | `reddsa =0.5.2` (`frost`), `frost-rerandomized =3.0.0` | 2026-10-14 | See docs/spikes/frost-pczt.md |
| `orchard` / `reddsa` | 0.16 / 0.6.1 | 2026-10-14 | Ironwood value pool. Consensus signature check |
| `zcash-devtool` | TBD (git rev) | | PCZT flows |
| WebZjs | TBD | | Browser wallet |
