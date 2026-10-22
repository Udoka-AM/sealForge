# Test plan run (task 6.1): AC1–AC14

**Date:** 2026-10-17. **Build:** `main` at the merge of PR #19, plus the commits on `task/6.3-testnet-auction`. **Where:** public Zcash testnet through the hosted instance `https://sealforge.udokaam.dev`, the local regtest chain, and the automated suites.

**Automated suites at this commit:** `cargo test --workspace` 85 passed, 0 failed. `pnpm test` 9 passed. `cargo clippy --workspace --all-targets -D warnings`, `cargo fmt --check`, `pnpm lint` and `pnpm typecheck` all clean.

**Testnet auctions used below:**
- `KE5DFF4PDA` Genesis Pass: 7 sealed bids, 4 units ([testnet-genesis.md](testnet-genesis.md))
- `K7GHHBC4GP` Community Pass: 25 sealed bids, 10 units (this document)

| AC | Result | Evidence |
|---|---|---|
| AC1 DKG → escrow → open | **Pass** | Testnet: Greg, Harry and Dmitry ran the DKG at the same moment. It finished in about 10 seconds with the same group key on all three, and the escrow `utest1huqeq0…` appears under "Escrow (2-of-3 trustees)" on `/a/KE5DFF4PDA`. The auction then opened. Test: `dkg_e2e::three_trustees_run_the_dkg_through_the_coordinator`, which now joins all three concurrently |
| AC2 bid recorded within 2 blocks | **Pass** | Testnet `K7GHHBC4GP`: each bid's block height against the coordinator's tip when the bid first appeared, sampled every 3 s. All 25 bids were recorded within 2 blocks: 13 at the same height as the coordinator's tip, 9 one block later, 1 two blocks later, and 2 seen before the coordinator's 25-second tip refresh had caught up. The raw samples are in `~/.sealforge-host/testnet/community/detection-lag.tsv` on the host. The public count went from 5 to 25 as bids arrived. The public page shows the bid count once 5 or more bids exist (`MIN_PUBLIC_BID_COUNT`, so a count can't single out the first bidders) |
| AC3 invalid payments | **Pass** | Testnet: Genesis bidder 6's price was under the reserve, so it was recorded `under_reserve`, excluded from clearing and refunded in full (0.0501 TAZ, reveal included). `K7GHHBC4GP`: two bids priced under the 0.003 TAZ reserve (0.002 and 0.0015) were recorded as `under_reserve`. Two unrevealed deposits were `not_revealed`. All four were excluded from clearing and refunded in full. Tests: `scanner_ingest::records_payments_with_memo_level_checks` covers malformed memos and `wrong_auction`; `close_publish::closes_at_close_height_and_publishes_an_auditable_result` covers `under_reserve`, `wrong_auction` and `before_open` |
| AC4 close rules | **Pass** | Testnet: both auctions closed only after 10 blocks past the last window, and an unrevealed deposit was refunded in full. Late, before-open and duplicate payments are covered by tests on purpose, since a public chain run can't force them: `close_publish` (E late, F duplicate of A keeps the earliest, before_open) and `scanner_ingest` (a payment after close is `late` and refunded in full). The same `sealforge-core::selection` runs in the coordinator, the CLI auditor and the WASM auditor |
| AC5 uniform-price clearing | **Pass** | Testnet `KE5DFF4PDA`: 4 units, 5 valid revealed bids. The top 4 won, and each paid the 4th-highest price (0.07 TAZ). `K7GHHBC4GP`: 10 units, 21 valid revealed bids. The top 10 won, and each paid the 10th-highest price, 0.0072 TAZ, the value predicted from the plan before the run. Tests: `sealforge-core` clearing tests and `pipeline` fixtures (20-bid uniform, 500-bid first-price, ties) |
| AC6 publish | **Pass** | `/audit/KE5DFF4PDA` shows the UIVK, rules, block range 4,507,132–4,507,214 and Merkle root `ecb24fda…e8f17380`. The same fields are at `/api/auctions/<code>/result` |
| AC7 independent recompute | **Pass** | Testnet: `sealforge-audit` (CLI, `testnet.zec.rocks`) and **Rescan the chain in my browser** (WASM, gRPC-web) both reported a match on price, volume, winners, root and every outcome for `KE5DFF4PDA`. `K7GHHBC4GP`: `sealforge-audit` scanned 4,509,858–4,509,959 and found 49 payments (25 deposits, 23 reveals, the fee buffer). Match on every field, root `6886bdbe…17c33a02`. A tampered result is caught: [audit-rescan.md](audit-rescan.md) |
| AC8 receipt inclusion | **Pass** | Testnet: Genesis bidder 3's receipt, loaded into `/verify` on the hosted site, showed "Your bid is included". Its Merkle proof matches the published root, checked in the browser: won at 0.07 TAZ, 0.0301 TAZ refund as 3 notes (0.02, 0.01, 0.0001), confirmed in block 4,507,163 |
| AC9 refund planning | **Pass** | Testnet `KE5DFF4PDA`: 19 notes in canonical denominations (0.1, 0.05, 0.02, 0.01, 0.0001 TAZ), sent in two batches. Each batch mixes notes from 5 different bidders, and each note had its own random delay. The demo ran with `SF_REFUND_DELAY_SECS=20,120`, which is test networks only; the 10–240 min default is covered by `close_publish` (nothing due at first, everything due five hours later) and `sealforge-core::refunds` tests. `K7GHHBC4GP`: 73 notes in 5 refund transactions of at most 20 outputs. The three full batches mixed notes from 20, 20 and 14 different bidders |
| AC10 2-of-3 signing | **Pass** | Testnet: every refund and payout was signed by 2 of the 3 trustees, with a different pair each time (Greg and Dmitry, Dmitry and Harry, Greg and Harry), then broadcast and confirmed. `K7GHHBC4GP`: the 5 refunds and the payout used three different pairs: Greg and Harry (four transactions), Greg and Dmitry, and Harry and Dmitry. One signature can't finalise: FROST refuses a one-signer package and won't aggregate a single share (`dkg_e2e`, added in this run), and the coordinator only aggregates once both chosen signers have posted shares |
| AC11 ShieldRamp quote + Linkability Score | **Not met** | Only the scoring module exists (`packages/shieldramp/src/score.ts`, unit-tested). The 1Click quote route returns "not implemented", and there is no MetaMask flow. It needs a 1Click JWT and mainnet funds (tasks 1.7, 4.1, 4.2) |
| AC12 ShieldRamp delayed bid | **Not met** | Depends on AC11. Not built |
| AC13 linkability harness | **Pass** | [linkability.md](linkability.md): naive refunds 100% linkable, sealForge 0.1% (target under 5%). Chart: `linkability.svg` |
| AC14 no transparent hops, reuse or PII | **Pass** | [SECURITY-REVIEW.md](../SECURITY-REVIEW.md). On testnet, every escrow payment the scanner recorded was in the Ironwood pool, refunds and payouts were built Ironwood-only, refund and payout transactions carry no memo, and bid memos hold only the code, a salted commitment and the refund address. Caveat for the demo runs: each simulated bidder's refund address was its wallet's own address, which also received its funding. The browser flow asks for a fresh address per bid |

## The 25-bid auction (`K7GHHBC4GP`)

**Community Pass:** 10 units, reserve 0.003 TAZ, sealed bids from 25 wallets. Deposits were 0.005, 0.01 or 0.02 TAZ, with hidden prices from 0.0015 to 0.018. Two bidders never revealed and two bid under the reserve. Run with `scripts/testnet-demo.sh`, which now schedules bids against the clock and sends reveals six at a time.

| Step | Time (2026-10-17) | Result |
|---|---|---|
| Fund, create, DKG, open | 00:29–00:33 | 25 wallets funded in one transaction. DKG with Greg, Harry and Dmitry in 30 s. Opened at 4,509,858 |
| Bids | 00:33–00:50 | 25 sealed bids, one every 42 s, all before the close at 4,509,923 |
| Reveals | 00:51–01:01 | 23 reveals in the window ending 4,509,959. 2 bidders deliberately silent |
| Close, publish | 01:19 | Price 0.0072 TAZ, 10 winners, 21 valid bids. Chain audit matches |
| Refunds | 01:20–01:33 | 5 transactions, 73 notes, 0.1803 TAZ, confirmed at 4,509,981 to 4,510,008 |
| Payout | 01:36 | 0.0675 TAZ (0.072 proceeds plus the 0.001 buffer, minus 0.0055 of escrow fees), confirmed at 4,510,017 |
| Settled | 01:37 | 68 minutes after launch. Testnet blocks averaged about a minute during the run, slower than the 25 s target |

Spot-checked wallets ended exactly where expected:
- Winning bidder 0: 0.013 TAZ
- Unrevealed bidder 6: 0.0103 TAZ, the deposit back in full
- Under-reserve bidder 8: 0.0052 TAZ
- Organiser's payout wallet: 0.0675 TAZ

## Not covered here

- **Mainnet (tasks 6.2, 6.3):** no mainnet auction has run. It needs real ZEC, and the testnet runs stand in for it.
- **ShieldRamp:** AC11 and AC12, above.
- **Lighthouse and screen reader:** still open in [a11y.md](a11y.md). axe reports 0 serious issues on every page.
