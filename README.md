# sealForge

**Sealed-bid auctions on Zcash. The clearing price is public. The bidders aren't. And you don't have to trust us: anyone can recompute the result from the chain.**

sealForge is an open-source kit for running private, verifiable sales (tickets, passes, NFTs, allocations) with shielded ZEC.

| What happens | How |
|---|---|
| **You bid privately** | Scan one ZIP-321 QR with any Zcash wallet. The payment is a shielded Ironwood transfer, and its memo carries a salted commitment and a fresh refund address. Your receipt stays in your browser |
| **Nobody can run off with the money** | Bids go to an escrow whose spend authority is split across three trustees with **FROST 2-of-3**. One trustee alone can't move funds; the coordinator can't move funds at all |
| **Optional: nobody sees bids early** | In **sealed-reveal** mode you deposit a standard budget and reveal your bid after the close. Until then the coordinator, the trustees and the public see only budgets |
| **Anyone can check the result** | After close, the escrow's incoming viewing key and a Merkle root of every bid are published. `sealforge-audit` rescans the chain with that key, through any lightwallet server, and recomputes price, volume, winners, each bidder's outcome and the root |
| **Refunds can't be traced back to bids** | Refunds are split into standard denominations, sent after random delays, and batched with other bidders' refunds. Measured: **0.1%** of refunds linkable at the exit, against 100% for naive refunds |

Built for the [Zecathon](https://thezecathon.com) **Private Markets** track (Oct 6–28, 2026).

## See it work

Everything below ran on a real (local) Zcash chain: Zebra with Ironwood active, Zaino, real shielded transactions, real FROST signatures.

| Claim | Evidence |
|---|---|
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

To run the whole thing locally, on a private chain: [docs/QUICKSTART.md](docs/QUICKSTART.md) and [infra/regtest/README.md](infra/regtest/README.md).

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

| Done | Waiting on |
|---|---|
| FROST 2-of-3 escrow, on-chain spends · bid flow and receipts · scanner · close/publish · Merkle receipts · chain auditor (CLI) and published-list auditor (WASM) · unlinkable refunds and payout · organiser console · browser trustee signing · sealed-reveal mode · security, accessibility, performance, backups | **Testnet and mainnet runs** (hosted node) · **ShieldRamp** cross-chain bidding via NEAR Intents (API access) · the live mainnet micro-auction |

## Architecture

```
Browser (apps/web: Next.js)
  bid flow · receipts · verify · audit (WASM) · organiser console · trustee signing (WASM)
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
apps/web/                      Next.js 16: auctions, bid flow, verify, audit, console, trustee page
crates/sealforge-core/         Pure, deterministic logic: memos, ZIP-321, selection, clearing, Merkle, refunds
crates/sealforge-coordinator/  axum API, escrow scanner, closer, refund/payout worker
crates/sealforge-chain/        Zcash wallets: escrow scanning, PCZT escrow spends, independent rescan
crates/sealforge-escrow/       FROST DKG + rerandomized signing of Ironwood spends
crates/sealforge-signer-wasm/  Trustee signing for the browser
crates/sealforge-auditor/      Auditor library, `sealforge-audit` CLI, WASM build
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

## Licence

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
