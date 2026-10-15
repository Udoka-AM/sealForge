# sealForge threat model

What sealForge protects, against whom, and where the limits are. Every row points at the code or test that enforces it. Where a protection isn't built yet, this document says so.

**Status (2026-10-15):** auction logic, escrow cryptography, close/publish, receipts, the auditor (published-list mode) and refund planning are built and tested. On-chain scanning, PCZT signing of refunds and ShieldRamp depend on the testnet node and aren't live yet.

## Assets

| Asset | Why it matters |
|---|---|
| Bidder identity (which wallet bid) | The core promise: price public, bidders private |
| Bid amounts during the auction | Leaking them lets others bid strategically |
| Escrowed funds | Bidder money until refunds and payout |
| Integrity of the result | Price, winners and refunds must be checkable by anyone |
| Refund flow | Refunds must not re-identify bidders |

## Adversaries and protections

| Adversary | Protected? | How | Enforced by |
|---|---|---|---|
| **Public chain observer, during the auction** | ✅ | Bids are shielded Ironwood payments. Amount, sender and memo are encrypted | Zcash shielded pool. Bid memo grammar in `sealforge-core::memo` |
| **Public chain observer, after close** | ✅ amounts revealed, ✅ bidders hidden | The published UIVK reveals incoming notes (amount, memo) but never senders. Refund addresses are fresh diversified addresses | `POST /publish` derives the UIVK from the encrypted UFVK (`routes/auctions.rs`) |
| **Organiser publishing a false result** | ✅ | Anyone recomputes price, volume, winners, every outcome and the Merkle root with the auditor. Bidders check their own inclusion from their receipt | `sealforge-auditor::audit`, `verify_receipt`. Tests catch inflated volume, hidden bids and rewritten refunds |
| **Organiser hiding a bid from the published list** | ⚠️ Not yet | Needs the chain rescan with the UIVK (task 3.4). Today's audit checks the published list is internally consistent, and the audit page says so | Planned: auditor chain scan |
| **Auction operator / trustees** | ⚠️ Partial in deposit mode, ✅ in sealed-reveal mode | Deposit mode: trustees and the coordinator see bid amounts live, and the auction page says so. Sealed-reveal mode (commit-reveal): during bidding they see only deposits, which are standard budget amounts at or above each bid. Prices appear only in reveals sent after the close, which the chain audit checks | `sealforge-core::selection` commit-reveal tests, `tests/commit_reveal.rs`, `docs/evidence/regtest-e2e.md` (sealed-bid run) |
| **Single rogue trustee** | ✅ | FROST 2-of-3: one key share can't produce a spend signature | `sealforge-escrow` tests: every 2-of-3 subset signs, 1 cannot. Signatures pass Orchard's consensus check |
| **Coordinator during key generation** | ✅ | It only relays packages. Round 1 is signed by the sending trustee, and round 2 is also sealed to the recipient. The escrow is derived only if all three trustees report the same key. Trustees compare a fingerprint out of band | `routes/dkg.rs`, `sealforge-cli::trustee`. End-to-end test with a forged package |
| **Replay or forgery of organiser/trustee API calls** | ✅ | Ed25519 `SF-Sig` request signatures, ±60 s window, per-key nonce store | `sealforge-api::auth`, API integration tests |
| **Refund correlation at the exit** (amount matching when a bidder unshields or swaps a refund) | ✅ Mitigated | Canonical notes only, dust not sent, independent random delays, mixed batches. Measured: **0.1%** of refunds linkable vs **100%** for naive refunds | `sealforge-core::refunds`, `tools/linkability-harness`, [evidence](evidence/linkability.md) |
| **Source-chain observer (ShieldRamp)** | ⚠️ Reduced, not removed | EVM deposits are public. Planned mitigations: canonical amounts, split swaps, delays, an ephemeral wallet and a Linkability Score shown before signing. Never described as "untraceable" | Planned: ShieldRamp (needs 1Click access) |
| **The sealForge website** | ✅ | Receipt salts, refund addresses and receipts are created and kept in the browser. The audit runs the Rust auditor as WASM with published build provenance. No third-party font or analytics requests | `@sealforge/bid` (byte-compatible with Rust), `public/auditor/BUILD.json`, network checks in PR #6 and #9 |

## Data the coordinator holds

| Data | Protection | Retention |
|---|---|---|
| Escrow UFVK | `pgp_sym_encrypt` with `SF_DATA_KEY` | Until settled + 30 days |
| Refund addresses (from memos) | Encrypted column | Until refunds confirm + 30 days |
| Organiser and trustee public keys | Public keys only | Life of the auction |
| FROST key shares | **Never held.** They stay on trustee machines (owner-only files) | n/a |
| Receipt salts | **Never held.** Browser only | n/a |
| Client IPs | Dropped from access logs (Caddy filter). Not logged by the app | n/a |

## Known limits

1. **Completeness of the bid list** needs the chain rescan (task 3.4). Until then, a dishonest coordinator could omit a bid from the published list, although the bidder's own Verify check would then fail and expose it.
2. **Trustees see bids live.** Commit-reveal is a stretch goal.
3. **Exit behaviour matters.** A bidder who exits all refund notes at once recreates their refund amount. The exit path must warn about this.
4. **Rare, large refunds** in small auctions can produce a uniquely sized note (the residual 0.1–0.4%).
5. **Unreadable memos** can't be refunded automatically. They are counted for manual review.
