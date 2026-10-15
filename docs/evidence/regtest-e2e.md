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

## Not yet on chain

Refund notes are planned and batched, but not yet signed and broadcast. That is the PCZT + FROST step (task 3.9). The signing cryptography is proven in `sealforge-escrow`.
