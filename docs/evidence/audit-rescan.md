# Independent audit from the chain (task 3.4, AC7)

`sealforge-audit` rebuilds an auction's result from the chain alone. It uses the published incoming viewing key and a lightwallet server the auditor picks, and nothing from the coordinator except the published result being checked.

How it works:
1. Trial-decrypt compact blocks in the published scan range with the escrow's UIVK, for both the Orchard and Ironwood pools (`sealforge_chain::audit::scan_incoming`).
2. Fetch each matching transaction in full and decrypt the payment's value and memo.
3. Run the same selection, clearing and Merkle code as the coordinator (`sealforge-core`) and compare every field.

## Runs on the local chain (2026-10-15)

```text
$ sealforge-audit --published http://127.0.0.1:8080/v1/auctions/JTY3AX0SMC/result --endpoint http://127.0.0.1:8137
Scanned blocks 221–425 of JTY3AX0SMC in 0.1 s: 4 payment(s) to the escrow.
✓  clearing_price_zat       published 20000000   recomputed 20000000
✓  total_volume_zat         published 175000000  recomputed 175000000
✓  valid_bid_count          published 4          recomputed 4
✓  winner_count             published 3          recomputed 3
✓  merkle_root              published 76f6ca94…dd94  recomputed 76f6ca94…dd94
✓  bid_outcomes_mismatched  published 0          recomputed 0
Match: the published result is exactly what the chain says.

$ sealforge-audit --published …/NKZNE8FM69/result --endpoint http://127.0.0.1:8137
Scanned blocks 439–631 of NKZNE8FM69 in 0.1 s: 5 payment(s) to the escrow.
(all ✓, root a8325e35…9a8b)
```

The second auction's escrow also received the organiser's fee-buffer deposit. The auditor finds it among the 5 payments and leaves it out of the bid set exactly as the coordinator did.

**Tamper test:** the NKZNE8FM69 result was edited to hide the 0.30 ZEC bid and claim a 0.05 ZEC price. Clearing price, valid bid count and per-bid outcomes failed, the exit code was 1, and the output said "MISMATCH: the published result does not follow from the chain."

The browser Audit page still re-checks only the published list. It now shows the CLI command for this independent rescan.
