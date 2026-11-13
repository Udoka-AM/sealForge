# 2.6 Clearing

Status: `Testnet` (uniform price). First price is `Tested` only: both testnet auctions were uniform price.

Code: `crates/sealforge-core/src/clearing.rs` `clear`, called only through `crates/sealforge-core/src/result.rs` `compute_result`, which also builds the Merkle root. The coordinator, the CLI auditor and the WASM auditor all call `compute_result`.

## Inputs

- `AuctionRules { mode, quantity, reserve }`. `quantity ≥ 1` (`ZeroQuantity` otherwise; the database also limits it to 1–100,000).
- One `ClearingBid { leaf_hash, block_height, amount }` per **included** bid ([2.5](selection.md)): `valid` and `under_reserve`. In sealed-reveal mode, `amount` is the revealed price.
- Leaf hashes MUST be unique (`DuplicateLeaf` otherwise).

## Algorithm

1. **Eligible bids** = bids with `amount ≥ reserve`. Bids below it lose and get a full refund (`reserve_excludes_low_bids_with_full_refund`).
2. **Sort eligible bids** by the canonical priority (`clearing::priority`):
   1. `amount` descending,
   2. then `block_height` ascending (earlier bids win ties),
   3. then `leaf_hash` ascending (bytewise).

   The order is total because leaf hashes are unique (`ties_break_on_height_then_leaf_hash`).
3. `winner_count = min(eligible, quantity)`. The first `winner_count` bids win.
4. `clearing_price` = the amount of the **last winner**. With no eligible bids it is `None`, nobody wins, and everyone is refunded (`no_eligible_bids`).
5. Per bid:

| Mode | Winner pays | Loser pays | Refund |
|---|---|---|---|
| Uniform price | `clearing_price`, the `winner_count`-th highest eligible bid | 0 | `amount − paid` |
| First price | its own `amount` | 0 | `amount − paid` |

**Fewer bids than units.** If `eligible < quantity`, every eligible bid wins, and in uniform-price mode everyone pays the **lowest eligible bid** (`undersubscribed_uniform_auction_charges_lowest_bid`). The seller is protected by the reserve, not by the unit count.

`total_volume` = the sum of eligible amounts (checked against the ZEC supply, `VolumeOverflow`). `valid_bid_count` = number of eligible bids. Outcomes are returned **sorted by leaf hash**, one per included bid.

In sealed-reveal mode the refund also returns `deposit − price` plus the counting reveal payment. The coordinator adds those at close ([2.5](selection.md#sealed-reveal-mode)).

## Determinism guarantee

The result is a pure function of `(rules, set of bids)`:
- **Independent of input order.** Eligibility is per bid, the priority is a total order, and outcomes are sorted by leaf hash (`result_is_independent_of_input_order`). The Merkle root sorts leaves too ([2.4](commitments.md#merkle-tree)).
- **Byte-identical across implementations.** The coordinator, `sealforge-audit` and the WASM auditor link the same `sealforge-core`. Only the build target differs. `sealforge-core` has no I/O, clock, randomness or floating point (amounts are `u64` zatoshi).
- **Golden fixtures.** `crates/sealforge-core/tests/pipeline.rs` pins the exact clearing outcome and Merkle root for a 20-bid uniform-price auction and a 500-bid first-price auction (`tests/fixtures/bids-20*.json`, `bids-500*.json`). It also checks that payments plus refunds equal deposits for every bid, that every refund splits exactly into notes plus dust, and that every bidder has a valid inclusion proof. Values: [Appendix: test vectors](../appendix-test-vectors.md#20-bid-result).
- **Scale.** 20,000 bids in under one second is a test (`clears_twenty_thousand_bids_quickly`). Measured timing is in [evidence/perf.md](../../evidence/perf.md).

The published `algorithm_version` (`sealforge-core/<version>/clearing-v1`) names these rules. A change to them MUST change the `clearing-v1` suffix.
