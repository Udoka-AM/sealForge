# sealForge: Zecathon submission

**Track:** Private Markets
**Repo:** https://github.com/Udoka-AM/sealForge (MIT / Apache-2.0)
**Live (Zcash testnet):** https://sealforge.udokaam.dev · audit of the first auction: https://sealforge.udokaam.dev/audit/KE5DFF4PDA
**Demo video:** _link_

## One line

Sealed-bid auctions on Zcash where the clearing price is public, the bidders aren't, and anyone can recompute the result from the chain.

## The problem

Every public allocation leaks who paid what: tickets, passes, name registrations, token sales. On transparent chains, bids are public, so people front-run, snipe and profile the bidders. The usual fix is to trust the organiser. That gives the organiser full visibility and asks everyone else to take the result on faith.

## What we built

- **A built-in shielded wallet.** You create or restore it in the page with a 24-word phrase. It syncs, proves and signs in a Web Worker using the same librustzcash crates as the escrow, and holds and spends Ironwood notes. The phrase is encrypted with your passphrase and never leaves the browser. Each bid gets a fresh address, so refunds can't be linked to each other or to your funding address. Paying a deposit or a reveal is one tap.
- **Bidding from any Zcash wallet.** One ZIP-321 QR code: a shielded Ironwood payment whose memo carries a salted commitment and a fresh refund address. The receipt never leaves the browser.
- **A 2-of-3 FROST escrow.** Three trustees run a distributed key generation on their own machines. Any two co-sign refunds and the payout as PCZT spends, from the CLI or in the browser (WebAssembly). One trustee alone can't move funds, and neither can the coordinator.
- **A public audit.** After close, the escrow's viewing key and a Merkle root of all bids are published. `sealforge-audit`, or the browser, rescans the chain with only that key and a lightwallet server the auditor chooses. It recomputes the price, volume, winners, every bidder's outcome and refund, and the root, and fails loudly on any mismatch. Bidders check their own inclusion from their receipt.
- **Sealed-reveal mode.** Bidders deposit a standard budget, and the memo commits to the hidden bid. They reveal on chain after the close. Until then nobody sees bid amounts: not the coordinator, not the trustees, not the public.
- **Unlinkable refunds.** Refunds are split into standard denominations, sent after random delays and batched across bidders. Our harness measures 0.1% of refunds as linkable, against 100% for naive refunds.
- **My bids.** Each bid's life is shown from evidence only: payment confirmed in a block, included in the published result (Merkle proof checked in the browser), won or lost, refund received, settled.
- **An organiser console.** Create, open and publish auctions, and follow refunds and the payout. The organiser's key is a non-extractable WebCrypto key.

## Privacy, honestly

| | |
|---|---|
| Chain observers | See shielded transfers to an escrow. After close: amounts and memos, never senders |
| Coordinator and trustees | Deposit mode: they see bid amounts live, and the auction page says so. Sealed-reveal mode: only budgets until the reveals |
| Funds | 2-of-3 FROST. The coordinator holds no key share |
| A dishonest coordinator | Can't steal. A wrong or censored result fails the public chain audit |
| The built-in wallet | The phrase never reaches our server. The lightwallet server it syncs from sees your IP and timing, as with any light wallet |
| What we don't hide | That someone bid some amount into this escrow. The bid count is shown only from 5 bids up |

Full threat model: `docs/THREAT-MODEL.md`. Security review and the bugs it found: `docs/SECURITY-REVIEW.md`.

## Proven on Zcash testnet

- **Genesis Pass (`KE5DFF4PDA`):** 7 sealed bids from 7 wallets, one deliberately unrevealed and one under the reserve. Price 0.07 TAZ, 4 winners. Two refund transactions and the payout were each co-signed by a different pair of trustees, and the auction settled. The chain audit matches, from the CLI and in the browser. ([evidence](../evidence/testnet-genesis.md))
- **Community Pass (`K7GHHBC4GP`):** 25 sealed bids, 10 units. Price 0.0072 TAZ, exactly as predicted from the bids. 73 refund notes went out in 5 transactions, mixing up to 20 bidders each. It settled 68 minutes after launch, and every bid was recorded within 2 blocks. ([evidence](../evidence/test-run.md))
- **The full bidder journey in a browser with the built-in wallet:** restore the wallet, pay the deposit (confirmed at block 4,510,116), reveal (4,510,148), win at 0.004 TAZ with the proof checked in the browser, and receive the 0.0011 TAZ refund at the bid's own address in 2 notes. ([evidence](../WALLET-INTEGRATION.md))
- **Test plan:** 12 of 14 acceptance criteria pass with evidence. 89 Rust and 10 TypeScript tests pass. Testnet found three coordinator bugs (concurrent trustee joins, concurrent signature shares, auctions without winners), and each is fixed with a test that fails without the fix.
- **Also on a local regtest chain with Ironwood active:** deposit mode, a run driven entirely from the browser console, and sealed-reveal mode. ([evidence](../evidence/regtest-e2e.md))

Not run on mainnet: we had no mainnet ZEC to fund bidders. The code supports mainnet (Ironwood), and the hosted instance switches with one setting.

## What's original

- An auction escrow whose spend authority is a FROST threshold key, signing real Ironwood PCZT spends (rerandomized FROST over the PCZT α).
- A clearing result that is a pure function of on-chain shielded data, reproducible by anyone holding the published viewing key.
- Commit-reveal with on-chain reveals, so the coordinator can't censor a reveal without failing the audit.
- A browser wallet that builds, proves and signs Ironwood transactions from librustzcash compiled to WebAssembly, with nothing implemented from scratch.
- Refund unlinkability treated as a measured quantity, not a claim.

## What's next

- **Mainnet launch:** per-auction caps, independent trustees, and an external review of the escrow signing path.
- **A first real use:** for example, sealed-bid auctions for premium Zcash Name Service names, so they aren't first come, first served.
- **ShieldRamp:** bidding with USDC or ETH through NEAR Intents, with a Linkability Score. The session model, step machine and the Score are implemented and tested; the swap leg follows mainnet, because NEAR Intents settles real assets and has no test network. Until it is live the UI hides cross-chain funding rather than offering a path that would fail. Why it matters beyond the feature: every cross-chain bid is net inflow into the shielded pool, which makes sealForge a demand-side on-ramp for Zcash, not only an auction house. See [BUSINESS.md](../BUSINESS.md).
- **Reusable libraries:** releasing the browser Ironwood wallet and the FROST escrow as open-source components.

## Team

Udoka-AM
