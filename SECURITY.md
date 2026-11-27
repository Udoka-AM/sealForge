# Security policy

sealForge holds bidders' funds in a 2-of-3 FROST escrow, so we take reports seriously and fix them in public.

## Reporting a vulnerability

**Report privately** through GitHub: [Report a vulnerability](https://github.com/Udoka-AM/sealForge/security/advisories/new) (the repository's Security tab). Only the maintainers see it. If you can't use GitHub, email [udoka.am@gmail.com](mailto:udoka.am@gmail.com) with "sealForge security" in the subject, and we'll move the report to a private advisory. Please don't open a public issue, pull request or forum post for anything that could put funds or bidder privacy at risk before it's fixed.

Helpful to include:

- What an attacker can do, and what they need (which role: bidder, organiser, trustee, coordinator operator, network observer).
- The commit you read, and file and line references.
- Steps or a test that shows it, on testnet or a local regtest chain. **Never test against real funds or other people's auctions.**

Plain English is fine; short sentences help if you use a translation tool.

## What to expect

| Step | Target |
|---|---|
| Acknowledgement of your report | within 3 days |
| First assessment: confirmed or not, and a severity | within 7 days |
| Fix for a Critical or High finding | within 30 days of confirmation |
| Fix for a Medium or Low finding | within 90 days, or the next release |

Severity follows the impact on funds and privacy: **Critical** lets someone move or freeze escrowed funds, or forge a result the auditor accepts; **High** breaks a stated privacy property or a trustee check; **Medium** and **Low** cover the rest.

We keep you updated while we work on it. Fixes for anything exploitable on a live network are developed privately and published once deployed; the rest are fixed in public pull requests. Every finding gets an entry in [docs/SECURITY-REVIEW.md](docs/SECURITY-REVIEW.md) and, when it affects a release, a GitHub security advisory.

## Coordinated disclosure

We ask you to keep a finding private until it is fixed and deployed, or for 90 days from your report, whichever comes first. If a fix needs longer we'll explain why and agree a date with you. Credit goes in SECURITY-REVIEW.md and the advisory under the name or handle you choose, or none if you prefer.

## Safe harbour

We won't pursue or support legal action against research done in good faith under this policy: on testnet or your own regtest chain, without touching other people's funds, data or auctions, without degrading the public service, and reported to us privately. If you're unsure whether something is in bounds, ask through the private report first.

There is no paid bug bounty yet.

## Scope

In scope: everything in this repository: the auction core, escrow and signing, trustee CLI and browser signing, coordinator, auditor (CLI and WebAssembly), wallet, web app, and the deployment configuration under `infra/`.

Most important: anything that lets someone move or freeze escrowed funds, sign something trustees didn't check, change a published result without the auditor noticing, or link a bid or refund to a bidder beyond what [docs/THREAT-MODEL.md](docs/THREAT-MODEL.md) already states.

Known limits are listed in the threat model and in the [Mainnet gate](https://github.com/Udoka-AM/sealForge/milestone/1) issues. Reports that sharpen or break those assumptions are welcome too.

## Supported versions

Only `main`. The public site runs on Zcash testnet; there is no mainnet deployment yet.
