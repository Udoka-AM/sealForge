# Agent guide: sealForge

Read before writing code:
1. `docs/06-IMPLEMENTATION-PLAN.md`: pick the next task in milestone order. Respect dependencies.
2. `docs/01-PRD.md` §1.8: the acceptance criteria your task must satisfy.
3. `docs/02-TRD.md` and `docs/05-BACKEND-SCHEMA.md`: stack, data model and privacy rules. Don't invent alternatives. If something is missing, add it to the doc in the same PR.

## Non-negotiable privacy rules
- No transparent hops in bid, refund or payout flows. No address reuse. No PII in memos.
- Never log client IPs, refund addresses, commitments, salts or seeds.
- Receipt salts and ephemeral wallet seeds never leave the browser.
- `sealforge-core` stays pure: no I/O, no randomness, no clocks. The coordinator, CLI and WASM auditor must compute identical results.
- Copy never claims "untraceable" or "anonymous". Describe residual risk honestly.

## Commands
- Rust: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`
- WASM check: `cargo check -p sealforge-auditor --target wasm32-unknown-unknown --no-default-features --features wasm`
- JS: `pnpm lint`, `pnpm typecheck`, `pnpm test`, `pnpm build`, `pnpm dev`
- DB: `docker compose -f infra/docker-compose.yml up -d postgres`. Migrations run when the coordinator starts. New migration = new timestamped file in `migrations/` (forward-only).

## Conventions
- Amounts are integer zatoshi (`u64` / `BIGINT` / `bigint`). Never floats.
- Placeholder code is marked `TODO(<task id>)`. Remove the marker when the task is done.
- One PR per task where practical. Fill in the PR template's privacy checklist.
- Before closing a milestone: run the app, verify its ACs, open `carry-over` issues for anything unresolved.
