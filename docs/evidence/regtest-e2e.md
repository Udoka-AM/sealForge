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

## The whole lifecycle from the console (task 3.10)

A second auction, `NKZNE8FM69`, run from the browser console against the same local chain. The coordinator ran with `SF_REFUND_DELAY_SECS=20,90` (regtest only) so refunds didn't wait hours.

| Step | Where | Result |
|---|---|---|
| Load key | `/console`: `organiser.key` loaded as a non-extractable WebCrypto key | Signed requests accepted through the same-origin proxy |
| Create | `/console/new`: 2 lots, uniform price, reserve 0.01 ZEC, 3-hour window | Draft created, close height 631 computed from the tip (439), three one-time invites shown |
| DKG | Three CLI trustees, run **in parallel** | Exposed a deadlock in `dkg/complete` (fixed: DKG writes lock the auction row first). Resumed and completed, same fingerprint everywhere |
| Open | `/console/NKZNE8FM69`: **Open for bids**, then confirm | `open` |
| Fee buffer | Memo `SF1FEE:NKZNE8FM69`, 0.001 ZEC | Recorded as `fee_buffer`, not a bid |
| Bids | 0.30, 0.12, 0.05, 0.02 ZEC at heights 446–449 | 4 valid bids |
| Close | Mined to 644 | Closed automatically |
| Publish | Console: **Publish result**, then confirm | Price 0.12 ZEC, 2 winners. Refunds planned |
| Refund | 6 notes from 3 bidders, 0.25 ZEC. Trustee 2 signed **in the browser** (`/trustee/NKZNE8FM69`, FROST in WebAssembly), trustee 3 with the CLI. Both showed sighash `d9c16125…` | Confirmed at **645** |
| Payout | 0.24045 ZEC = 0.24 proceeds + 0.001 buffer − 0.00055 fees. Trustee 2 in the browser, trustee 1 with the CLI | Confirmed at **646**, auction `settled` |

The console's detail page then showed 6 of 6 notes sent, the refund transaction at block 645 ("2 of 2 signed"), and "Paid 0.24045 ZEC at block 646" with both txids.

Found and fixed along the way:
- The DKG deadlock above, now covered by a concurrent e2e test that fails 5 out of 5 runs without the fix.
- The trustee page paused polling in a hidden tab, so round 2 waited until the tab was visible again. It now polls in the background.
- Dev wallets created before `transparent-inputs` was enabled weren't recognised. They're now matched by their Orchard key.

## Sealed bids (commit-reveal, task 4.6)

Auction `PT6G5C67G7`: 2 lots, uniform price, reserve 0.01 ZEC. Bidding closed at 838 and reveals were accepted until 982. It was created with **Sealed reveal** in the console. Each sealed bid and each reveal memo was built by the **browser bid flow** (receipt v2 kept in IndexedDB), then paid by the dev wallet.

| Bidder | Hidden bid | Deposit (all anyone saw while bidding was open) | Reveal | Outcome | Refund |
|---|---|---|---|---|---|
| 1 | 0.42 | 0.5 | block 842 | won, pays 0.30 | 0.2001 |
| 2 | 0.30 | 0.5 | block 841 | won, pays 0.30 | 0.2001 |
| 3 | 0.12 | 0.2 | block 840 | lost | 0.2001 |
| 4 | 0.25 | 0.5 | none | `not_revealed` | 0.5 |

- **While bidding was open:** the coordinator's records (and anyone with the escrow's viewing key) showed four deposits of 0.5, 0.5, 0.2 and 0.5 ZEC, with no prices. The auction page hid the Bid button once the tip passed 838 and pointed bidders to the reveal.
- **Close:** the closer waited for 982 + 10. It counted the three reveals, computed the uniform price of 0.30 over revealed prices, and folded each counted 0.0001 ZEC reveal into its bidder's refund.
- **Audit:** `sealforge-audit` rescanned 646–982 with the viewing key and found 7 payments (4 deposits, 3 reveals). **Match** on price, volume, winners, Merkle root and every bid's outcome, refunds included.
- **Verify page:** bidder 1's receipt verified in the browser: included under root `315a3f7d…`, won at 0.3 ZEC, refund 0.2001 ZEC as 2 notes. Bidder 4's receipt reports "not in the published result", with the note that unrevealed deposits are refunded in full.
- **Refunds and payout:** two refund batches (0.2003 and 0.9 ZEC) were confirmed at 996 and 1000, and the 0.5993 ZEC payout at 1007. The auction then settled.

While auditing this run, the chain audit first flagged all 3 bid outcomes. The published refunds (deposit + reveal − paid) were correct, but the auditor compared them with the clearing-only refund (price − paid). The published result now carries `reveal_zat` per bid, and both the chain audit and the browser's quick check account for the deposit and reveal. Tests now cover a coordinator that keeps part of a deposit.
