# Changelog

All notable changes to sealForge are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org). Security fixes are also listed in [docs/SECURITY-REVIEW.md](docs/SECURITY-REVIEW.md).

## [Unreleased]

### Added

- Per-client API rate limits (GCRA) for reads and writes, keyed by a salted hash of the client address that is never stored or logged, plus a cap on requests in flight. Over-budget requests get `429` with `Retry-After`. (#77)
- Retention: 30 days after an auction is settled or cancelled, its escrow viewing key, escrow wallet file and bidders' refund addresses are erased, with an audit-log entry recording counts only. (#80)
- Protocol fee: an auction carries a rate of 0 or 5% to 15% of its proceeds, fixed before it opens and paid as a second output of the payout. Trustees refuse a payout with the wrong amount or address, or one that pays the fee twice; the auditor recomputes it. The operator sets an auction's rate through an admin endpoint (`SF_ADMIN_TOKEN`). (#75)
- Trustees vouch for their per-auction keys with a published long-term key, shown on the auction page and checked in the browser and by `sealforge-audit`. (#53)
- Trustee backups (`sealforge trustee backup`), and a sweep of an escrow from two trustees' backups with no coordinator (`sealforge escrow recover-start`, `recover-cosign`, `recover-finish`). (#52, #71)
- Trustees decode and check every escrow transaction before signing, in the CLI and the browser: every output must be owed, each refund goes to its bid's refund address read from the chain, and no note is paid twice. (#47, #48, #70)
- Co-signers confirm they were shown the same commitments before releasing a signature share. (#51)
- Reproducible WebAssembly builds in a pinned container, byte-checked in CI. (#69)
- Security policy with private vulnerability reporting. (#68)
- Public documentation site, protocol specification and shared cross-language test vectors. (#34, #35)

### Changed

- Mainnet auctions open only when all three trustees have certified a long-term key, and the escrow check in browsers, the trustee CLI and `sealforge-audit` fails on mainnet otherwise. Per-auction value caps were decided against. (#78)
- Strict nonce-based Content Security Policy: `'unsafe-inline'` is gone from `script-src`; pages render per request with a fresh nonce. (#79)
- Escrow transactions stay under ZIP 218's per-block action limit: refunds wait behind a consolidation and large payouts split. (#43)
- Late payments are refunded before the organiser is paid. (#49)
- Batches that expire unmined are marked expired and rebuilt. (#50)
- The hosted site runs on a single server in Docker behind a Cloudflare tunnel. (#28, #33)

### Fixed

- FROST signing nonces are consumed before a share is sent, and the escrow address is checkable by trustees and bidders, not only the coordinator (F9, F10). (#39)
- An open auction can no longer be cancelled while a bid may be unscanned. (#42)
- Malformed trustee lists, unverified escrows on any payment path, and a public page that differs from what trustees attested are all refused (F13, F14, F15). (#44, #45, #46)

### Security

- `url` 2.5.9 for RUSTSEC-2026-0592 (stack overflow in `Url::origin` on nested `blob:` URLs).
- Dependency updates for RUSTSEC-2026-0385 (`ctutils`), RUSTSEC-2026-0382 (`rustls-webpki`) and RUSTSEC-2026-0381 (`webpki-roots`), and Next.js 16.4.6 for seven advisories, including cross-client response injection in self-hosted deployments. (#76)
- Next.js 16.4.2 for that release's advisories. (#24)

## [0.1.0] - 2026-10-22

First public release: sealed-bid auctions in shielded ZEC with a 2-of-3 FROST escrow, deposit and sealed-reveal bid modes, uniform-price and first-price clearing, unlinkable refunds, bidder receipts with Merkle inclusion proofs, an auditor in the CLI and in the browser, an in-browser wallet, the organiser console and browser trustee signing, running on Zcash testnet.

[Unreleased]: https://github.com/Udoka-AM/sealForge/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Udoka-AM/sealForge/releases/tag/v0.1.0
