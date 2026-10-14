# Pinned versions

Filled in during Milestone 1, task 1.2 (PRD open question Q1). Record each pin with the reason and the date it was verified against NU7 testnet.

| Component | Version | Verified on | Notes |
|---|---|---|---|
| Rust toolchain | 1.89.0 | 2026-10-08 | `rust-toolchain.toml` |
| Node | 22 LTS | 2026-10-08 | `.nvmrc` |
| Next.js | 16.4.0 | 2026-10-08 | `apps/web` |
| Zebra (`zebrad`) | TBD | | Must support NU7 testnet + Ironwood |
| Zaino | TBD | | |
| `zcash_client_backend` | =0.25.0-pre.2 (when scanning lands) | 2026-10-14 | First release with Ironwood pools |
| `zcash_keys` / `zcash_protocol` | =0.17.0-pre.2 / =0.11.0-pre.1 | 2026-10-14 | `zcash_keys` with `orchard` + `unstable-frost`. `zcash_protocol` knows NU7 |
| `pczt` | =0.10.0-pre.2 | 2026-10-14 | Has `shielded_sighash` and `apply_ironwood_signature` |
| FROST | `reddsa =0.5.2` (`frost`), `frost-rerandomized =3.0.0` | 2026-10-14 | See docs/spikes/frost-pczt.md |
| `orchard` / `reddsa` | 0.16 / 0.6.1 | 2026-10-14 | Ironwood value pool. Consensus signature check |
| `zcash-devtool` | TBD (git rev) | | PCZT flows |
| WebZjs | TBD | | Browser wallet |
