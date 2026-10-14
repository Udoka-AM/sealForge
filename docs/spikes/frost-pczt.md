# Spike: FROST 2-of-3 escrow for Ironwood spends (task 1.5)

**Status (2026-10-14):** cryptography proven off-chain. On-chain spend pending a synced NU7 testnet node (task 1.3).

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
| Real testnet transaction spending from the escrow | ⏳ | Needs Zebra + Zaino on NU7 testnet and testnet ZEC |

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

## On-chain plan (unblocks when the testnet node is up)

1. `sealforge escrow local-dkg` → fund the printed `utest1…` UA from a faucet or a team wallet.
2. Scan the escrow with its UFVK (`zcash_client_sqlite` + Zaino) until the note is spendable.
3. Build a proposal paying a team wallet, then `create_pczt_from_proposal`.
4. IO finalizer + prover (Orchard proving key) on the PCZT.
5. `pczt::roles::signer::Signer::shielded_sighash()` + each action's α → `sealforge_escrow::signing` with trustees 1 and 3 → `apply_ironwood_signature`.
6. Spend finalizer → tx extractor → broadcast via Zaino. Record the txid here.
7. Negative check: the same flow with one share must fail before broadcast.
