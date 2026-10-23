<p>
  <img src="apps/web/src/app/icon.svg" alt="" width="56" height="56">
</p>

# sealForge

**Live on Zcash testnet: [sealforge.udokaam.dev](https://sealforge.udokaam.dev)** · Release: [v0.1.0](https://github.com/Udoka-AM/sealForge/releases/tag/v0.1.0)

**Sealed-bid auctions on Zcash. The clearing price is public. The bidders aren't. And you don't have to trust us: anyone can recompute the result from the chain.**

sealForge is an open-source kit for running private, verifiable sales (tickets, passes, NFTs, allocations) with shielded ZEC.

| What happens | How |
|---|---|
| **You bid privately** | Scan one ZIP-321 QR with any Zcash wallet. The payment is a shielded Ironwood transfer, and its memo carries a salted commitment and a fresh refund address. Your receipt stays in your browser |
| **Or bid from the built-in wallet** | Create a shielded wallet in the page, or link the one you already use by importing its 24-word phrase (Zashi, Zodl and other ZIP 32 wallets). It syncs, proves and signs in a Web Worker with librustzcash; the phrase never leaves the browser. Paying a bid or a reveal is one tap |
| **Nobody can run off with the money** | Bids go to an escrow whose spend authority is split across three trustees with **FROST 2-of-3**. One trustee alone can't move funds; the coordinator can't move funds at all |
| **Optional: nobody sees bids early** | In **sealed-reveal** mode you deposit a standard budget and reveal your bid after the close. Until then the coordinator, the trustees and the public see only budgets |
| **Anyone can check the result** | After close, the escrow's incoming viewing key and a Merkle root of every bid are published. The Audit page (in the browser) or `sealforge-audit` (CLI) rescans the chain with that key, through any lightwallet server, and recomputes price, volume, winners, each bidder's outcome and the root |
| **Refunds can't be traced back to bids** | Refunds are split into standard denominations, sent after random delays, and batched with other bidders' refunds. Measured: **0.1%** of refunds linkable at the exit, against 100% for naive refunds |

Built for the [Zecathon](https://thezecathon.com) **Private Markets** track (Oct 6–28, 2026).

## See it work

Everything below ran on real Zcash chains with real shielded transactions and real FROST signatures: public **testnet**, and a local regtest chain (Zebra with Ironwood active, Zaino).

| Claim | Evidence |
|---|---|
| **The first auction on public testnet**, live at [sealforge.udokaam.dev](https://sealforge.udokaam.dev/audit/KE5DFF4PDA): 7 sealed bids, reveals, uniform price, 2 refund transactions and a payout, each signed by a different pair of trustees, settled, with the chain audit matching from the CLI and in the browser | [docs/evidence/testnet-genesis.md](docs/evidence/testnet-genesis.md) |
| **25 sealed bids on testnet**, settled, plus the test plan run: 12 of 14 acceptance criteria pass, and the two ShieldRamp criteria aren't built | [docs/evidence/test-run.md](docs/evidence/test-run.md) |
| **The whole bidder journey with the built-in wallet** on testnet: restore, pay the deposit, reveal, win, receive the refund at the bid's own address | [docs/WALLET-INTEGRATION.md](docs/WALLET-INTEGRATION.md) |
| A whole auction: bids → auto-close → publish → 2-of-3 signed refunds → payout → settled | [docs/evidence/regtest-e2e.md](docs/evidence/regtest-e2e.md) |
| The same lifecycle driven from the browser console, with a trustee signing in the browser (FROST in WebAssembly) | [regtest-e2e.md › console](docs/evidence/regtest-e2e.md#the-whole-lifecycle-from-the-console-task-310) |
| Sealed bids: only budgets visible while bidding was open; reveals after close; audit match | [regtest-e2e.md › sealed bids](docs/evidence/regtest-e2e.md#sealed-bids-commit-reveal-task-46) |
| Independent audit from the chain, and a tampered result caught | [docs/evidence/audit-rescan.md](docs/evidence/audit-rescan.md) |
| Refund unlinkability, measured | [docs/evidence/linkability.md](docs/evidence/linkability.md) |
| Security review (AC14), with the bugs it found and fixed | [docs/SECURITY-REVIEW.md](docs/SECURITY-REVIEW.md) |
| Performance against targets (20k-bid close in 13 ms) | [docs/evidence/perf.md](docs/evidence/perf.md) |
| Accessibility (axe, every page) and backup/restore drill | [a11y.md](docs/evidence/a11y.md), [backup-drill.md](docs/evidence/backup-drill.md) |

## How an auction runs

1. **Create.** The organiser creates the auction (CLI or the browser console) and sends three trustees a one-time invite.
2. **Key generation.** Each trustee runs a FROST DKG on their own machine. The coordinator only relays packages; key shares never leave the trustees. The escrow address is derived from the group key.
3. **Bid.** Bidders pay the escrow from any Zcash wallet. The scanner records each payment from the shielded memo.
4. **Close and publish.** At the close height (plus 10 confirmations) the result is computed with the same code the auditor runs. Publishing releases the viewing key and the Merkle root.
5. **Refund and pay out.** Refund notes are batched across bidders; any two trustees co-sign each batch (CLI or browser). After the last refund, the rest of the escrow is swept to the organiser.

## Verify it yourself

```bash
# Rescan a published auction from the chain, with nothing but its public result and a lightwallet server:
cargo run --release -p sealforge-auditor -- \
  --published https://<coordinator>/v1/auctions/<CODE>/result --endpoint https://<lightwalletd>
```

Bidders check their own bid at `/verify` by dropping their receipt file: inclusion proof, outcome and refund plan, computed in the browser.

## The web app

| Page | What it does |
|---|---|
| `/` | The story in five scenes, from real coordinator data: live auctions, a published clearing price, a real bid memo read field by field, and real escrow notes decoded with the published viewing key |
| `/a/<code>` | An auction: phase track, a block clock counting down the bidding or reveal window, rules, what stays private, and the published result |
| `/a/<code>/bid` | The bid flow: amount (or hidden bid and budget), refund address, then pay by QR or from the built-in wallet |
| `/me` | My bids: each bid's life from on-chain evidence only (paid, included, won or lost, refunded, settled) |
| `/verify` | Drop a receipt: inclusion proof, outcome and refund plan, computed in the browser |
| `/audit/<code>` | Rescan the chain in the browser with the published viewing key and compare every figure |
| `/wallet` | Create a wallet, link an existing one, or keep yours and pay by QR. Balance, sync, funding, activity |
| `/console`, `/trustee/<code>` | Organiser console and trustee signing (FROST in WebAssembly) |
| `/how-it-works` | The flow and the threat model in plain words |

Design: a dark, gold-accented system on IBM Plex, one 12-column grid, and motion that carries meaning (blocks ticking, seals stamping, proofs resolving). Motion is built with [Motion](https://motion.dev) and a plain 2D canvas, pauses off-screen, and every scroll scene has a still version for `prefers-reduced-motion`. Figures on the site come from the coordinator or the chain; the two illustrations are labelled.

To run the whole thing locally, on a private chain: [docs/QUICKSTART.md](docs/QUICKSTART.md) and [infra/regtest/README.md](infra/regtest/README.md). To host it on a server: [docs/DEPLOY-VPS.md](docs/DEPLOY-VPS.md).

## Threat model (summary)

| Adversary | Protected? | How / caveat |
|---|---|---|
| Public chain observer, during the auction | ✅ | Bids are shielded transfers. Amounts, senders and memos are encrypted |
| Public chain observer, after close | ✅ bidders hidden | The published viewing key reveals incoming amounts and memos, never senders. Refund addresses are fresh |
| Coordinator / trustees during bidding | ⚠️ in deposit mode · ✅ in sealed-reveal mode | Deposit mode: escrow viewers see bid amounts live (stated on the auction page). Sealed-reveal: they see only budget deposits until bidders reveal |
| A rogue trustee | ✅ | FROST 2-of-3: one share can't sign |
| A dishonest coordinator | ✅ detectable | It can't move funds, and any result that doesn't follow from the chain fails the public audit |
| Refund correlation (amount + timing) | ✅ mitigated | Standard denominations, random delays, cross-bidder batches ([evidence](docs/evidence/linkability.md)) |

Full version, including what is **not** protected: [docs/THREAT-MODEL.md](docs/THREAT-MODEL.md).

## Status

| Done | Not yet |
|---|---|
| FROST 2-of-3 escrow, on-chain spends · bid flow and receipts · scanner · close/publish · Merkle receipts · chain auditor in the CLI and in the browser (WASM, any gRPC-web lightwallet server) · unlinkable refunds and payout · organiser console · browser trustee signing · sealed-reveal mode · built-in shielded wallet (create or link by phrase) · embeddable auction card (`<script src=…/embed.js data-auction=CODE>`) · security, accessibility, performance, backups · **public testnet runs** (7 and 25 bids, settled) · hosted instance · release v0.1.0 | **Mainnet** (no mainnet ZEC to fund bidders; the code supports it) · **ShieldRamp** cross-chain bidding via NEAR Intents · an outside re-verification of a published auction |

## Architecture

```
Browser (apps/web: Next.js)
  bid flow · receipts · verify · audit (WASM) · organiser console · trustee signing (WASM)
  built-in wallet (WASM in a Web Worker; syncs from any gRPC-web lightwallet server)
        │ HTTPS (same-origin proxy; organiser/trustee requests Ed25519-signed in the browser)
        ▼
  sealforge-coordinator ── Zaino ── Zebra (Ironwood)
  (axum API, escrow scanner,       ▲
   closer, refund + payout worker) │ gRPC
        │                          │
    PostgreSQL          sealforge CLI × 3 trustees (FROST shares stay local)
                        sealforge-audit (anyone, any lightwallet server)
```

## Repository layout

```
apps/web/                      Next.js 16: home, auctions, bid flow, my bids, verify, audit, wallet, console, trustee page
crates/sealforge-core/         Pure, deterministic logic: memos, ZIP-321, selection, clearing, Merkle, refunds
crates/sealforge-coordinator/  axum API, escrow scanner, closer, refund/payout worker
crates/sealforge-chain/        Zcash wallets: escrow scanning, PCZT escrow spends, independent rescan
crates/sealforge-escrow/       FROST DKG + rerandomized signing of Ironwood spends
crates/sealforge-signer-wasm/  Trustee signing for the browser
crates/sealforge-auditor/      Auditor library, `sealforge-audit` CLI, WASM build
crates/sealforge-scan/         Find and decrypt escrow payments with only the viewing key (native and WASM)
crates/sealforge-wallet/       The bidder's in-browser wallet: keys, sync, Ironwood payments (librustzcash)
crates/sealforge-api/          Shared API types and the SF-Sig request signing scheme
crates/sealforge-cli/          `sealforge` CLI for organisers and trustees
packages/bid/                  @sealforge/bid: client-side commitments, memos, receipts
migrations/                    Postgres migrations (sqlx)
scripts/                       Local regtest chain, WASM builds, backup and restore drill
tools/linkability-harness/     Naive vs sealForge refund linkability measurement
docs/                          Product, PRD, TRD, app flow, UI brief, schema, plan, evidence
```

## Development

Requirements: Rust 1.99 (pinned in `rust-toolchain.toml`), Node 22, pnpm 9, Docker.

```bash
git clone https://github.com/Udoka-AM/sealForge && cd sealForge
cp .env.example .env
pnpm install
docker compose -f infra/docker-compose.yml up -d postgres
cargo run -p sealforge-coordinator   # applies migrations, serves :8080
pnpm dev                             # web on :3000
```

The web app reads three settings at runtime. Without `LIGHTWALLETD_GRPCWEB` the wallet page explains that this deployment can't run the wallet, and the bid flow offers QR payment only.

| Variable | Example | Used for |
|---|---|---|
| `COORDINATOR_URL` | `http://127.0.0.1:8080` | The coordinator API, proxied same-origin |
| `LIGHTWALLETD_GRPCWEB` | `https://<envoy in front of lightwalletd>` | The built-in wallet's sync and broadcast |
| `NEXT_PUBLIC_DEFAULT_NETWORK` | `testnet` | Network labels and address checks |

Checks run in CI:

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
pnpm lint && pnpm typecheck && pnpm test && pnpm build
```

## Docs

| | |
|---|---|
| [Product description](docs/PRODUCT.md) | [PRD](docs/01-PRD.md) |
| [TRD](docs/02-TRD.md) | [App flow](docs/03-APP-FLOW.md) |
| [UI/UX brief](docs/04-UI-UX-BRIEF.md) | [Backend schema](docs/05-BACKEND-SCHEMA.md) |
| [Implementation plan](docs/06-IMPLEMENTATION-PLAN.md) | [Pinned versions](docs/VERSIONS.md) |
| [Threat model](docs/THREAT-MODEL.md) | [Security review](docs/SECURITY-REVIEW.md) |
| [Deploy on a VPS](docs/DEPLOY-VPS.md) | [Wallet integration](docs/WALLET-INTEGRATION.md) |

## Licence

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
