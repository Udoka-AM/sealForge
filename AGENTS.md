# Guide for coding agents

This file gives automated contributors (and people) the context they need to change sealForge safely. [CONTRIBUTING.md](CONTRIBUTING.md) is the full contributor guide; everything there applies here too.

## Where things are

- `crates/sealforge-core`: auction rules, memos, commitments, clearing, Merkle receipts, refund policy. Pure.
- `crates/sealforge-escrow`, `sealforge-chain`, `sealforge-verify`: FROST key generation and signing, escrow wallets and PCZTs, and the checks trustees run before signing.
- `crates/sealforge-coordinator`: the API, chain scanner, closer and refund and payout worker.
- `crates/sealforge-auditor`, `sealforge-scan`: recompute a published result from the chain (CLI and WebAssembly).
- `crates/sealforge-cli`: the organiser and trustee command line. `apps/web`: the Next.js site.
- Specification and security model: `docs/site` (published at docs.sealforge.auction) and `docs/THREAT-MODEL.md`. Data model: `docs/05-BACKEND-SCHEMA.md`.

## Rules that are never relaxed

- No transparent hops in bid, refund or payout flows. No address reuse. No personal data in memos.
- Never log client IPs, refund addresses, commitments, salts or seeds.
- Receipt salts and wallet seeds never leave the browser.
- `sealforge-core` stays pure: no I/O, no randomness, no clocks. The coordinator, the CLI and the WebAssembly auditor must compute identical results.
- Text for users never claims "untraceable" or "anonymous". State the residual risk.
- Amounts are integer zatoshi (`u64`, `BIGINT`, `bigint`), never floating point.

## Commands

- Rust: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (integration tests need `DATABASE_URL` and the compose Postgres).
- Dependency policy: `cargo deny check`.
- WebAssembly modules: `scripts/build-wasm.sh` rebuilds all four in the pinned container; commit the result when an included crate changes.
- Web: `pnpm lint`, `pnpm typecheck`, `pnpm test`, `pnpm build`.
- Database: `docker compose -f infra/docker-compose.yml up -d postgres`. Migrations run when the coordinator starts; a new migration is a new timestamped file in `migrations/` (forward-only).

## Working conventions

- One change per pull request, with docs, the threat model and `CHANGELOG.md` updated in the same PR when behaviour changes.
- A placeholder in code names its tracking issue: `TODO(#123)`.
- Changes that affect the chain (escrow spends, refunds, payouts) are verified on a local regtest chain (`infra/regtest`) and the run recorded in `docs/evidence`.
