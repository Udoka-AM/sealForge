# 6. Implementation Plan: sealForge

**Purpose:** Give the AI agent an ordered build sequence with checkpoints.

**Owners:** **A** = Zcash/Rust (escrow, FROST, refunds). **B** = Frontend + ShieldRamp/Intents. **C** = Auditor, clearing engine, QA/threat model, demo. "Agent" = AI coding agent working under that owner's review.

**Rule before moving to the next milestone:** run the app, verify the milestone's acceptance criteria (IDs from [01-PRD.md §1.8](01-PRD.md#18-acceptance-criteria)), and record any unresolved issue as a GitHub issue labelled `carry-over`.

| Milestone | Dates (2026) | Exit check |
|---|---|---|
| M1 Project setup | Oct 6–9 | CI green. Node stack synced on NU7 testnet. **2-of-3 FROST PCZT spend proven on testnet** |
| M2 Data and auth | Oct 9–12 | Migrations applied. Signed-request auth. A testnet bid is detected and parsed (AC2, AC3) |
| M3 Core user journey | Oct 13–19 | Full testnet auction: ≥ 20 bids from ≥ 5 wallets → close → audited → all refunds paid (AC4–AC10) |
| M4 Secondary features | Oct 17–23 | ShieldRamp bid lands. Linkability Score. Harness chart. Widget package (AC11–AC13) |
| M5 Quality | Oct 21–25 | Security review (AC14), accessibility pass, error states, performance targets |
| M6 Release | Oct 24–28 | Live mainnet micro-auction settled. `v0.1.0` tagged. Submitted by Oct 27 |

---

## Milestone 1: Project setup (Oct 6–9)

| # | Task | Owner | Inputs | Output | Definition of done | Depends on |
|---|---|---|---|---|---|---|
| 1.1 | Repo scaffold, licences, CI | A + Agent | This doc set | Cargo workspace, pnpm workspace, `ci.yml` | ✅ Done in the initial commit. CI passes on `main` | — |
| 1.2 | Pin Zcash dependency versions | A | NU7 testnet activation, crates.io, Zebra/Zaino releases | `docs/VERSIONS.md`, Zcash crates uncommented in `Cargo.toml` | `cargo build` passes with the pinned librustzcash crates. Versions documented with reasons (PRD Q1) | 1.1 |
| 1.3 | Provision VM, run Zebra + Zaino + Envoy (testnet) | A | Hetzner account, `infra/docker-compose.yml` | Synced testnet node, gRPC + gRPC-web endpoints over TLS | `grpcurl` gets the latest block. WebZjs syncs a test wallet through Envoy | 1.1 |
| 1.4 | Start mainnet node sync (VM-2) | A | Same compose with `NETWORK=mainnet` | Syncing mainnet node | Sync started by Oct 8. Fully synced by Oct 12 | 1.3 |
| 1.5 | **Critical spike: FROST 2-of-3 DKG + PCZT spend on testnet** | A | `frost-client`/`frostd`, `zcash-devtool` PCZT flow | `crates/sealforge-escrow` spike + `docs/spikes/frost-pczt.md` | A testnet txid spending from a DKG-derived escrow with 2 of 3 shares. 1 share alone fails (PRD Q2). **If blocked by Oct 9 noon: escalate, fall back to direct `frost-rerandomized` + PCZT crates** | 1.2, 1.3 |
| 1.6 | Memo schema v1 + ZIP-321 URI generator | C + Agent | Schema in [05-BACKEND-SCHEMA.md §5.7](05-BACKEND-SCHEMA.md#57-data-validation) | `sealforge-core::memo`, `::zip321` with tests | Round-trip encode/decode tests. URIs open prefilled in Zodl/Zashi on testnet | 1.1 |
| 1.7 | ShieldRamp spike: 1Click quote → deposit → ZEC to WebZjs wallet | B | 1Click docs + JWT (PRD Q3), small mainnet funds | `apps/web/src/app/api/shieldramp/quote` route + spike notes | One real swap (≈ $5 USDC → ZEC) delivered to an ephemeral wallet. Findings on the transparent leg recorded (PRD Q4) | 1.3 |
| 1.8 | Frontend shell + design tokens | B + Agent | [04-UI-UX-BRIEF.md](04-UI-UX-BRIEF.md) | Layout, top bar, tokens, fonts, Home/Auction placeholders | `pnpm build` passes. Lighthouse a11y ≥ 95 on the shell | 1.1 |
| 1.9 | Vercel project + preview deploys | B | GitHub repo | Preview per PR, production on `main` | A PR shows a working preview URL | 1.8 |

## Milestone 2: Data and auth (Oct 9–12)

| # | Task | Owner | Inputs | Output | Definition of done | Depends on |
|---|---|---|---|---|---|---|
| 2.1 | Initial migration | A + Agent | §5.2 tables | `migrations/20261007000000_init.sql` (scaffolded) completed + `cargo sqlx prepare` data | `sqlx migrate run` on a clean DB. Seed loads. CI runs migrations against a Postgres service container | 1.1 |
| 2.2 | Signed-request auth (Ed25519 + nonces) | A + Agent | §5.5 | `coordinator::auth` extractor, organiser/trustee roles | Tests: valid sig passes, bad sig / old ts / replayed nonce rejected, wrong-auction trustee forbidden | 2.1 |
| 2.3 | Auction CRUD + lifecycle state machine | A + Agent | §5.2 lifecycle | `POST/GET/PATCH /v1/auctions`, transition endpoints | Illegal transitions return 409. Covered by tests | 2.2 |
| 2.4 | DKG relay endpoints + CLI `trustee join` / `dkg` | A | 1.5 spike | `/v1/auctions/{id}/dkg/*`, CLI commands | 3 CLI trustees complete DKG through the coordinator. Escrow UA is stored. AC1 | 1.5, 2.3 |
| 2.5 | Escrow scanner (IVK) | A | Zaino endpoint, `zcash_client_backend` | Scanner task writing `bids` + `scanner_state` | AC2 and AC3 pass on testnet. Restart resumes from `last_height` without duplicates | 1.6, 2.3 |
| 2.6 | `sealforge init` + organiser registration | A + Agent | §5.5 | CLI `init`, `auction new` | Fresh machine → registered organiser → draft auction in < 2 min | 2.2 |
| **Exit** | **A bid paid from Zodl/zcash-devtool to the FROST escrow on testnet is detected and parsed by the coordinator.** | | | | | |

## Milestone 3: Core user journey (Oct 13–19)

Ordered by dependency: core logic → API → UI → refunds.

| # | Task | Owner | Inputs | Output | Definition of done | Depends on |
|---|---|---|---|---|---|---|
| 3.1 | Clearing engine (uniform + first-price) | C + Agent | PRD AC5 | `sealforge-core::clearing` | Property tests: determinism, tie-break rule, reserve, N ≥ M edge cases. 20k bids < 1 s | 1.6 |
| 3.2 | Merkle tree + inclusion proofs + receipts | C + Agent | Leaf definition in `sealforge-core` | `sealforge-core::merkle`, `::receipt` | Proofs verify for every leaf in the fixtures. Tampered proofs fail | 3.1 |
| 3.3 | Close + publish pipeline | A | 2.5, 3.1, 3.2 | Auto-close at `close_height`, `clearing_results`, publish endpoint | AC4, AC6 | 3.1, 3.2 |
| 3.4 | Public Auditor CLI | C | 3.1, 3.2, Zaino | `sealforge-audit --ivk … --from … --to …` | AC7 on a testnet auction (byte-for-byte root match) | 3.3 |
| 3.5 | Auditor WASM + `/audit/{id}` page | C + B | 3.4 | `wasm-pack` build, S11 screen | AC7 in the browser. < 60 s for a 1-day range | 3.4, 1.8 |
| 3.6 | Bid UI: S2 → S3 → S4 → S5 | B + Agent | [03-APP-FLOW.md](03-APP-FLOW.md) §3.3–3.4 | Auction page, amount, QR, receipt | A testnet bid end-to-end from a phone wallet. Receipt saved and downloadable | 1.6, 2.5 |
| 3.7 | Verify page S10 + My bids S9 | B + Agent | 3.2 | Screens | AC8 | 3.3, 3.6 |
| 3.8 | Refund planner (denominations + delays) | A + C | ZIP 318 design, PRD Q7 | `sealforge-core::refunds` | AC9. Sum of notes = refund due for every fixture bid | 3.1 |
| 3.9 | Refund scheduler + PCZT batches + trustee signing | A | 1.5, 3.8 | Scheduler, `/v1/batches/*`, CLI `trustee sign` | AC10. Batches mix ≥ 3 bidders' notes when available | 3.8, 2.4 |
| 3.10 | Organiser console S13–S15 + trustee page S16 | B + Agent | 2.3, 3.9 | Screens | An organiser runs the whole lifecycle from the UI | 3.3, 3.9 |
| 3.11 | Payout to organiser | A | 3.9 | Payout batch after refunds | Status reaches `settled` | 3.9 |
| **Exit** | **Full testnet auction (≥ 20 bids from ≥ 5 wallets) → close → audited result → all refunds paid.** | | | | | |

## Milestone 4: Secondary features (Oct 17–23)

| # | Task | Owner | Inputs | Output | Definition of done | Depends on |
|---|---|---|---|---|---|---|
| 4.1 | ShieldRamp v1 (USDC/ETH → shielded → delayed bid) | B | 1.7, 3.6 | `packages/shieldramp` flow S6–S8 | AC12. One real mainnet cross-chain bid lands as a shielded bid | 1.7, 3.6 |
| 4.2 | Linkability Score | B + C | Intents flow stats, score factors | `packages/shieldramp/src/score.ts` + card UI | AC11. Unit tests per factor. Copy reviewed for honesty | 4.1 |
| 4.3 | Linkability test harness + chart | C | 3.9 refunds, fixtures | `tools/linkability-harness`, `docs/evidence/linkability.svg` | AC13 (< 5% vs ~100%) | 3.9 |
| 4.4 | Embeddable widget build (`@sealforge/shieldramp`) | B + Agent | 4.1 | ESM + UMD bundles, `<script>` embed | Embed works in a plain HTML page | 4.1 |
| 4.5 | "Run your own auction in 10 minutes" guide | C | M3 | `docs/QUICKSTART.md` | An outside tester finishes in ≤ 10 min | M3 |
| 4.6 | Commit-reveal mode (stretch, only if M3 exited by Oct 19) | A + C | PRD F13 | `bid_mode = commit_reveal` | Reveal flow tested on testnet | M3 |
| 4.7 | Ephemeral wallet page S12 (export, send out) | B | 4.1 | Screen | Seed export + re-import works | 4.1 |

## Milestone 5: Quality (Oct 21–25)

| # | Task | Owner | Inputs | Output | Definition of done | Depends on |
|---|---|---|---|---|---|---|
| 5.1 | Security review | C + A | TRD §2.7, PRD AC14 | `docs/SECURITY-REVIEW.md` | No transparent hops, no address reuse, no PII in memos, secrets audit, dependency audit (`cargo audit`, `pnpm audit`) clean or justified | M3, M4 |
| 5.2 | Threat model doc + README section | C | PRODUCT.md threat model | `docs/THREAT-MODEL.md` | Every adversary row has a test or evidence link | 4.3 |
| 5.3 | Error, empty, offline states | B + Agent | App Flow §3.7 | All states implemented | Each state reproduced manually and screenshotted | M3 |
| 5.4 | Accessibility pass | B | Brief §4.8 | Fixes | axe: 0 serious issues. Keyboard-only bid flow works. Lighthouse a11y ≥ 95 | 5.3 |
| 5.5 | Performance pass | A + B | TRD §2.8 | Measurements in `docs/evidence/perf.md` | All targets met or documented | M3 |
| 5.6 | Backup + restore drill | A | TRD §2.8 | Runbook in `infra/README.md` | Restore from last night's dump on a scratch DB | 1.4 |

## Milestone 6: Release (Oct 24–28)

| # | Task | Owner | Inputs | Output | Definition of done | Depends on |
|---|---|---|---|---|---|---|
| 6.1 | Test plan run (staging, testnet) | C | All ACs | `docs/evidence/test-run.md` | AC1–AC14 checked with evidence links | M5 |
| 6.2 | Mainnet deploy | A | Release image, VM-2 synced | Production coordinator + Vercel prod | Health checks green. Monitoring on | 6.1 |
| 6.3 | Live mainnet micro-auction | All | PRD Q5, Q6 | Settled auction | ≥ 25 bids, ≥ 3 via ShieldRamp, all refunds confirmed | 6.2 |
| 6.4 | Outside re-verification | C | Published IVK | Statement or screenshot from an outside auditor | ≥ 1 independent match | 6.3 |
| 6.5 | Demo video + submission write-up | C | Evidence | ≤ 3-min video, submission text | Reviewed by all owners | 6.3 |
| 6.6 | Tag `v0.1.0` + release artefacts | A | `main` | GitHub release (CLI binaries, audit page bundle) | Release published | 6.5 |
| 6.7 | Submit | Udoka | Shielded UA for prize | Submission | Submitted **by Oct 27** (hard stop Oct 28 23:59 UTC) | 6.6 |

**Rollback:** frontend via Vercel instant rollback to the previous production deployment. Coordinator via `docker compose` pinned to the previous image tag (`SF_IMAGE_TAG`). The DB is forward-only, so each release must be backward compatible with the previous image. Funds can't be "rolled back": if refunds misbehave, pause the scheduler (`SF_REFUNDS_PAUSED=true`). Trustees refuse to sign until fixed.

**Monitoring:** coordinator `/healthz` and `/readyz` (DB, Zaino, scanner lag) checked every minute by an uptime monitor (Better Stack free tier). Scanner lag > 4 blocks and failed batches alert to the team Telegram. Structured logs (`tracing` JSON) with 7-day retention. Vercel analytics off on bid routes.
