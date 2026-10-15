# Demo video script (≤ 3 minutes, draft)

Screen recording of the web app and two terminals. Narration is in quotes. Times are cumulative.

| Time | Screen | Narration |
|---|---|---|
| 0:00–0:15 | Home page, then an open auction page | "Every public sale leaks who paid what. sealForge runs sealed-bid auctions on Zcash: the price is public, the bidders aren't, and you don't have to trust us." |
| 0:15–0:40 | Console: create an auction with **Sealed reveal** on. Show the three trustee invites | "An organiser creates an auction and invites three trustees. Each runs key generation on their own machine. The escrow can only be spent by any two of them." |
| 0:40–0:55 | Terminal: three `sealforge trustee dkg … --wait`, matching fingerprints. Console shows the escrow, then **Open for bids** | "The coordinator never holds a key share. It can't move a single zatoshi." |
| 0:55–1:20 | Bid flow on a phone-sized window: hidden bid 0.42, deposit 0.5, QR, wallet pays. Receipt saved | "A bidder scans one QR from any Zcash wallet. In sealed mode they deposit a budget and the memo commits to their real bid. Nobody, not even the trustees, can see it." |
| 1:20–1:35 | Close passes. My bids → **Reveal my bid** QR | "After the close, bidders reveal on chain. Unrevealed bids just get their deposit back." |
| 1:35–1:55 | Console: **Publish result**. Audit page: price, winners, root, viewing key | "The organiser publishes the result with the escrow's viewing key and a Merkle root of every bid." |
| 1:55–2:20 | Terminal: `sealforge-audit --published … --endpoint …` → all ✓ **Match**. Then a tampered copy → ✗ **MISMATCH** | "Anyone rescans the chain with that key, from any node, and recomputes everything. Change one number and the audit fails." |
| 2:20–2:40 | Verify page: drop the receipt → "Your bid is included · Won at 0.3 ZEC · refund 0.2001 in 2 notes" | "Each bidder proves their own bid was counted, in their browser." |
| 2:40–2:55 | Trustee page signing a refund batch; console shows refunds confirmed and the payout; status **Settled** | "Refunds go out in standard pieces, at random times, mixed across bidders. Two trustees sign each batch. Then the organiser is paid." |
| 2:55–3:00 | Repo README | "sealForge. Open source, on Zcash today." |

Recording notes:
- Record on the hosted testnet (or mainnet) instance once it's up; the local regtest stack works for rehearsal (`scripts/regtest.sh`, `SF_REFUND_DELAY_SECS=20,90`).
- Pre-create the trustee keystores and keep a funded wallet ready, so the waits don't make the cut.
