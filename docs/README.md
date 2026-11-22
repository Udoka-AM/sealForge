# sealForge docs

The public documentation site is published with Mintlify at <https://sealforge.mintlify.app>. Its source is [site/](site/) (MDX pages plus `docs.json`). The files below are the engineering record it cites.

## Current reference

| Doc | Purpose |
|---|---|
| [THREAT-MODEL.md](THREAT-MODEL.md) | Trust assumptions, adversaries, protections, where each is enforced, known limits |
| [SECURITY-REVIEW.md](SECURITY-REVIEW.md) | Security review: every finding (F1–F17) with its fix, tests and credit, key handling, open items |
| [05-BACKEND-SCHEMA.md](05-BACKEND-SCHEMA.md) | Tables, auth flow, validation, retention, migrations |
| [DEPLOY-VPS.md](DEPLOY-VPS.md) | Hosting the stack on one server: layout, deploys, rollback, fee and admin configuration |
| [VERSIONS.md](VERSIONS.md) | Pinned dependency versions and when each was verified |
| [QUICKSTART.md](QUICKSTART.md) | Run your own auction, from clone to open |
| [WALLET-INTEGRATION.md](WALLET-INTEGRATION.md) | The in-browser wallet and the bidder journey (compatibility contract) |
| [BUSINESS.md](BUSINESS.md) | The business case: problem, verticals, revenue models (the take rate built, the rest `Specified`) |
| [spikes/frost-pczt.md](spikes/frost-pczt.md) | FROST 2-of-3 escrow spike for Ironwood spends |

## Evidence

| File | What it records |
|---|---|
| [evidence/test-run.md](evidence/test-run.md) | Acceptance test run AC1–AC14, and the 25-bid testnet auction `K7GHHBC4GP` |
| [evidence/test-inventory.md](evidence/test-inventory.md) | Generated list of every automated test, by crate and package |
| [evidence/testnet-genesis.md](evidence/testnet-genesis.md) | The first testnet auction, `KE5DFF4PDA` (7 sealed bids, 4 units) |
| [evidence/regtest-e2e.md](evidence/regtest-e2e.md) | A full auction on the local Zebra + Zaino regtest chain |
| [evidence/audit-rescan.md](evidence/audit-rescan.md) | Independent chain audit (CLI and browser) and the tamper test |
| [evidence/linkability.md](evidence/linkability.md) | Measured refund linkability (with `linkability.json`, `linkability.svg`) |
| [evidence/perf.md](evidence/perf.md) | Performance pass against TRD targets |
| [evidence/backup-drill.md](evidence/backup-drill.md) | Backup and restore drill |
| [evidence/a11y.md](evidence/a11y.md) | Accessibility pass |

## Design history

Written to plan the first build in October 2026. Kept as they were; where they disagree with the code, the code and the current reference above win.

| Doc | Purpose |
|---|---|
| [PRODUCT.md](PRODUCT.md) | Product description from the build plan |
| [01-PRD.md](01-PRD.md) | Product requirements: users, goals, features, acceptance criteria, open questions |
| [02-TRD.md](02-TRD.md) | Technical requirements: stack, hosting, auth, architecture, security |
| [03-APP-FLOW.md](03-APP-FLOW.md) | Screens, journeys, per-action states |
| [04-UI-UX-BRIEF.md](04-UI-UX-BRIEF.md) | Visual direction |
| [06-IMPLEMENTATION-PLAN.md](06-IMPLEMENTATION-PLAN.md) | Milestones and tasks |
| [history/zecathon-2026/](history/zecathon-2026/) | The Zecathon 2026 submission write-up and demo script, where the project started |
