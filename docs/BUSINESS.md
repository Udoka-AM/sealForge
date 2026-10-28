# The case for sealForge

**Every public sale leaks who paid what. That leak has a price, and someone pays it.**

## The problem, in money terms

Allocation is one of the few places where a blockchain's transparency is a
direct cost rather than a feature.

Run an auction on a transparent chain and every bid is public before it clears.
The predictable results:

- **Bidders shade down.** A bid that can be seen is a bid that can be beaten by
  one tick. Rational bidders under-reveal, and the seller clears below true
  demand.
- **Speed beats value.** Visible order flow is an invitation. The allocation
  goes to whoever reacts fastest to someone else's bid, not to whoever values
  the asset most.
- **Bidders are profiled permanently.** An address that bid on a sale is linked
  to that interest forever. For a treasury, a fund or a competitor, that alone
  rules out participating.

The usual fix is to move the sealed part off-chain and ask everyone to trust the
operator. That trades one cost for another: the operator now sees every bid, and
the participants have no way to check the result they were given.

**sealForge removes the choice.** Bids are sealed because they are shielded
payments carrying a commitment. The result is public and recomputable by anyone
from the chain. Nobody has to trust the operator, and the operator never gets
the information in the first place.

## Why this needs Zcash specifically

This is not an application that happens to run on Zcash. Three properties have
to be in the same place, and they are only in the same place here.

| Requirement | What Zcash provides |
|---|---|
| The bidder must be hidden | Shielded transfers hide sender, amount and memo by default |
| The bid must be binding, without extra infrastructure | The encrypted memo carries the salted commitment, so **the bid is the payment** — one on-chain action, no separate commit contract or sequencer |
| The result must be publicly verifiable without deanonymising anyone | Incoming viewing keys give **selective disclosure**: publish one key and anyone recomputes amounts and memos, while senders stay private |

A transparent chain can do the third and not the first. A mixer can do the first
and not the third. The combination is what makes a sealed-bid auction both
private and auditable, and it is Zcash's to own.

## Where the demand is

Sealed-bid mechanisms are the standard answer wherever the thing sold is scarce
and the buyers are strategic. Each of these is an existing market that currently
runs either transparently or on trust.

| Vertical | Why sealed bids win |
|---|---|
| **Name registration** (e.g. premium Zcash Name Service names) | First-come-first-served rewards bots. A sealed auction prices the name and funds the registry |
| **Token and NFT launches** | Public bidding invites snipers and gas wars; sealed bids clear at a real price |
| **Ticketing and passes** | Scalping is an arbitrage on a known price. A uniform-price sealed auction removes the gap |
| **Treasury and RWA allocations** | Institutions will not bid where the bid is a public signal |
| **OTC blocks and DAO allocations** | Counterparties need privacy during, and provability after |

The common shape: the seller wants the highest honest price, the buyers want
privacy, and both want to be able to prove afterwards that it was fair. That is
exactly what the published viewing key and Merkle root deliver.

## ShieldRamp: an on-ramp disguised as a feature

The hard constraint on any Zcash-native market is that a participant must
already hold shielded ZEC. That caps the addressable buyer set to people who
have already crossed the bridge.

**ShieldRamp removes the precondition.** A holder of USDC or ETH bids, and the
swap settles into the shielded pool through NEAR Intents, with a
**Linkability Score** shown before signing so the bidder sees the residual risk
honestly rather than being told it is "anonymous".

The ecosystem consequence matters more than the feature:

- **Each cross-chain bid is net inflow into shielded ZEC.** The auction is the
  reason to acquire it, and the shielded pool is where it lands.
- **It makes the privacy the product, not the hurdle.** The bidder is not
  buying ZEC and then finding a use; they are buying an outcome that requires
  shielded settlement.
- **It is a demand-side on-ramp.** Most bridges move liquidity because someone
  already wants the asset. This creates the want first.

That is why ShieldRamp sits on the roadmap directly behind mainnet rather than
in a backlog. Its session model, step machine and the Linkability Score are
implemented and tested today; the swap leg follows mainnet because NEAR Intents
settles real assets and has no test network to exercise it against.

## How it makes money

| Model | Mechanics |
|---|---|
| **Take rate on settled volume** | A basis-point fee on the clearing proceeds, swept with the payout the escrow already performs |
| **Hosted coordinator** | Organisers who want the auction run for them, rather than deploying the stack |
| **White label** | Registries, launchpads and ticketing platforms embedding the auction card and the audit page under their own brand |
| **Trustee as a service** | Independent trustees are a requirement, not an extra. Being one of the three, for a fee, is a service with a real cost base |

The escrow already sweeps a payout to the organiser, so a protocol fee is a
change to one amount, not new machinery.

## What is defensible

- **The auditor is the trust anchor**, and it is the hard part. A result that is
  a pure function of on-chain shielded data, reproducible by anyone with the
  published key, is what makes the privacy claim checkable rather than asserted.
- **The escrow signing path** — rerandomised FROST over the PCZT alpha, signing
  real Ironwood spends — took the bulk of the build and is not quick to
  reproduce.
- **Unlinkability as a measured quantity.** 0.1% of refunds linkable at the
  exit, against 100% for naive refunds, with the harness published. Competitors
  claiming privacy without a number are making a weaker claim.

## What would have to be true

Stated plainly, because the same honesty is what makes the audit claim credible:

- **Mainnet, with guardrails.** Per-auction caps, genuinely independent
  trustees, and an external review of the escrow signing path before real funds.
- **One real seller in one vertical.** The mechanism is proven; the demand is
  not. A first paying auction matters more than a second feature.
- **ShieldRamp swap economics.** Fees and slippage on the cross-chain leg have
  to leave the bid competitive. That is measurable only against live routes.

## Where to look next

| | |
|---|---|
| Try an auction, audit it yourself | [sealforge.udokaam.dev](https://sealforge.udokaam.dev) |
| What it does, in product terms | [PRODUCT.md](PRODUCT.md) |
| What is shipped and what is sequenced | [README › Status](../README.md#status) |
| What it does not protect against | [THREAT-MODEL.md](THREAT-MODEL.md) |
| Refund unlinkability, measured | [evidence/linkability.md](evidence/linkability.md) |
