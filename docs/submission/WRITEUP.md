# sealForge: Zecathon submission (draft)

**Track:** Private Markets
**Repo:** https://github.com/Udoka-AM/sealForge (MIT / Apache-2.0)
**Demo:** _video link_ · _live site link_ · _mainnet auction code_

## One line

Sealed-bid auctions on Zcash where the clearing price is public, the bidders aren't, and anyone can recompute the result from the chain.

## The problem

Every public allocation (tickets, passes, NFT drops, token sales) leaks who paid what. On transparent chains, bids are public, so people front-run, snipe and profile the bidders. The usual fix, "trust the organiser", gives the organiser full visibility and asks everyone else to take the result on faith.

## What we built

- **Bidding from any Zcash wallet.** One ZIP-321 QR: a shielded Ironwood payment whose memo carries a salted commitment and a fresh refund address. The receipt never leaves the browser.
- **A 2-of-3 FROST escrow.** Three trustees run a distributed key generation on their own machines. Any two co-sign refunds and the payout as PCZT spends, from the CLI or in the browser via WebAssembly. One trustee alone, or the coordinator, can't move funds.
- **A public audit.** After close, the escrow's incoming viewing key and a Merkle root of all bids are published. `sealforge-audit` rescans the chain with only that key and a lightwallet server of the auditor's choice. It recomputes the price, volume, winners, every bidder's outcome and refund, and the root, and fails loudly on any mismatch. Bidders verify their own inclusion from their receipt in the browser.
- **Sealed-reveal mode.** Bidders deposit a standard budget, and the memo commits to the hidden bid. They reveal on chain after the close. Until then nobody (coordinator, trustees, public) sees bid amounts.
- **Unlinkable refunds.** Refunds are split into standard denominations, released after random delays, and batched across bidders. Our harness measures 0.1% of refunds linkable at the exit, against 100% for naive refunds.
- **An organiser console.** Create, open and publish an auction, and follow refunds and the payout, from the browser. The organiser's key is held as a non-extractable WebCrypto key.

## Privacy, honestly

| | |
|---|---|
| Chain observers | See shielded transfers to an escrow. After close: amounts and memos, never senders |
| Coordinator and trustees | Deposit mode: see bid amounts live, and the auction page says so. Sealed-reveal mode: see only budgets until the reveals |
| Funds | 2-of-3 FROST. The coordinator holds no key share |
| A dishonest coordinator | Can't steal. A wrong or censored result fails the public chain audit |
| What we don't hide | That someone bid some amount into this escrow. The number of bids is shown only from 5 bids up |

Full threat model: `docs/THREAT-MODEL.md`. Security review and the bugs it caught: `docs/SECURITY-REVIEW.md`.

## Proven

- Full auctions on a Zcash regtest chain with Ironwood active: deposit mode, a run driven entirely from the browser console, and sealed-reveal mode. Each was bid, closed, published, refunded with 2-of-3 signatures, paid out and settled. The chain audit matches byte for byte (`docs/evidence/`).
- _Mainnet micro-auction: code, bids, settlement, outside auditor's statement._

## What's original

- An auction escrow whose spend authority is FROST-threshold, signing real Ironwood PCZT spends (rerandomized FROST over the PCZT α).
- A clearing result that is a pure function of on-chain shielded data, reproducible by anyone holding the published viewing key.
- Commit-reveal with on-chain reveals, so the coordinator can't censor a reveal without failing the audit.
- Refund unlinkability treated as a measured quantity, not a claim.

## What's next

ShieldRamp (bid with USDC or ETH through NEAR Intents into an ephemeral shielded wallet, with a Linkability Score), a hosted public instance, and an embeddable auction widget.

## Team

Udoka-AM
