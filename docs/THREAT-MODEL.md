# sealForge threat model

What sealForge protects, against whom, and where the limits are. Every row points at the code or test that enforces it. Where a protection isn't built, this document says so.

**Status (2026-11-13, `main` at `3a6002e`):** auction logic, escrow cryptography (FROST 2-of-3 DKG and rerandomized signing of Ironwood spends through PCZTs), close and publish, receipts, both bid modes (deposit and sealed-reveal), refund planning, and the independent chain audit (CLI and in-browser) are built, tested, and have run on public Zcash testnet (auctions `KE5DFF4PDA` and `K7GHHBC4GP`, see [evidence/testnet-genesis.md](evidence/testnet-genesis.md) and [evidence/test-run.md](evidence/test-run.md)). No auction has run on mainnet. ShieldRamp's quote and swap leg is specified, not built.

## Trust assumptions

Stated first because every protection below depends on them.

1. **Deposit mode: the coordinator and trustees see bid amounts live.** The coordinator holds the escrow UFVK to scan for bids, so it decrypts each bid's amount and memo as it arrives. Sealed-reveal mode removes the price from what they see until the reveal window (see below).
2. **The coordinator samples `nk` and `rivk` and holds the escrow UFVK.** When the last trustee completes the DKG, the coordinator builds the Orchard FVK `ak ‖ nk ‖ rivk` from the FROST group key `ak` and fresh `nk`, `rivk` from `OsRng` (`crates/sealforge-coordinator/src/routes/dkg.rs`, `sealforge_escrow::address::escrow_fvk`). It stores the UFVK encrypted with `SF_DATA_KEY`. This gives the coordinator viewing power over the escrow, not spending power: spends need a RedPallas signature under `ak`, which needs two FROST shares.
   - A database dump **plus** `SF_DATA_KEY`: the attacker reads every escrow's incoming and outgoing notes, including bids before close (sealing breaks for deposit-mode amounts) and refund addresses. Funds don't move.
   - Two trustee key shares: the attacker can produce spend authorization signatures. To build a spend they also need the escrow FVK (from the coordinator database and `SF_DATA_KEY`) to construct the transaction.
3. **Coordinator liveness.** Refunds and the payout need the coordinator (to build PCZTs and aggregate) plus two of the three trustees. A stopped coordinator delays an auction. It can't redirect funds: deposits stay in the escrow until two trustees sign ([evidence/testnet-genesis.md](evidence/testnet-genesis.md) §Timing records a real outage).
4. **The published UIVK is permanent.** At publish, the UIVK is released so anyone can audit. From then on, anyone with it can decrypt every incoming note to that escrow: each deposit's amount and memo (auction code, commitment, refund address) and, in sealed-reveal mode, each reveal (price and salt). This is by design. It does not reveal who sent the payments, and it does not decrypt outgoing refunds or the payout.

## Assets

| Asset | Why it matters |
|---|---|
| Bidder identity (which wallet bid) | The core promise: price public, bidders private |
| Bid amounts during the auction | Leaking them lets others bid strategically |
| Escrowed funds | Bidder money until refunds and payout |
| Integrity and completeness of the result | Price, winners and refunds must be checkable by anyone, including that no bid was left out |
| Refund flow | Refunds must not re-identify bidders |

## Adversaries and protections

| Adversary | Protected? | How | Enforced by |
|---|---|---|---|
| **Public chain observer, during the auction** | ✅ | Bids are shielded Ironwood payments. Amount, sender and memo are encrypted on chain | Zcash note encryption. Escrow UA has only an Orchard receiver (`sealforge_escrow::address::escrow_address`) |
| **Public chain observer, after publish** | ⚠️ Amounts revealed, ✅ senders hidden | The published UIVK decrypts incoming notes (amount, memo) but not senders. Refund addresses in memos become readable; the bid form asks for a fresh address per bid | `routes/auctions.rs` publish derives the UIVK from the encrypted UFVK. Trust assumption 4 |
| **Organiser or coordinator publishing a false result** | ✅ | Anyone recomputes price, volume, winners, every outcome and the Merkle root from the chain. Bidders check their own inclusion from their receipt | `sealforge_auditor::audit_chain`, `verify_receipt`. Tests: `inflated_volume_or_hidden_bid_is_caught`, `rewritten_outcome_is_caught`, `tampered_root_breaks_inclusion`. Testnet: [evidence/audit-rescan.md](evidence/audit-rescan.md) tamper test |
| **Organiser or coordinator hiding a bid from the published list** | ✅ within the published scan range | The auditor rescans the chain with the UIVK and a lightwallet server of the auditor's choice, reclassifies every payment with the same `sealforge-core::selection` code, and compares. A missing bid changes the bid count, outcomes and root. Residual: the default scan range is the published `scan_from_height..scan_to_height`; an auditor can widen it with `--from/--to` | `sealforge_chain::audit::scan_incoming`, `sealforge-scan`, `sealforge-audit` CLI, browser rescan. Commit-reveal: `commit_reveal_tests::a_suppressed_reveal_is_caught` |
| **Coordinator and trustees, deposit mode** | ⚠️ Amounts visible | They see bid amounts live. The auction page says so | Trust assumption 1 |
| **Coordinator and trustees, sealed-reveal mode** | ✅ prices hidden until the reveal window | During bidding they see only deposits (a budget at or above the hidden price) and a commitment `BLAKE2b(code ‖ 0 ‖ refund_ua ‖ 0 ‖ price_le ‖ salt)`. Prices appear only in reveals mined after `close_height`. Deposit amounts are still visible | `sealforge_core::receipt::price_commitment`, `selection::classify_commit_reveal`, `tests/commit_reveal.rs`. Testnet `KE5DFF4PDA`, `K7GHHBC4GP` |
| **Single rogue trustee** | ✅ | FROST 2-of-3: one share can't produce a spend signature, and FROST refuses a one-signer package | `sealforge-escrow` tests `one_trustee_cannot_sign_alone`, `two_of_three_produces_consensus_valid_spend_auth_signatures`. `dkg_e2e` |
| **Coordinator during key generation** | ✅ | It only relays packages. Round 1 is signed by the sending trustee; round 2 is also sealed to the recipient's X25519 key. The escrow is derived only if all three trustees report the same public key package. Trustees compare a fingerprint out of band | `routes/dkg.rs`, `sealforge-cli::trustee`. `dkg_e2e::three_trustees_run_the_dkg_through_the_coordinator` |
| **Replay or forgery of organiser/trustee API calls** | ✅ | Ed25519 `SF-Sig` request signatures, ±60 s window, per-key nonce store | `sealforge-api::auth`, `api` integration tests |
| **Refund correlation at the exit** (amount matching when a bidder unshields or swaps a refund) | ✅ Mitigated | Canonical notes only, dust not sent, independent random delays, mixed batches. Measured: **0.1%** of refunds linkable vs **100%** for naive refunds | `sealforge-core::refunds`, `tools/linkability-harness`, [evidence](evidence/linkability.md) |
| **Source-chain observer (ShieldRamp)** | ⚠️ Not built | EVM deposits are public. Planned mitigations: canonical amounts, split swaps, delays, an ephemeral wallet and a Linkability Score shown before signing. Only the scoring module exists | `packages/shieldramp/src/score.ts` (Specified) |
| **The sealForge website** | ✅ | Receipt salts, refund addresses and receipts are created and kept in the browser. The audit runs the Rust auditor and scanner as WASM with published build provenance. Production CSP: same-origin scripts and WASM, `frame-ancestors 'none'`. No third-party font or analytics requests | `@sealforge/bid` (byte-compatible with Rust), `apps/web/public/*/BUILD.json`, SECURITY-REVIEW F5 |

## Data the coordinator holds

| Data | Protection | Retention |
|---|---|---|
| Escrow UFVK | `pgp_sym_encrypt` with `SF_DATA_KEY` | Until settled + 30 days |
| `nk`, `rivk` | Inside the UFVK above. Sampled by the coordinator (trust assumption 2) | As the UFVK |
| Refund addresses (from memos) | Encrypted column. Also readable by anyone with the published UIVK after publish | Until refunds confirm + 30 days |
| Organiser and trustee public keys | Public keys only | Life of the auction |
| FROST key shares | **Never held.** They stay on trustee machines (owner-only files) or browser memory | n/a |
| Receipt salts | **Never held.** Browser only | n/a |
| Client IPs | Dropped from access logs (Caddy filter). Not logged by the app | n/a |

## Known limits

1. **Deposit mode exposes amounts to the operator.** Use sealed-reveal mode when that matters. Sealed-reveal still shows deposit sizes, which bound each price from above.
2. **The coordinator samples `nk`/`rivk`** and so can always view the escrow. Alternatives (organiser-sampled, or jointly derived by trustees) are an open question for reviewers.
3. **Audit range.** The auditor scans the published block range by default. Bids outside it are not checked unless the auditor widens the range. Bidders' own receipt checks (`/verify`) catch omission of their own bid.
4. **Exit behaviour matters.** A bidder who exits all refund notes at once recreates their refund amount. The exit path must warn about this.
5. **Rare, large refunds** in small auctions can produce a uniquely sized note (the residual 0.1–0.4%, [evidence/linkability.md](evidence/linkability.md)).
6. **Unreadable memos and unusable refund addresses** can't be refunded automatically. They are kept for manual review (SECURITY-REVIEW F2).
7. **Mainnet has not been run.** Mainnet gates are listed in [SECURITY-REVIEW.md](SECURITY-REVIEW.md) §Open items and the docs site roadmap.
