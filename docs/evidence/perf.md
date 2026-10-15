# Performance pass (task 5.5)

Measured 2026-10-15 on an Apple M3 Pro laptop, against the local regtest stack (release builds). TRD §2.8 targets.

| Metric | Target | Measured | How |
|---|---|---|---|
| Clearing computation, 20,000 bids | < 1 s | **12.8 ms**, covering selection, clearing and the Merkle root | `cargo run --release -p sealforge-core --example bench_close` (best of 5) |
| API reads, p95 | < 300 ms | **2–12 ms** at 20 concurrent clients | `ab -n 2000 -c 20` on `/v1/chain` (26k req/s), `/v1/auctions` (2.2k req/s), `/v1/auctions/{code}` (3.7k req/s), `/v1/auctions/{code}/result` (2.0k req/s) |
| API writes, p95 | < 800 ms | **13.6 ms** (p99 40.7 ms) at 10 concurrent clients | 500 SF-Sig-signed `POST /v1/auctions` (validation, 3 trustee invites, audit log) |
| Bid detection lag | ≤ 2 blocks | **≤ 25 s** (one scanner tick) plus ~0.1 s to sync and record, so under one 75 s block | Coordinator log: `Scanning Historic(445..449)` → `recorded escrow payments` 88 ms later |
| Auditor rescan, CLI | < 15 s for 3,500 blocks | **0.08 s for 646 blocks** of local chain | `sealforge-audit --from 1 --to 646` |
| Page JS, auction page | < 200 KB gzipped | **176 KB** (bid flow 192 KB, console 179 KB) | Production build. Every `/_next/static` script on the page, `gzip -9` |

Not yet measured, because they need the hosted testnet stack (task 1.3):
- **Auditor rescan on real blocks.** Regtest blocks are nearly empty. Testnet blocks carry other users' Orchard and Ironwood actions, so trial decryption costs more per block. To be re-run over a 3,500-block testnet range.
- **Browser rescan (WASM).** The browser Audit page checks the published list. The chain rescan runs in the CLI only for now.
- **LCP on 4G.** Needs Lighthouse against the deployed site.
