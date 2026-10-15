# sealForge

**Auditable sealed-bid auctions on Zcash. Bid from any chain. The clearing price is public, and the bidders aren't.**

sealForge is an open-source kit for running private, verifiable sales:

- **FROST 2-of-3 escrow.** Bids go into an Ironwood shielded address whose spend authority is split across three trustees. No single party can move funds.
- **One-scan bidding.** Any Zcash wallet pays a ZIP-321 QR whose memo carries the auction ID, a bid commitment and a fresh refund address.
- **Public audit.** After close, the escrow's incoming viewing key and a Merkle root of all bids are published. Anyone can rescan the chain (CLI or in-browser WASM) and recompute the clearing price, volume and winners.
- **Unlinkable refunds.** Refunds are split into canonical denominations, delayed at random and batched across bidders.
- **ShieldRamp.** Bid with USDC or ETH through NEAR Intents into an ephemeral in-browser Zcash wallet, with a **Linkability Score** shown before you sign.

Built for the [Zecathon](https://thezecathon.com) **Private Markets** track (Oct 6–28, 2026).

> **Status:** in active build. Core auction logic, the FROST 2-of-3 escrow, close/publish, receipts, the WASM auditor and refund planning work end to end locally. On-chain scanning and refund signing follow once the testnet node is up. Run your own: [docs/QUICKSTART.md](docs/QUICKSTART.md).

## Architecture

```
Browser (apps/web: Next.js)                         NEAR Intents 1Click
  bid flow · receipts · verify · organiser console        ▲
  ShieldRamp (wagmi + WebZjs) · auditor (WASM)            │ server route
        │ HTTPS                │ gRPC-web                 │
        ▼                      ▼                          │
  sealforge-coordinator ──── Envoy ── Zaino ── Zebra ◀────┘
  (axum: API, IVK scanner,                ▲
   refund scheduler)                      │
        │                                 │
    PostgreSQL              sealforge CLI × 3 trustees (FROST shares stay local)
```

Details: [docs/02-TRD.md §2.6](docs/02-TRD.md#26-architecture).

## Threat model (summary)

| Adversary | Protected? | How / caveat |
|---|---|---|
| Public chain observer, during the auction | ✅ | All bids are shielded transfers. Amounts, senders and memos are encrypted |
| Public chain observer, after close | ✅ amounts revealed / ✅ bidders hidden | The published IVK reveals incoming notes (amount, memo), not senders. Refund addresses are fresh |
| Auction operator / trustees | ⚠️ Partial | IVK holders can see bids live. Commit-reveal mode (stretch) fixes this. The MVP documents the caveat |
| Source-chain observer (ShieldRamp) | ⚠️ Reduced, not eliminated | EVM deposits are public. Canonical amounts, split swaps, delays and an ephemeral wallet reduce linkage, and the score quantifies what's left. Never "untraceable" |
| Single rogue trustee | ✅ | FROST 2-of-3 |
| Refund correlation (amount + timing) | ✅ Mitigated | Canonical notes, random delays and cross-bidder batching. Measured: 0.1% of refunds linkable at the exit vs 100% for naive refunds ([evidence](docs/evidence/linkability.md)) |

Full version, with where each protection is enforced and the known limits: [docs/THREAT-MODEL.md](docs/THREAT-MODEL.md).

## Repository layout

```
apps/web/                  Next.js 16 app: auctions, bid flow, verify, audit, console
packages/shieldramp/       @sealforge/shieldramp: cross-chain bidding + Linkability Score
crates/sealforge-core/     Pure, deterministic logic: memo, ZIP-321, clearing, Merkle, denominations
crates/sealforge-coordinator/  axum API, escrow scanner, refund scheduler
crates/sealforge-escrow/   FROST DKG + PCZT signing
crates/sealforge-auditor/  Auditor library, `sealforge-audit` CLI, WASM build
crates/sealforge-cli/      `sealforge` CLI for organisers and trustees
migrations/                Postgres migrations (sqlx)
seed/                      Dev/staging seed data
infra/                     docker compose (Postgres, Zebra, Zaino, Envoy, coordinator, Caddy)
tools/linkability-harness/ Naive vs sealForge refund linkability measurement
docs/                      Product description, PRD, TRD, app flow, UI brief, schema, plan
```

## Quickstart (development)

Requirements: Rust 1.89 (via `rust-toolchain.toml`), Node 22, pnpm 9, Docker.

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

## Licence

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
