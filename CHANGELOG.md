# Changelog

All notable changes to sealForge are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org). Security fixes are also listed in [docs/SECURITY-REVIEW.md](docs/SECURITY-REVIEW.md).

## [Unreleased]

### Added

- Protocol fee: an auction carries a rate of 0 or 5% to 15% of its proceeds, fixed before it opens and paid as a second output of the payout. Trustees refuse a payout with the wrong amount or address, or one that pays the fee twice; the auditor recomputes it. The operator sets an auction's rate through an admin endpoint (`SF_ADMIN_TOKEN`). (#75)
- Trustees vouch for their per-auction keys with a published long-term key, shown on the auction page and checked in the browser and by `sealforge-audit`. (#53)
- Trustee backups (`sealforge trustee backup`), and a sweep of an escrow from two trustees' backups with no coordinator (`sealforge escrow recover-start`, `recover-cosign`, `recover-finish`). (#52, #71)
- Trustees decode and check every escrow transaction before signing, in the CLI and the browser: every output must be owed, each refund goes to its bid's refund address read from the chain, and no note is paid twice. (#47, #48, #70)
- Co-signers confirm they were shown the same commitments before releasing a signature share. (#51)
- Reproducible WebAssembly builds in a pinned container, byte-checked in CI. (#69)
- Security policy with private vulnerability reporting. (#68)
- Public documentation site, protocol specification and shared cross-language test vectors. (#34, #35)

### Changed

- Escrow transactions stay under ZIP 218's per-block action limit: refunds wait behind a consolidation and large payouts split. (#43)
- Late payments are refunded before the organiser is paid. (#49)
- Batches that expire unmined are marked expired and rebuilt. (#50)
- The hosted site runs on a single server in Docker behind a Cloudflare tunnel. (#28, #33)

### Fixed

- FROST signing nonces are consumed before a share is sent, and the escrow address is checkable by trustees and bidders, not only the coordinator (F9, F10). (#39)
- An open auction can no longer be cancelled while a bid may be unscanned. (#42)
- Malformed trustee lists, unverified escrows on any payment path, and a public page that differs from what trustees attested are all refused (F13, F14, F15). (#44, #45, #46)

### Security

- Next.js 16.4.2 for that release's advisories. (#24)

## [0.1.0] - 2026-10-22

First public release: sealed-bid auctions in shielded ZEC with a 2-of-3 FROST escrow, deposit and sealed-reveal bid modes, uniform-price and first-price clearing, unlinkable refunds, bidder receipts with Merkle inclusion proofs, an auditor in the CLI and in the browser, an in-browser wallet, the organiser console and browser trustee signing, running on Zcash testnet.

[Unreleased]: https://github.com/Udoka-AM/sealForge/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Udoka-AM/sealForge/releases/tag/v0.1.0
