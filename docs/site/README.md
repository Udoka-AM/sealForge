# sealForge documentation

Auditable sealed-bid auctions on Zcash: the clearing price is public, the bidders aren't.

These pages are written for Zcash protocol and wallet engineers first. Every technical claim cites the code (`path`, function or constant) or an evidence file. Every behaviour carries a status label from the table below. Plain Markdown, so it renders on GitHub, Mermaid included. The engineering documents in [`docs/`](../README.md) (PRD, TRD and the rest) are the design history these pages cite.

## Status labels

| Label | Meaning |
|---|---|
| `Mainnet` | Has run on Zcash mainnet. **Nothing has yet.** |
| `Testnet` | Has run on public Zcash testnet. The page cites the auction code and transactions |
| `Regtest` | Exercised end to end on the local Zebra + Zaino chain |
| `Tested` | Unit or integration tests only |
| `Specified` | Types or design exist, not built |
| `Not built` | Neither |

## Contents

1. [Overview](01-overview.md): what it is, status of every component, the three guarantees and what isn't guaranteed
2. Protocol specification (`SF1`)
   - [2.1 Roles and what each one knows](02-protocol/roles.md)
   - [2.2 Lifecycle state machine](02-protocol/lifecycle.md)
   - [2.3 Wire formats](02-protocol/wire-formats.md)
   - [2.4 Commitments and domain separation](02-protocol/commitments.md)
   - [2.5 Bid selection](02-protocol/selection.md)
   - [2.6 Clearing](02-protocol/clearing.md)
   - [2.7 Refund privacy](02-protocol/refunds.md)
3. [Zcash integration reference](03-zcash-integration.md): every protocol touchpoint, and the upstream dependency risks
4. [Architecture and infrastructure](04-architecture.md)
5. [Security](05-security.md): trust assumptions, threat model, key custody, findings, mainnet gates
6. [Verification and evidence](06-verification.md): audit any auction, the testnet record, reproducibility, test inventory
7. [Guides](07-guides.md): organiser, trustee, bidder, auditor, integrator
8. [API and SDK reference](08-api.md)
9. [Product and economics](09-product.md)
10. [Roadmap and mainnet gates](10-roadmap.md)
11. [Open questions for reviewers](11-open-questions.md)

Appendices: [glossary](appendix-glossary.md), [test vectors](appendix-test-vectors.md), [ZIP and spec index](appendix-specs.md), [changelog and memo versioning](appendix-changelog.md).

## Conventions

- Amounts are integer zatoshi (1 ZEC = 10^8 zat). "TAZ" is testnet ZEC.
- `‖` is byte concatenation. `u32 LE` and `u64 LE` are little-endian unsigned integers.
- Paths are relative to the repository root. Code citations are to `main` at the commit these pages were written against (see the [changelog](appendix-changelog.md)).
- Upstream behaviour (Orchard, Ironwood, PCZT, RedPallas, note encryption, consensus) is cited to the ZIP, the protocol specification or the upstream source and version. It is not restated from memory.
