# sealForge: Product Description

> Extracted from the Zecathon build plan (prepared 2026-10-05). The plan called this product **SEALED + ShieldRamp**. It ships as **sealForge**, and **ShieldRamp** stays the name of its cross-chain bidding widget.

*Prepared 2026-10-05. Build window: **3 weeks (Mon Oct 6 to Sun Oct 26, 2026)**, plus 2 buffer days before the **Oct 28, 23:59 UTC** deadline. Zcon7 runs Oct 27–29.*

> **Sources.** Figures come from thezecathon.com (site config), @zksnarks_ posts, crypto.news (Sep 21, 2026), the NEAR Intents explorer, nearstats.org, ZIP 318, Equilibrium's ZAIR write-up and Zcash forum threads.
>
> **Forecasts.** Revenue and volume numbers are **estimates built on stated assumptions**, not observed data. Treat them as pitch-ready scenarios and pressure-test them before quoting.

---

## 0. Hackathon ground rules

| Constraint | Effect on the build |
|---|---|
| One submission per account, **one track** | Two products means two accounts (separate teams/submitters). Pick each track deliberately. |
| Judged on **Privacy → Usefulness → Execution → Originality**. "Leaks are disqualifying, not deductions." | Every flow needs a written threat model. Never claim privacy you don't deliver. Working beats ambitious. |
| Code written during the window, open source at submission | Fresh repos from Oct 6. Libraries are allowed. |
| **Ironwood** (NU6.3) is live. Orchard is spend-only | Target Ironwood receivers and v6 transactions, not old Orchard tutorials. |
| **NU7 activates on testnet Oct 6** (25 s blocks, v4 tx invalid) | Pin Zebra, librustzcash and wallet versions that support NU7 testnet on day 1. |
| ZSAs (shielded assets) are **not live** | NFT "ownership" is issuer-side, as Zilkroad does it today. Design around that honestly. |
| Prize is paid in ZEC to a shielded address | Have a shielded UA ready for the submission form. |

---

**One-liner:** *Open-source, auditable sealed-bid auctions on Zcash that anyone can bid in from any chain. The clearing price is public and the bidders are not.*

**Track:** **Private Markets** ($15K), plus a Grand Prize run ($20K). Cross-chain bidding is a feature, not a second track entry.

## 1. Why this is a winning vertical

1. **It is the organisers' own thesis, as a product.**
   - The site's asset folder is literally `darkpool/`, and the Private Markets track is framed as the "dark-pool thesis": *the price should be public and the participants should not*.
   - sealForge turns that sentence into reusable infrastructure.
2. **It fixes the criticism of their September auction.** The zkSNARKs auction drew:
   - 16,971 bids, 8,000 winners, a 1.5 ZEC uniform clearing price;
   - **$36.94M of submitted volume** (crypto.news; zkSNARKs say ~$40M);
   - **$19M+ refunded** and **42,000 new wallets**.

   ZachXBT and Onchain Diary criticised that nobody outside could audit the clearing price or the volume. sealForge makes both **publicly recomputable** without exposing bidders.
3. **It fixes the cross-chain leak the press named.**
   - About **$19M arrived through Aurora/NEAR Intents across 1,718 swaps** (USDC 27%, ETH 21%).
   - Crypto.news noted that *"source-chain transfers remained publicly visible."*
   - ShieldRamp is the de-correlated, privacy-scored version of that onboarding path.
4. **The demand is proven, and it is not a toy market.**
   - NEAR Intents shows **~$268M of ZEC swap volume over 30 days** and **$2.4B all-time** (explorer.near-intents.org, Oct 5, 2026).
   - NEAR Intents is the only cross-chain resource listed on the Zecathon site.
5. **It matches the Zypherpunk pattern.** The largest Zcash hackathon prizes went to reusable infrastructure: zcash-router-sdk $13K, t2z $9K, cipherscan $9K. sealForge is a kit, not a one-off app.
6. **It uses deep Zcash features, which raises the Originality and Privacy scores.** Shielded escrow, FROST threshold custody, viewing-key auditability, ZIP-321 payment URIs, memo-encoded commitments, and denomination-based refunds borrowed from **ZIP 318's** migration-privacy design.

## 2. Product scope

| Component | What it does |
|---|---|
| **Auction Coordinator** | Creates auctions (uniform-price or first-price sealed bid), issues bid instructions, closes the book, computes the clearing result, schedules refunds. |
| **FROST Escrow (2-of-3)** | Holds bids in an Ironwood address whose spend authority is split across 3 trustees (organiser, independent auditor, community signer). No single party can move funds. |
| **Bid channel** | The bidder pays the escrow from any Zcash wallet through a **ZIP-321 URI/QR**. The memo carries `auction_id`, a bid **commitment** and a fresh **refund UA**. |
| **Public Auditor** | Once the book closes, the escrow's **incoming viewing key (IVK)** and a **Merkle root of all bids** are published. A static web page/CLI rescans the chain and recomputes clearing price, volume and winners. Every bidder gets an inclusion receipt. |
| **Refund Engine** | Splits each refund into canonical denominations (1/2/5 × 10^k ZEC, as ZIP 318 recommends), with random delays, batched across bidders and broadcast over Tor (optional). |
| **ShieldRamp widget** | Bid with USDC, ETH or SOL. It gets a NEAR Intents **1Click** quote, delivers ZEC to an **ephemeral in-browser Zcash wallet** (WebZjs), shields it, waits a randomised delay, then submits the bid in denominations. It shows a **Linkability Score** before the user signs. |
| **Exit path (stretch)** | Send winnings or refunds back out to USDC through Intents, with an explicit unshielding warning and score. |

### Threat model (put this in the README; judges reward honesty)

| Adversary | Protected? | How / caveat |
|---|---|---|
| Public chain observer, during the auction | ✅ | All bids are shielded transfers into Ironwood. Amounts, senders and memos are encrypted. |
| Public chain observer, after the close | ✅ amounts revealed / ✅ bidders hidden | The IVK reveals incoming notes (amount, memo), but **not sender addresses**. Refund UAs are fresh diversified addresses and can't be linked to a bidder's other addresses. |
| Auction operator / trustees | ⚠️ Partial | Anyone holding the IVK can see bids live. **Commit-reveal mode** fixes this: the deposit is a bucketed "budget", the memo holds `H(price ‖ salt)`, and the bid is revealed after the close. Deposit-as-bid is the default, with this caveat documented. Commit-reveal shipped as the per-auction "Sealed reveal" option (task 4.6). |
| Source-chain observer (ShieldRamp) | ⚠️ Reduced, not eliminated | The EVM/SOL deposit is public, and Intents' ZEC deposit side may be transparent. Mitigations: canonical amounts, split swaps, timing jitter, ephemeral wallet, mandatory delay before the bid. The **score quantifies residual risk**. Never call this "untraceable". |
| Single rogue trustee | ✅ | FROST 2-of-3. One key can't spend. |
| Refund correlation (amount + timing) | ✅ Mitigated | Denominations + random delays + cross-bidder batching. The demo shows the before/after linkability. |

## 3. Tech stack

| Layer | Choice | Notes |
|---|---|---|
| Node / indexer | **Zebra** (`zebrad`, testnet then mainnet) + **Zaino** or lightwalletd | gRPC for wallets. Envoy as gRPC-web proxy for the browser. |
| Zcash core | **librustzcash** crates (`zcash_client_backend`, `zcash_client_sqlite`, `zcash_primitives`, `zcash_keys`, shielded-pool crates) at an Ironwood/NU7-capable release | Verify exact versions on day 1 against NU7 testnet. |
| Threshold custody | ZF **FROST** tooling (`frost-client` / `frostd`, RedPallas via `reddsa`) + **`zcash-devtool`** PCZT flows | Use PCZTs to construct, sign (2-of-3) and finalise refund/payout transactions. |
| Coordinator backend | **Rust** (`axum`, `tokio`, `sqlx` + Postgres/SQLite) | Auction state machine, clearing engine, refund scheduler, Merkle builder. |
| Auditor | Rust CLI + **WASM** build for a static web page | Inputs: IVK, auction params, block range. Outputs: clearing price, volume, winners, Merkle root. |
| Browser wallet | **WebZjs** (ChainSafe) | Ephemeral wallet for ShieldRamp users. Seed stays local and can be exported. |
| Cross-chain | **NEAR Intents 1Click API** (quote → deposit address → status polling) | Mainnet-only, so demo with small amounts. Check whether an app-fee parameter is available for monetisation. |
| EVM / Solana | `wagmi` + `viem`, Solana wallet adapter (stretch) | USDC/ETH first. |
| Frontend | **Next.js + TypeScript + Tailwind** | Auction page, bid flow, auditor page, ShieldRamp widget (embeddable `<script>` / React component). |
| Network privacy | Arti (Tor) for broadcast (stretch) | Off by default in the demo, documented. |
| CI / QA | GitHub Actions, testnet integration tests, a linkability test harness | The harness doubles as demo evidence. |

**Team assumption:** 3 people.
- **A:** Zcash/Rust (escrow, FROST, refunds).
- **B:** Frontend + ShieldRamp/Intents.
- **C:** Auditor, clearing engine, QA/threat model, demo.

## 4. Three-week build plan

### Week 1 (Oct 6–12): Foundations on NU7 testnet
- [ ] Repo, licence (MIT/Apache-2.0), CI. Pin Zebra/librustzcash/WebZjs versions that work after **NU7 testnet activation (Oct 6)**.
- [ ] Zebra + Zaino/lightwalletd running. Envoy gRPC-web proxy for the browser.
- [ ] FROST 2-of-3 key generation (DKG). Derive the escrow Ironwood UA. Prove one 2-of-3 PCZT spend on testnet. **(Critical-path spike: do it first.)**
- [ ] Memo schema v1 (`auction_id`, `bid_commitment`, `refund_ua`, `version`). ZIP-321 URI generator.
- [ ] Coordinator skeleton: create/open/close auction. Escrow scanner via IVK.
- [ ] ShieldRamp spike: 1Click quote → deposit → ZEC delivered to a WebZjs ephemeral wallet (small mainnet amount).
- **Exit criterion:** a bid paid from Zodl/zcash-devtool to the FROST escrow on testnet is detected and parsed by the coordinator.

### Week 2 (Oct 13–19): The core loop works end-to-end
- [ ] Uniform-price clearing engine (top-N bids clear at the N-th price; ties broken by a deterministic rule). Also first-price mode.
- [ ] Merkle tree of bids. Per-bidder inclusion receipts.
- [ ] **Public Auditor** CLI + WASM page: paste the IVK, recompute the result, compare with the published root.
- [ ] Refund engine v1: canonical denominations + random delay + batching. Trustees co-sign through the FROST PCZT flow.
- [ ] Next.js bid UI: auction page, countdown, QR/URI bid, receipt view.
- [ ] ShieldRamp v1: USDC/ETH → ZEC → auto-shield → delayed bid in denominations.
- **Exit criterion:** full testnet auction (≥ 20 bids from ≥ 5 wallets) → close → audited result → all refunds paid.

### Week 3 (Oct 20–26): Privacy hardening, polish, proof
- [ ] **Linkability Score** (shown before signing): amount-uniqueness vs recent Intents flow, time-gap risk, transparent-hop count, denomination fit.
- [ ] Linkability test harness: run a naive vs a sealForge auction and measure how many refunds a heuristic attacker can link to bids. Publish the chart.
- [ ] Commit-reveal mode (stretch; ship it if Week 2 finished on time).
- [ ] Embeddable widget package (`@sealforge/shieldramp`) + docs + a "run your own auction in 10 minutes" guide.
- [ ] Threat-model doc, architecture diagram, 3-minute demo video, live mainnet micro-auction (e.g. 10 lots, 0.01 ZEC minimum).
- [ ] Security review pass: no transparent hops in core flows, no address reuse, no PII in memos.
- **Buffer (Oct 27–28):** submission write-up, final video, open-source tag `v0.1.0`, submit before Oct 28 23:59 UTC.

## 5. MVP goal (definition of done)

1. **A live mainnet sealed-bid auction** with a FROST 2-of-3 escrow, ≥ 25 bids, settled and refunded.
2. **A public audit page.** Anyone can paste the published IVK and independently reproduce the clearing price, volume and winner set, matching the published Merkle root.
3. **Cross-chain bidding.** At least one bid placed with USDC or ETH via ShieldRamp, landing as a shielded bid.
4. **Measured privacy.** The harness shows refunds are not linkable to bids by amount/timing heuristics (target: under 5% linkable vs ~100% in the naive flow), and ShieldRamp shows a score before signing.
5. **Reusable kit.** Open-source Rust crate + auditor WASM + React widget, with a quickstart so another team could run an auction.

## 6. User experience

**Bidder (has a Zcash wallet):**
1. Opens an auction page: lot details, countdown, "price public after close, bidders never".
2. Enters a max bid → scans the QR (ZIP-321) with Zodl/Vizor/Noir. The memo and amount are prefilled. One tap to send.
3. Gets a **receipt** (bid commitment + auction ID). It is stored locally and can be downloaded.
4. After the close: opens **Verify** and sees the clearing price, total volume, and "your bid is included (Merkle proof ✓)".
5. A win is shown in the UI and the item is allocated. Refunds arrive as several shielded notes over the next hours, with a progress bar explaining why.

**Bidder (EVM/Solana, no Zcash wallet):**
1. Clicks **Bid with USDC**. Connects MetaMask/Phantom.
2. ShieldRamp shows a quote, the **Linkability Score** (e.g. "Medium: your USDC deposit is public; we split into 3 swaps and delay your bid 20–90 min") and the recommended amount rounding.
3. Signs. A private Zcash wallet is created in the browser (with a seed backup prompt). ZEC arrives, is shielded, and the bid is placed automatically after the delay.
4. Refunds go to the same private wallet. An optional "cash out to USDC" step comes with a clear privacy warning.

**Auction organiser:**
1. Runs `sealforge init` → configures lots, quantity, reserve price, duration, trustees.
2. Trustees run the FROST DKG (CLI or simple UI). The escrow address is generated.
3. Embeds the auction page/widget. At the close: one click publishes the result + IVK + Merkle root. Trustees co-sign the refund batches.

**Auditor / journalist:**
Opens the public audit page → pastes the IVK → watches the chain rescan → gets the same numbers as the organiser. No trust required.

## 7. Expected volume and ARR (estimates)

**Revenue model (post-hackathon):**
- **Protocol fee** on cleared volume (0.5–1%), charged to the organiser, not the bidders. zkSNARKs just moved royalties to 0% after backlash, so keep bidder-side fees at 0.
- **ShieldRamp routing fee** of 15–25 bps on routed volume. Use the Intents app-fee field if it is available; otherwise set it per integration.
- **Hosted/managed auctions** (trustee service, white-label page): $2–10K per launch.

**Assumptions:**
- ~$1,450/ZEC, as implied by the auction figures (25,305 ZEC ≈ $37M).
- The zkSNARKs auction cleared ~12,000 ZEC ≈ **$17M**.
- NEAR Intents ZEC volume of ~$268M per 30 days ≈ $3.2B/yr, about half of it inbound.

| Scenario | Auctions (yr 1) | Cleared volume | Fee rev. (0.75%) | ShieldRamp routed | Routing rev. (20 bps) | Managed launches | **ARR** |
|---|---|---|---|---|---|---|---|
| Conservative | 6 × $250K | $1.5M | $11K | $5M (~0.3% of inbound Intents ZEC) | $10K | 2 × $3K | **≈ $27K** |
| Base | 15 × $1M | $15M | $113K | $25M (~1.5%) | $50K | 6 × $5K | **≈ $193K** |
| Upside | 1 zkSNARKs-scale ($17M) + 25 × $2M | $67M | $503K | $80M (~5%) | $160K | 12 × $8K | **≈ $759K** |

**Hackathon-period targets (for the demo slide):**
- ≥ 1 mainnet auction.
- ≥ 25 bids.
- ≥ 3 cross-chain bids.
- 100% of results independently re-verified by at least one outside person (invite a judge/community member to run the auditor).


## 8. Submission checklist
- [ ] Public repo with licence, README (pitch, architecture diagram, **threat model**, known limitations), quickstart.
- [ ] Demo video ≤ 3 min: problem → live flow → privacy proof (audit page / linkability chart / issuer-can't-link demo).
- [ ] Mainnet evidence: txids (shielded, so show the viewing-key-based audit rather than raw data), live URLs.
- [ ] No transparent hops in core flows, no address reuse, no PII in memos. Self-audit before submitting.
- [ ] Shielded UA for the prize payout.
- [ ] Submit by **Oct 27** (one day early). Oct 28 23:59 UTC is the hard stop, and Zcon7 traffic will be heavy.
