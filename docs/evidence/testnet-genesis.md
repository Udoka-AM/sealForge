# Evidence: the first auction on Zcash testnet

**Date:** 2026-10-16. **Chain:** Zcash testnet (NU7, about 25-second blocks) through the public lightwallet server `testnet.zec.rocks`. **Site:** the hosted instance at `https://sealforge.udokaam.dev` (testnet), auction `KE5DFF4PDA`.

Run with `scripts/testnet-demo.sh` from one machine. The organiser **GenesisAuction**, the three trustees **Greg**, **Harry** and **Dmitry**, and seven simulated bidders each had their own keystore or wallet. Each bidder used its own wallet, so each had its own refund address. The bids and reveals used the same memo and receipt format as the browser bid flow. The coordinator ran with `SF_MIN_WINDOW_BLOCKS=24` and `SF_REFUND_DELAY_SECS=20,120`, which are allowed on test networks only, so the auction fits in about an hour.

## The auction

**Genesis Pass:** 4 units, sealed bids (commit-reveal), uniform price, reserve 0.01 TAZ. Bidding closed at block 4,507,184 and reveals at 4,507,214. Escrow `utest1huqeq0fg8xptpdc38k…`, a 2-of-3 FROST key from a DKG the three trustees ran at the same moment (10 seconds).

| Bidder | Deposit | Hidden price | Reveal | Outcome | Paid | Refund |
|---|---|---|---|---|---|---|
| 0 | 0.20 | 0.16 | yes | won | 0.07 | 0.1301 |
| 2 | 0.20 | 0.125 | yes | won | 0.07 | 0.1301 |
| 1 | 0.10 | 0.09 | yes | won | 0.07 | 0.0301 |
| 3 | 0.10 | 0.07 | yes | won | 0.07 | 0.0301 |
| 4 | 0.05 | 0.04 | yes | lost | 0 | 0.0501 |
| 5 | 0.10 | 0.085 | **no** | `not_revealed` | 0 | 0.10 |
| 6 | 0.05 | 0.006 | yes | `under_reserve` | 0 | 0.0501 |

Amounts in TAZ. Refunds include the 0.0001 reveal payment where the reveal counted. While bidding was open, the coordinator and anyone with the viewing key saw only seven deposits, with no prices.

| Step | Result |
|---|---|
| Open | Block 4,507,132. The organiser's 0.001 TAZ fee buffer (`SF1FEE:`) was recorded as such, not as a bid |
| Bids | 7 sealed deposits mined at 4,507,142–4,507,165 |
| Reveals | 6 reveals (`SF1R:`) mined at 4,507,194, after the bidding close and inside the reveal window |
| Close | 5 valid bids. Uniform price 0.07 TAZ (the 4th-highest revealed price), 4 winners, Merkle root `ecb24fda…e8f17380` |
| Publish | Organiser published the result and viewing key |
| Refund 1 | 10 notes, 0.2004 TAZ, co-signed by Greg and Dmitry. Tx `c2598ecd…25f8df74`, confirmed at **4,507,719** |
| Refund 2 | 9 notes, 0.3202 TAZ, co-signed by Dmitry and Harry. Tx `5172b8b1…2188c4c2`, confirmed at **4,507,724** |
| Payout | 0.2794 TAZ to the organiser (0.28 proceeds plus the 0.001 buffer, minus 0.0016 of escrow fees), co-signed by Greg and Harry. Tx `7a53a321…757d4657`, confirmed at **4,507,744** |
| Settled | Block 4,507,746 |

Every refund and payout is a FROST 2-of-3 spend from the escrow. Each one used a different pair of trustees, chosen by whichever two committed first.

## Audits

- **Command line:** `sealforge-audit --published <result> --endpoint https://testnet.zec.rocks:443` scanned 4,507,132–4,507,214 with only the published viewing key. It found 14 payments (7 deposits, 6 reveals, the fee buffer). **Match** on price, volume, winners, Merkle root and every bid's outcome.
- **Browser:** **Rescan the chain in my browser** on `/audit/KE5DFF4PDA` downloaded the same blocks through `sealforge-lwd.udokaam.dev` (gRPC-web) and decrypted them in WebAssembly. Result: "Every published figure matches".

## Every wallet afterwards

| Wallet | Funded | Ended with | Difference |
|---|---|---|---|
| Bidder 0 | 0.2004 | 0.1302 | paid 0.07, fees 0.0002 |
| Bidder 2 | 0.2004 | 0.1302 | paid 0.07, fees 0.0002 |
| Bidder 1 | 0.1004 | 0.0302 | paid 0.07, fees 0.0002 |
| Bidder 3 | 0.1004 | 0.0302 | paid 0.07, fees 0.0002 |
| Bidder 4 | 0.0504 | 0.0502 | fees 0.0002 |
| Bidder 5 | 0.1004 | 0.1003 | fee 0.0001 (no reveal) |
| Bidder 6 | 0.0504 | 0.0502 | fees 0.0002 |
| GenesisAuction payout | 0 | 0.2794 | |

Escrow: 0.8016 in (deposits, reveals, buffer) and 0.8000 out (0.5206 refunds plus 0.2794 payout). The other 0.0016 went to ZIP-317 fees, paid from the organiser's side.

## Timing

The plan fits in about 55 minutes. Setup took 2 minutes, then about 22 minutes of bidding (52 blocks), 12 minutes of reveals (30 blocks) and 4 minutes of close confirmations (10 blocks). Publishing to settled took 15 minutes.

This run took longer on the wall clock. The reveals were in by 04:58 UTC. At 05:00 UTC, before the reveal window ended, the host laptop's lid was closed and it slept (`pmset` log: "Clamshell Sleep"). The coordinator couldn't reach the lightwallet server until the machine woke at 08:47 UTC. It closed the auction within seconds of waking, and the rest took the expected 15 minutes. Nothing was at risk: deposits stay in the escrow until two trustees sign, so a stopped coordinator only delays an auction. A real deployment runs on a server (`infra/README.md`).

## Found and fixed while getting here

A dry run on a private testnet coordinator (`EFKHFHRY9W`) came first. Three coordinator bugs only showed up when trustees acted at the same moment, or when an auction had no winners. Each fix landed with a test that fails without it:

1. **Trustees joining at once left the auction in draft.** Each join counted empty slots without seeing the other joins, so none started the DKG. Joins now lock the auction row. The e2e test joins all three concurrently (fails 3 out of 3 without the fix).
2. **Trustees signing at once stalled the batch.** Two shares posted in the same millisecond each saw only themselves, so the batch never became `signed`. Commits and shares now lock the batch row. The scanner also re-runs the idempotent transitions, so a stalled batch recovers. The e2e test posts both shares concurrently (fails 3 out of 3 without the fix).
3. **An auction without winners never settled.** No proceeds meant no payout, and settling waited for a payout. The fee buffer is now swept back to the organiser and the auction settles. The dry run used this path: its 0.001 TAZ buffer came back as 0.0007 after fees.

The dry run also confirmed that PCZT and FROST escrow spends work on NU7. Its refund confirmed at 4,507,093 and its payout at 4,507,121.

## Reproduce

```bash
SF_HOST_NETWORK=testnet scripts/host-local.sh start      # with ~/.sealforge-host/testnet/demo.env
./target/release/sealforge --coordinator http://127.0.0.1:8090 --home <organiser-dir> init --invite <code> --name GenesisAuction
scripts/testnet-demo.sh <plan.conf>                       # plan format at the top of the script
```
