# Security review (task 5.1, PRD AC14)

**Scope:** coordinator, chain worker, CLI, escrow and signer crates, auditor, and the web app as of branch `task/5-quality` (2026-10-15).
**Method:** code reading against the threat model, dependency audits, and attacks tried on the local regtest chain.
**Status:** every AC14 check passes, with fixes below. Open items are listed at the end with owners.

## AC14 checks

| Check | Result | Evidence |
|---|---|---|
| **No transparent hops** in bid, refund or payout flows | Pass | The escrow address has only an Orchard receiver (`sealforge-escrow::address::escrow_address`). Escrow spends are built with an Ironwood-only change strategy (`sealforge-chain::spend`). Refund and payout addresses must have an Orchard receiver (fixed in this review, see F2). The scanner reads only Orchard and Ironwood notes. ShieldRamp's Intents leg is not built yet (task 4.1); PRD Q4 tracks its transparent-leg question |
| **No address reuse** | Pass, with a user-side caveat | One escrow address per auction, derived from a fresh DKG. The bid form asks for "a fresh Unified Address from your wallet, used only for this bid". The app can't enforce that for an address the bidder pastes. The ephemeral-wallet path (task 4.7) will generate one per bid |
| **No PII in memos** | Pass | Bid memo: `SF1:<code>:<commitment>:<refund_ua>`. The commitment is a salted hash. Refund and payout transactions carry no memo. Fee-buffer memo: `SF1FEE:<code>` |
| **No PII in logs or URLs** | Pass | Caddy drops client IPs, and Caddy redacts `Authorization` by default. The coordinator logs method and path only, and paths contain auction codes and batch ids. The console proxy forwards no client IP headers. No commitment, address or amount appears in any URL or query string (`grep` of `apps/web/src`) |
| **Secrets** | Pass | No keys, `.env` files or key shares tracked in git (`git ls-files`, history grep). `SF_DATA_KEY`, `ONECLICK_JWT` and the Postgres password come from the environment. Escrow UFVKs and refund addresses are pgcrypto-encrypted at rest |
| **Dependencies** | Pass after F1 | `cargo audit`: rustls advisories fixed. Allowed warning: `atomic-polyfill` is unmaintained, but it's only active on targets without atomics (via `frost-core → postcard → heapless`). `pnpm audit --prod`: no known vulnerabilities |

## Findings fixed in this review

| # | Severity | Finding | Fix |
|---|---|---|---|
| F1 | Medium | rustls 0.23.45 had four advisories published 2026-10-14 (RUSTSEC-2026-0346..0349) | Updated to 0.23.46 |
| F2 | **High** | A bid memo's refund address was stored unchecked. One bid with a malformed, wrong-network or transparent refund address made every refund batch it joined fail to build, blocking refunds for all bidders. A Sapling-only address would also have crossed pools and revealed the amount | Unusable addresses are dropped at ingest: the bid still counts, and its refund goes to manual review. Refund and payout addresses must have an Orchard receiver. Covered in `scanner_ingest` |
| F3 | High (availability) | Trustees finishing the DKG together deadlocked in Postgres (HTTP 500) | DKG writes lock the auction row first. The concurrent e2e test fails 5 of 5 times without the fix |
| F4 | Medium (availability) | A signer chosen for a batch who goes offline (or loses their nonces) stranded the batch forever | Signing rounds reset after 30 minutes (`signers_selected_at`). The CLI discards nonces from a reset round instead of refusing to recommit |
| F5 | Medium | The web app sent no CSP and no framing protection. The console and trustee pages sign with keys stored in the browser, so a framing site could clickjack them | Production CSP: same-origin scripts and WebAssembly only, `frame-ancestors 'none'`, plus `X-Frame-Options: DENY`. Verified that the WASM auditor and the trustee signer still run under it |
| F6 | Low | Escrow wallet files (viewing-key material) were created with default permissions | The wallet directory is created owner-only (0700) |
| F7 | Low | The restore drill passed `SF_DATA_KEY` on the `psql` command line, where `ps` can see it | SQL goes through stdin |
| F8 | Medium | Payments arriving after close were recorded as valid bids with no refund planned | Stored as `late`, refunded in full by a late-refund planner |

## Key handling (what an attacker gets)

| Asset | Where it lives | If stolen |
|---|---|---|
| Trustee FROST share | Trustee's disk (`key-package.hex`, 0600), or browser memory while `/trustee` is open | One share can't spend. Two can, together with the escrow FVK |
| Escrow UFVK | Coordinator DB, encrypted with `SF_DATA_KEY` | Bids become visible before close (breaks sealing). Can't spend without two shares |
| `SF_DATA_KEY` | Coordinator environment, password manager, offline copy | With a DB dump: UFVKs and refund addresses. **If lost, escrows can't be spent** (see `infra/README.md` §Backups) |
| Organiser key | `~/.sealforge/organiser.key`, or a non-extractable WebCrypto key in IndexedDB | Can create, open, publish and cancel that organiser's auctions. Can't move funds |
| Nonces (FROST round 1) | Trustee disk until round 2, then deleted. Browser memory only | A nonce plus that round's share would reveal the trustee's signing share. Never reused, and deleted after use |

## Open items

| Item | Why it's open | Owner |
|---|---|---|
| Rate limiting on public endpoints | Not needed on regtest. Add a Caddy rate limit before mainnet | Ops, task 6.2 |
| Strict nonce-based CSP | Next's hydration needs `'unsafe-inline'` without per-request nonces. React escaping is the primary XSS defence, and there are no `dangerouslySetInnerHTML` sinks | Web |
| Coordinator sees bids before close | By design for v1: it holds the UFVK to scan. Commit-reveal mode (task 4.6) removes the amount from the memo | Product |
| Browser trustee: the key share is in JS memory while the page is open | Same trust as the CLI on that machine. Recommend the CLI for large auctions | Docs |
| ShieldRamp transparent leg (PRD Q4) | Feature not built (needs 1Click access) | Task 4.1 |
