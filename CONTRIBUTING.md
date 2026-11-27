# Contributing to sealForge

Thanks for helping. sealForge holds bidders' funds in a threshold escrow, so changes are held to the standard of code that moves money: small, tested, documented, and reviewed.

**Security issues** go through the private channel in [SECURITY.md](SECURITY.md), never a public issue or pull request.

## Before you start

- For anything beyond a small fix, open an issue first so the approach can be agreed before you write it.
- Read [docs/THREAT-MODEL.md](docs/THREAT-MODEL.md) and the [protocol specification](https://sealforge.mintlify.app) for the part you're changing. The privacy rules below are not negotiable.

## Development setup

Requirements: Rust (the version in `rust-toolchain.toml`), Node (`.nvmrc`), pnpm 9 and Docker.

```bash
pnpm install
docker compose -f infra/docker-compose.yml up -d postgres
cargo run -p sealforge-coordinator   # applies migrations, serves :8080
pnpm dev                             # web app on :3000
```

To run whole auctions against a private Zcash chain, see [infra/regtest/README.md](infra/regtest/README.md).

## Checks

Everything CI runs, locally:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
DATABASE_URL=postgres://sealforge:sealforge@localhost:5432/sealforge cargo test --workspace
cargo deny check
pnpm lint && pnpm typecheck && pnpm test && pnpm build
```

The browser runs four WebAssembly modules (auditor, signer, scanner, wallet) that are committed to `apps/web/public/`. If you change any crate they include, rebuild them in the pinned container and commit the result: `scripts/build-wasm.sh`. CI rebuilds them and fails unless the committed bytes match.

## Privacy and correctness rules

- No transparent hops in bid, refund or payout flows. No address reuse. No personal data in memos.
- Never log client IPs, refund addresses, commitments, salts or seeds.
- Receipt salts and wallet seeds never leave the browser.
- `sealforge-core` stays pure: no I/O, no randomness, no clocks. The coordinator, the CLI and the WebAssembly auditor must compute identical results.
- Amounts are integer zatoshi (`u64`, `BIGINT`, `bigint`), never floating point.
- Database migrations are forward-only: add a new timestamped file in `migrations/`.
- User-facing text never claims "anonymous" or "untraceable"; it states what stays private and what doesn't.

## Pull requests

1. Branch from `main` (`feat/…`, `fix/…`, `docs/…`, `chore/…`).
2. Keep each PR to one change. Update the docs, the threat model and `CHANGELOG.md` in the same PR when behaviour changes.
3. Write commit messages that say what changed and why, in the imperative ("Refuse a payout that pays the fee twice").
4. Fill in the pull request template, including the privacy checklist.
5. A maintainer reviews every PR. Changes to the escrow, signing, verification or auditing code need a maintainer's approval (see `.github/CODEOWNERS`). PRs merge once CI is green.

## Releases

Versions follow [Semantic Versioning](https://semver.org). `CHANGELOG.md` collects changes under "Unreleased"; a release moves them under the new version, tags `vX.Y.Z` on `main`, and publishes GitHub release notes from that section.

## Licence

By contributing you agree that your contributions are dual-licensed under [MIT](LICENSE-MIT) and [Apache-2.0](LICENSE-APACHE), as the rest of the project is.

## Conduct

Everyone taking part is expected to follow the [Code of Conduct](CODE_OF_CONDUCT.md).
