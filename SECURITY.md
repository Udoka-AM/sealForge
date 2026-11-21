# Security policy

sealForge holds bidders' funds in a 2-of-3 FROST escrow, so we take reports seriously and fix them in public.

## Reporting a vulnerability

**Report privately** through GitHub: [Report a vulnerability](https://github.com/Udoka-AM/sealForge/security/advisories/new) (the repository's Security tab). Only the maintainers see it. Please don't open a public issue, pull request or forum post for anything that could put funds or bidder privacy at risk before it's fixed.

Helpful to include:

- What an attacker can do, and what they need (which role: bidder, organiser, trustee, coordinator operator, network observer).
- The commit you read, and file and line references.
- Steps or a test that shows it, on testnet or a local regtest chain. **Never test against real funds or other people's auctions.**

Plain English is fine; short sentences help if you use a translation tool.

## What to expect

- An acknowledgement within 3 days.
- A first assessment (confirmed or not, and severity) within 7 days.
- Fixes are developed in public pull requests once the issue is safe to disclose, or right away when it can't be exploited on a live network. Each finding gets an entry in [docs/SECURITY-REVIEW.md](docs/SECURITY-REVIEW.md).
- Credit in SECURITY-REVIEW.md under the name or handle you choose, or none if you prefer.

There is no paid bug bounty yet. The grant proposal plans "Audit this auction" bounties for published figures that don't match the chain.

## Scope

In scope: everything in this repository: the auction core, escrow and signing, trustee CLI and browser signing, coordinator, auditor (CLI and WebAssembly), wallet, web app, and the deployment configuration under `infra/`.

Most important: anything that lets someone move or freeze escrowed funds, sign something trustees didn't check, change a published result without the auditor noticing, or link a bid or refund to a bidder beyond what [docs/THREAT-MODEL.md](docs/THREAT-MODEL.md) already states.

Known limits are listed in the threat model and in the [Mainnet gate](https://github.com/Udoka-AM/sealForge/milestone/1) issues. Reports that sharpen or break those assumptions are welcome too.

## Supported versions

Only `main`. The public site runs on Zcash testnet; there is no mainnet deployment yet.
