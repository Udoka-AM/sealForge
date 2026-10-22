# Demo video script (3 minutes or less)

A screen recording of https://sealforge.udokaam.dev (testnet) and one terminal. Narration is in quotes. Times are cumulative.

| Time | Screen | Narration |
|---|---|---|
| 0:00–0:15 | Home page, then an open auction | "Every public sale leaks who paid what. sealForge runs sealed-bid auctions on Zcash: the price is public, the bidders aren't, and you don't have to trust us." |
| 0:15–0:35 | Console: an auction with **Sealed reveal**, the three trustees (Greg, Harry, Dmitry), the escrow address | "An organiser creates an auction and three trustees run key generation on their own machines. Any two of them can move the escrow. The server holds no key share; it can't move a single zatoshi." |
| 0:35–0:55 | **Wallet** page: balance, network status, activity | "No Zcash wallet? There's one built in. Your recovery phrase never leaves the browser, and it proves and signs shielded transactions right here." |
| 0:55–1:20 | Bid page: unlock inline, enter a hidden bid, pick a deposit, choose **My sealForge wallet** for refunds, **Pay** | "I enter my real bid, which stays hidden, and a standard deposit. Refunds go to a fresh address used only for this bid. One tap pays, and the proof is built in my browser." |
| 1:20–1:35 | My bids: **Deposit confirmed in block …**, then after close **Reveal for 0.0002 TAZ** → **Reveal confirmed** | "Every status here comes from the chain: confirmed in a block, not 'probably sent'. After bidding closes I reveal, again in one tap." |
| 1:35–1:55 | Audit page: price, winners, root, viewing key. **Rescan the chain in my browser** → every figure matches | "The organiser publishes the result with the escrow's viewing key. Anyone rescans the chain from any node and recomputes everything." |
| 1:55–2:10 | Terminal: `sealforge-audit` on a tampered copy → ✗ **MISMATCH** | "Change one number and the audit fails." |
| 2:10–2:35 | My bids: **Won at 0.004 TAZ · Merkle proof checks**, refund bar filling, **Won · settled**. Wallet activity: **Refund · …** in two notes | "My bid is in the result, I won at the clearing price, and my refund arrives in standard pieces, at random times, mixed with other bidders' refunds, so nobody can match them to me." |
| 2:35–2:50 | Community Pass audit: 25 bids, 10 winners, match | "We've run this on public testnet: 25 sealed bids, a 2-of-3 escrow, and every refund and payout settled." |
| 2:50–3:00 | README | "sealForge. Open source, on Zcash today." |

## Recording notes

- **Have a funded wallet ready.** Use the hosted site, restore a funded testnet wallet, and keep the tab open. A full reload locks the wallet, which is by design, and a locked wallet needs the passphrase again.
- **Warm the prover before the take.** The first payment in a page takes 30–40 s while the proving key is built. Make one small payment off camera first, or cut the wait.
- **Run a short auction for the take.** Use a private coordinator and `scripts/testnet-demo.sh` with short windows (for example `BID_BLOCKS=24`, `REVEAL_BLOCKS=16`, two scripted bidders). Bid as the third bidder in the browser. Testnet blocks vary from 25 to 75 s, so record each step when it lands and edit the waits out.
- **Pre-record the terminal.** The tampered-copy audit runs against `KE5DFF4PDA`'s published result with one number edited.
- **Keep the host awake.** The Mac must stay plugged in with the lid open while recording, because the site is served from it.
