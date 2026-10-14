# 1. Product Requirements Document (PRD): sealForge

**Purpose:** Define what the product must do and how success will be measured.

| | |
|---|---|
| Owner | Udoka (Udoka-AM) |
| Track | Zecathon **Private Markets** (+ Grand Prize) |
| Status | Approved for build, v0.1 |
| Build window | Oct 6 – Oct 26, 2026. Submission deadline Oct 28, 23:59 UTC |
| Related | [PRODUCT.md](PRODUCT.md), [02-TRD.md](02-TRD.md), [06-IMPLEMENTATION-PLAN.md](06-IMPLEMENTATION-PLAN.md) |

---

## 1.1 Product name and one-sentence idea

**sealForge**: open-source, auditable sealed-bid auctions on Zcash. Anyone can bid from any chain, and the clearing price is public while the bidders stay private.

**Problem solved:** today, private token or NFT sales either stay private and can't be audited (the September 2026 zkSNARKs auction: $36.94M submitted, nobody outside could verify the clearing price or volume), or they are auditable and expose every bidder (public on-chain auctions). Cross-chain bidders also leak their identity through public source-chain transfers. sealForge gives organisers an auction that anyone can audit and that keeps bidders private, with an honest, scored cross-chain on-ramp called **ShieldRamp**.

## 1.2 Target users

| User | Who they are | What they need |
|---|---|---|
| **Bidder (Zcash-native)** | Holds ZEC in a shielded wallet (Zodl, Vizor, Noir, Zashi, zcash-devtool) | Place a bid in one scan, without revealing identity, address or bid size to the public, and get a provable receipt |
| **Bidder (cross-chain)** | Holds USDC/ETH (later SOL) in MetaMask/Phantom and has no Zcash wallet | Bid without learning Zcash first, understand exactly how private the path is, and get refunds back privately |
| **Auction organiser** | NFT/collectible issuer, token project, community treasury, Zcash ecosystem team (e.g. zkSNARKs, Zilkroad) | Run a credible sale that buyers trust, without custodying funds alone, and get a provable public result |
| **Trustee** | Independent auditor, community signer, organiser's own signer | Co-sign escrow spends (refunds, payout) and never be able to move funds alone |
| **Auditor / journalist / judge** | Anyone sceptical of the result (ZachXBT-style investigators, press) | Recompute clearing price, volume and winners from the chain, without trusting the organiser |

## 1.3 Problem and current workaround

| Pain point | What users do today | Why it fails |
|---|---|---|
| Auction results can't be audited | Trust the organiser's spreadsheet / tweet (zkSNARKs, Sep 2026) | Accusations of inflated volume and unverifiable clearing price. Trust collapses after the fact |
| Bids are public on transparent chains | Bid from fresh wallets, use mixers, or don't bid | Fresh wallets still get linked through funding and timing. Mixers carry legal risk |
| Cross-chain bidders leak identity | Swap via NEAR Intents/Aurora straight into the auction address (~$19M across 1,718 swaps in Sep 2026) | Source-chain transfers are public, and amounts plus timing link the swap to the bid |
| Refunds re-identify bidders | Organiser sends one refund per bidder for `deposit − price` | Unique refund amounts and immediate timing link each refund to its bid |
| Single-custodian escrow | Organiser holds all bid funds in one wallet | One compromised key or rogue operator can take everything |

## 1.4 Goal and success measure

**Goal:** prove that a Zcash-native auction can be **private for bidders and fully auditable for everyone else**, and ship it as a reusable kit that any team can run.

| Outcome | Measurable signal | Target (by Oct 26) |
|---|---|---|
| Live, real auction | Mainnet auction settled with FROST 2-of-3 escrow | ≥ 1 auction, ≥ 25 bids, 100% refunds paid |
| Public auditability | Independent reruns of the auditor that match the published Merkle root | ≥ 1 outside person (judge/community) reproduces the result. 100% match |
| Cross-chain reach | Bids placed via ShieldRamp (USDC/ETH → shielded bid) | ≥ 3 cross-chain bids |
| Measured privacy | % of refunds a heuristic attacker links to bids (amount + timing) | < 5% with sealForge vs ~100% for the naive flow |
| Reusability | Time for a new team to run a testnet auction from the quickstart | ≤ 10 minutes |

## 1.5 Core features

| # | Feature | User benefit | Priority |
|---|---|---|---|
| F1 | **Auction Coordinator**: create uniform-price or first-price sealed-bid auctions, open, close, compute the result | Organisers run a sale with clear rules and a deterministic outcome | **P0** |
| F2 | **FROST 2-of-3 escrow** on an Ironwood address (organiser + auditor + community signer) | No single party can move bidder funds | **P0** |
| F3 | **Bid channel**: ZIP-321 URI/QR with memo `SF1:<auction_id>:<commitment>:<refund_ua>` | Bid from any Zcash wallet in one scan. Amount and memo are prefilled | **P0** |
| F4 | **Bid receipt**: commitment + auction ID + salt, stored locally and downloadable | Bidders can later prove their bid was counted | **P0** |
| F5 | **Public Auditor** (Rust CLI + WASM web page): IVK + params → clearing price, volume, winners, Merkle root | Anyone can recompute the result. No trust required | **P0** |
| F6 | **Merkle inclusion proofs** per bid | Each bidder verifies "my bid is included" | **P0** |
| F7 | **Refund Engine**: canonical denominations (1/2/5 × 10^k), random delays, cross-bidder batching, FROST co-signed PCZTs | Refunds can't be linked back to bids by amount or timing | **P0** |
| F8 | **ShieldRamp widget**: USDC/ETH → NEAR Intents 1Click → ephemeral in-browser Zcash wallet (WebZjs) → shield → delayed bid | Non-Zcash users can bid privately without installing a Zcash wallet | **P1** |
| F9 | **Linkability Score** shown before signing (amount uniqueness, time gap, transparent hops, denomination fit) | Users see residual risk honestly before committing funds | **P1** |
| F10 | **Linkability test harness** + published chart (naive vs sealForge) | Judges and users see the privacy claim measured | **P1** |
| F11 | **Organiser console** (web) + `sealforge` CLI (`init`, `dkg`, `close`, `publish`, `refund`) | Organisers and trustees operate auctions without hand-written scripts | **P1** |
| F12 | **Embeddable widget** `@sealforge/shieldramp` + "run your own auction in 10 minutes" guide | Other teams can reuse the kit | **P1** |
| F13 | **Commit-reveal mode** (bucketed budget deposit + `H(price ‖ salt)` memo, reveal after close) | Bids stay hidden even from trustees during the auction | **P2 (stretch)** |
| F14 | **Exit path**: refunds/winnings → USDC via Intents with an unshielding warning and score | Cross-chain bidders can cash out | **P2 (stretch)** |
| F15 | **Tor (Arti) broadcast** for refunds and bids from the ephemeral wallet | Network-level IP privacy | **P2 (stretch)** |
| F16 | SOL support in ShieldRamp (Solana wallet adapter) | Solana users can bid | **P2 (stretch)** |

## 1.6 Out of scope for version one

- Shielded assets (ZSAs) as the auctioned item. Ownership of the item stays issuer-side (as Zilkroad does it today) because ZSAs are not live.
- Multi-unit bids from one bidder in a single payment. One payment is one unit at one price. Bidders who want several units send several bids.
- Combinatorial, Dutch, English or continuous auctions.
- Fiat on-ramps and KYC flows.
- Native mobile apps. The web app is responsive and the bid itself happens in the user's existing mobile wallet.
- Hosted multi-tenant SaaS (accounts, billing, white-label theming). v1 is self-hosted per organiser.
- Protocol fees and the ShieldRamp routing fee. The revenue model stays documented but is switched off for the hackathon.
- Governance or trustee rotation after DKG. A new auction means a new DKG.
- Transparent (t-addr) bids. They are rejected in the UI and ignored by the scanner.

## 1.7 User stories

**Bidder (Zcash-native)**
- As a **Zcash bidder**, I want **to scan a QR that prefills the amount and memo**, so I can **place a bid in one tap without typing a memo by hand**.
- As a **Zcash bidder**, I want **a downloadable receipt with my commitment and salt**, so I can **later prove my bid was counted**.
- As a **Zcash bidder**, I want **to see "your bid is included (Merkle proof ✓)" after close**, so I can **trust the result without trusting the organiser**.
- As a **losing bidder**, I want **my refund in several shielded notes over a few hours**, so I can **get my ZEC back without the refund being linked to my bid**.

**Bidder (cross-chain)**
- As an **ETH/USDC holder**, I want **to bid with MetaMask through a quote I can read**, so I can **join without setting up a Zcash wallet first**.
- As a **cross-chain bidder**, I want **to see a Linkability Score with a plain explanation before signing**, so I can **decide whether the residual risk is acceptable**.
- As a **cross-chain bidder**, I want **to back up and export my ephemeral wallet seed**, so I can **keep my refunds and winnings if I close the tab**.

**Organiser and trustees**
- As an **organiser**, I want **to run `sealforge init` and configure lots, quantity, reserve, duration and trustees**, so I can **launch an auction in minutes**.
- As a **trustee**, I want **to co-sign refund batches with my FROST share**, so I can **guarantee that no single party moves funds**.
- As an **organiser**, I want **one action that publishes the result, IVK and Merkle root at close**, so I can **give the public everything needed to audit me**.

**Auditor**
- As an **auditor or journalist**, I want **to paste the published IVK and watch the auditor rescan the chain**, so I can **independently reproduce the clearing price, volume and winner set**.

## 1.8 Acceptance criteria

| ID | Given [starting state] | When [action] | Then [observable result] |
|---|---|---|---|
| AC1 | An organiser has a configured auction in `draft` and 3 trustees | the trustees complete the FROST DKG | an escrow Ironwood UA is derived, shown on the auction page, and the auction can move to `open` |
| AC2 | An auction is `open` | a bidder pays the ZIP-321 URI from a shielded wallet | within 2 blocks of confirmation the coordinator records the bid as `valid`, and the bid count on the page increments |
| AC3 | An auction is `open` | a payment arrives with a malformed memo, below the reserve, or for another auction ID | the bid is stored as `invalid` with a reason code, excluded from clearing, and marked for a full refund |
| AC4 | An auction is `open` and the close height is reached | the coordinator closes the book | no later payment is counted (`late` status, full refund), and the clearing result is computed deterministically |
| AC5 | A uniform-price auction with N units and M > N valid bids | clearing runs | the top N bids win, every winner pays the N-th highest bid, and ties are broken by (earlier block height, then lower leaf hash). With M ≤ N valid bids, every bidder wins and pays the lowest bid |
| AC6 | An auction is `closed` | the organiser publishes | the IVK, auction params, block range and Merkle root are public, and the audit page shows them |
| AC7 | A published auction | anyone runs the auditor CLI or WASM page with the IVK | the recomputed clearing price, volume, winners and Merkle root equal the published values byte-for-byte |
| AC8 | A bidder holds a receipt | they open Verify and load the receipt | the page shows a valid Merkle inclusion proof against the published root |
| AC9 | Refund amounts are computed | the refund engine plans refunds | every refund is split into canonical denominations, scheduled with random delays (default 10–240 min), and mixed into batches with other bidders' notes |
| AC10 | A refund batch PCZT is built | 2 of 3 trustees sign | the transaction finalises and broadcasts. With only 1 signature it can't be finalised |
| AC11 | A cross-chain bidder connects MetaMask | ShieldRamp fetches a quote | the quote, the Linkability Score (Low/Medium/High with reasons) and the recommended rounded amount appear before any signature is requested |
| AC12 | A cross-chain bidder has signed | ZEC arrives in the ephemeral wallet | it is shielded, and the bid is placed automatically after a random 20–90 min delay, with progress visible and resumable after a reload |
| AC13 | The linkability harness runs on the same bid set in naive and sealForge mode | the heuristic attacker runs | naive linkage is ≈ 100% and sealForge linkage is < 5%, and the chart is saved to `docs/evidence/` |
| AC14 | Any core flow (bid, refund, payout) | the security review checks transactions | there are no transparent hops, no address reuse and no PII in memos |

## 1.9 Open questions

| # | Question | Default if not decided by | Deadline |
|---|---|---|---|
| Q1 | Which librustzcash / Zebra / Zaino / WebZjs versions support Ironwood + NU7 testnet today? | Use the newest pre-releases that pass the M1 spike. Record them in `docs/VERSIONS.md` | Oct 8 |
| Q2 | Does `zcash-devtool` + `frost-client` support 2-of-3 PCZT signing for Ironwood spends, or does it still need Orchard-era tooling? | If not, fall back to a 2-of-3 flow built directly on `frost-rerandomized` + PCZT crates | Oct 9 (critical path) |
| Q3 | NEAR Intents 1Click: API key/JWT access, rate limits, minimum swap sizes, and is an app-fee parameter available? | Request access now. Demo with small mainnet amounts and no fee | Oct 10 |
| Q4 | Does the ZEC deposit side of Intents deliver to a shielded address, or is the leg transparent? | Assume transparent. Auto-shield in the ephemeral wallet and count it in the score | Oct 10 |
| Q5 | Who are the 3 trustees for the live mainnet auction (organiser, independent auditor, community signer)? | Udoka + 2 invited community members | Oct 17 |
| Q6 | What is the live demo auction (item, quantity, reserve)? | 10 lots of a commemorative "sealForge Zcon7" digital pass, 0.01 ZEC reserve | Oct 19 |
| Q7 | Refund remainder below the smallest denomination (0.0001 ZEC): add it to the smallest note, or require bids to be canonical amounts? | The UI rounds bids to canonical amounts and the remainder joins the smallest note | Oct 13 |
| Q8 | Who pays ZIP-317 fees for refunds and payout? | The organiser pre-funds a fee buffer into the escrow before `open` | Oct 13 |
| Q9 | Is commit-reveal (F13) in v1? | Only if the M3 exit criterion is met by Oct 19 | Oct 19 |
| Q10 | Domain and hosting account ownership (e.g. `sealforge.xyz`)? | Vercel default domain until purchased | Oct 20 |
| Q11 | Licence: dual MIT/Apache-2.0? | Yes (scaffold ships both) | Done |
