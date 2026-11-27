## Summary

<!-- What changes and why, in a few sentences. Link the issue it closes: "Closes #123". -->

## How it was tested

<!-- Commands run, tests added, and for changes that touch the chain, the regtest or testnet run and where its evidence is recorded (docs/evidence). -->

## Checklist

- [ ] `cargo fmt`, `cargo clippy -D warnings` and `cargo test --workspace` pass; `pnpm lint`, `pnpm typecheck` and `pnpm test` pass for web changes
- [ ] WebAssembly modules rebuilt with `scripts/build-wasm.sh` if any crate they include changed
- [ ] Docs updated in the same PR where behaviour, the API, the data model or a security property changed
- [ ] `CHANGELOG.md` updated under "Unreleased" for user-visible changes

## Privacy and security

- [ ] No transparent hops added to bid, refund or payout flows
- [ ] No address reuse
- [ ] Nothing new logged that identifies bidders (IPs, refund addresses, commitments, salts, seeds)
- [ ] `docs/THREAT-MODEL.md` still accurate, or updated here
