# Evidence: a full auction on a real (local) Zcash chain

**Date:** 2026-10-15. **Chain:** Zebra 7.0.0-rc.0 regtest with Ironwood (NU6.3) active, Zaino 0.10.2 for lightwallet gRPC (`scripts/regtest.sh`). Everything below used the real binaries: `sealforge-coordinator` (with its scanner, closer and refund planner running) and the `sealforge` CLI, plus a dev wallet that builds real shielded Ironwood transactions.

| Step | What happened | Result |
|---|---|---|
| 1. Coordinator | Started with `SF_NETWORK=regtest`, no dev chain height | The scanner read the real tip (221) from Zaino |
| 2. Organiser | `sealforge init`, `auction new` (3 lots, uniform price, 0.01 ZEC reserve, close at 425) | Draft `JTY3AX0SMC` with 3 trustee invites |
| 3. Trustees | Three keystores: `trustee join`, then `trustee dkg` | Same fingerprint for all three. Escrow `uregtest1x05sx6…` |
| 4. Open | `auction open` | Opened at the real chain height 221 |
| 5. Bids | The dev wallet sent 4 shielded Ironwood payments with `SF1:` memos: 0.5, 0.2, 1.0 and 0.05 ZEC | Mined at heights 222–225 |
| 6. Scanner | The escrow's view-only wallet synced from Zaino, decrypted the memos and recorded the bids | 4 bids, all `valid`, refund addresses encrypted |
| 7. Close | Mined to 436. The closer waited for 10 confirmations past 425 | Closed automatically |
| 8. Publish | `sealforge auction publish` | Price 0.2 ZEC, volume 1.75 ZEC, 3 winners, scan range 221–425 |
| 9. Verify | The 0.5 ZEC bid's receipt was checked on `/verify` (WASM auditor) | Included under the published root, won at 0.2 ZEC, 0.3 ZEC refund as 2 notes |
| 10. Refunds | The refund planner ran after publishing | Auction moved to `refunding` with the refund notes planned |

Published outcomes:

| Bid | Block | Outcome | Paid | Refund |
|---|---|---|---|---|
| 1.0 ZEC | 224 | won | 0.2 | 0.8 |
| 0.5 ZEC | 222 | won | 0.2 | 0.3 |
| 0.2 ZEC | 223 | won | 0.2 | 0 |
| 0.05 ZEC | 225 | lost | 0 | 0.05 |

## Reproduce

```bash
scripts/regtest.sh up <dev-wallet-UA>     # see infra/regtest/README.md
cargo run -p sealforge-chain --features dev-wallet --example regtest_bid -- address dev.sqlite 1
# start the coordinator with SF_NETWORK=regtest SF_ZAINO_URL=http://127.0.0.1:8137,
# then follow docs/QUICKSTART.md with network = "regtest"
cargo run -p sealforge-chain --features dev-wallet --example regtest_bid -- bid dev.sqlite <escrow-ua> - - <CODE> 50000000 1
scripts/regtest.sh mine 1
```

## Refunds and payout on chain (tasks 3.9, 3.11)

The same auction, continued with `sealforge trustee sign` on each trustee's keystore:

| Step | What happened | Result |
|---|---|---|
| 11. Batch | The batcher held due notes until 3 bidders' notes could share a transaction | 1 refund batch: 6 outputs from 3 bidders, 1.15 ZEC |
| 12. Build | The chain worker proposed, created and proved the PCZT (3 escrow spends) | `awaiting_signatures` in about 4 s (release build) |
| 13. Sign | Trustees 3 and 1 committed first and signed; trustee 2 was told it wasn't needed | Coordinator aggregated, checked each signature against `rk`, broadcast |
| 14. Confirm | Mined 1 block | Refund tx `b0cf5894…9e6f` confirmed at **438** |
| 15. Payout | The escrow was swept to the payout UA: 0.59965 ZEC left, minus the 0.0001 ZEC fee | Payout batch 0.59955 ZEC, 2 escrow spends |
| 16. Sign | Trustees 2 and 1 signed this time | Broadcast |
| 17. Settle | Mined 1 block | Payout confirmed at **439**, auction `settled` automatically |

Escrow arithmetic: 1.75 ZEC in, 1.15 ZEC refunded, 0.00035 ZEC refund fee (ZIP-317, 7 logical actions), 0.59955 ZEC payout, 0.0001 ZEC payout fee. Winners paid 0.6 ZEC, so the organiser bore 0.00045 ZEC of fees. There was no refund dust in this run.

Caveat: all four test bids used the dev wallet's single refund address, so this run shows the mechanics, not unlinkability. The browser bid flow uses a fresh refund address per bid, and the linkability harness measures mixing (`docs/evidence/linkability.md`).
