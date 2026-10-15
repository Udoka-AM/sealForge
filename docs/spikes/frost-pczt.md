# Spike: FROST 2-of-3 escrow for Ironwood spends (task 1.5)

**Status (2026-10-15):** ✅ done. A real 2-of-3 FROST spend from the escrow was mined on a local Zcash chain (Zebra 7.0.0-rc.0 regtest with Ironwood, via `scripts/regtest.sh`). A single trustee is refused before signing.

## Result

| Step | Status | Evidence |
|---|---|---|
| 2-of-3 DKG (three rounds, run per trustee) | ✅ | `sealforge-escrow::dkg`, test `two_of_three_produces_consensus_valid_spend_auth_signatures` |
| Group key `ak` has even Y (Orchard requirement) | ✅ | Enforced by the ciphersuite's `post_dkg` hook. Asserted in tests |
| Escrow Orchard/Ironwood FVK from `ak` + fresh `nk`, `rivk` (ZIP 312) | ✅ | `address::escrow_fvk`. FVK round-trips with `ak` = group key |
| Testnet Unified Address, UFVK and UIVK | ✅ | `address::escrow_address`, `escrow_view_keys` (`utest1…`, `uviewtest1…`, `uivktest1…`) |
| Rerandomized threshold signature over sighash with PCZT's α | ✅ | Verifies under Orchard's consensus RedPallas `SpendAuth` check with `rk = ak + [α]G` (reddsa 0.6, as used by `orchard`) |
| Every 2-of-3 subset can sign | ✅ | {1,2}, {1,3}, {2,3} |
| One trustee alone cannot sign | ✅ | test `one_trustee_cannot_sign_alone` |
| Signature bound to α and sighash | ✅ | Changing either fails verification |
| Real transaction spending from the escrow | ✅ | Regtest txid `8367e1c6ca8ef305678a7179faf3d161647f90da40bb5462ecc6dc6275ec016f`, mined at height 437 (2 Ironwood actions). Trustees 1 and 3 signed. The recipient received 1 ZEC with memo "sealForge escrow payout (2-of-3 FROST)" |
| One trustee alone can't spend on chain | ✅ | Same flow with only trustee 1: refused at FROST signing ("Incorrect number of commitments"), so nothing is broadcast |

Try it:

```bash
cargo test -p sealforge-escrow
cargo run -p sealforge-cli -- escrow local-dkg --out ./escrow-testnet   # prints a testnet escrow UA
```

`local-dkg` holds all three shares on one machine, so it is for testnet only and refuses mainnet.

## Library choices

- **FROST ciphersuite:** `reddsa = 0.5.2` with the `frost` feature (renamed `reddsa-frost` in `Cargo.toml`). ZF removed FROST from `reddsa 0.6` and plans to move the ciphersuites into the `ZcashFoundation/frost` repo, which doesn't have them yet (checked 2026-10-14). 0.5.2 is ZF-published and built on `frost-rerandomized 3.0.0` / `frost-core 3.0.0`. **Revisit** when ZF publishes the RedPallas ciphersuite.
- **Alternative considered:** `zakura-reddsa-frost 3.0.0` (published 2026-10-13 by ebfull/p0mvn). Credible authors but 40 downloads. Not chosen for a custody path yet.
- **Signing with a fixed α:** Orchard fixes the spend randomizer in the PCZT, so we call `frost_rerandomized::sign` with `Randomizer::deserialize(α)`. Upstream deprecated it in favour of seed-derived randomizers, which doesn't fit Orchard. The call is `#[allow(deprecated)]` with a comment.
- **Two reddsa versions:** 0.5.2 (FROST) and 0.6.1 (what `orchard 0.16` verifies with). They interoperate through 32-byte keys and 64-byte signatures, and the tests verify FROST output with 0.6.1.

## On-chain flow (as built, `sealforge-chain::spend`)

1. `EscrowWallet` (view-only `zcash_client_sqlite` account from the escrow UFVK) syncs from Zaino.
2. `build_payment`: `propose_transfer`, then `create_pczt_from_proposal`, then IO finalizer, then the Ironwood proof (`PostNu6_3` circuit). Returns the shielded sighash and, for each escrow spend, the action index and α (read through `pczt::roles::low_level_signer`).
3. Trustees sign each `(sighash, α)` with `sealforge_escrow::signing` (2-of-3 rerandomized FROST).
4. `apply_signatures` checks each signature against the action's `rk` and applies it.
5. `finalize` (`extract_and_store_transaction_from_pczt`), then `broadcast`.

Reproduce: `cargo run -p sealforge-chain --example escrow_spend -- <escrow.sqlite> <local-dkg dir> <to-ua> <zat> 1,3`.
