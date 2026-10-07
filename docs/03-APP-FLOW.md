# 3. App Flow: sealForge

**Purpose:** Show each screen, user path, and result of a tap or click.

Related: [01-PRD.md](01-PRD.md), [04-UI-UX-BRIEF.md](04-UI-UX-BRIEF.md)

---

## 3.1 Entry points

| Entry point | Arrives at | Who |
|---|---|---|
| Direct link to an auction, `/a/{auctionId}` (shared on X/Telegram by the organiser) | Auction page | Bidders |
| Embedded widget on an organiser's site (`<SealForgeAuction id=… />` or `<script>` embed) | Auction card → opens the auction page or the ShieldRamp modal in place | Bidders |
| Home `/` (list of live and recent auctions) | Home | Everyone |
| Audit link `/audit/{auctionId}` (posted with the result) | Audit page | Auditors, journalists, bidders |
| Receipt file (`sealforge-receipt-*.json`) dropped onto `/verify` | Verify page | Bidders |
| Organiser console `/console` (requires the organiser key file) | Console dashboard | Organisers |
| CLI `sealforge …` | Terminal | Organisers, trustees |
| Deep link `/a/{auctionId}?ramp=1` | Auction page with ShieldRamp open | Cross-chain bidders |

## 3.2 Screen inventory

| # | Screen | Route | Purpose | Required data |
|---|---|---|---|---|
| S1 | Home | `/` | Discover live, upcoming and settled auctions. Explain the privacy promise in one line | `GET /auctions?status=open,upcoming,settled` |
| S2 | Auction page | `/a/{id}` | Lot details, rules, countdown, live bid count, CTAs: **Bid with Zcash** / **Bid with USDC or ETH** | Auction, lot, escrow UA, status, `close_height`, current height, bid count |
| S3 | Bid amount | `/a/{id}/bid` (sheet on mobile) | Enter max bid, see canonical rounding and reserve | Auction rules (reserve, min increment, mode), denomination table |
| S4 | Pay with wallet | `/a/{id}/bid/pay` | Show the ZIP-321 QR, deep link and copyable URI. Wait for detection | Locally built memo + URI. Polling `GET /auctions/{id}/bids/status?commitment=` |
| S5 | Receipt | `/a/{id}/receipt/{commitment}` | Confirm the bid was detected. Download and store the receipt | Receipt (local), bid status from the server |
| S6 | ShieldRamp: connect + quote | modal on S2 | Connect EVM wallet, choose asset, show quote, Linkability Score, rounding | wagmi account, 1Click quote, score inputs |
| S7 | ShieldRamp: wallet backup | modal step | Create the ephemeral Zcash wallet, set a passphrase, back up the seed | WebZjs wallet, passphrase |
| S8 | ShieldRamp: progress | modal / `/ramp/{sessionId}` (local id) | Track swap → ZEC received → shielded → delay → bid placed | 1Click status, WebZjs balance, local session state |
| S9 | My bids | `/me` | List local receipts and ephemeral wallets, refund progress | IndexedDB receipts, per-auction public status |
| S10 | Verify | `/verify` and `/a/{id}/verify` | Load a receipt, show the inclusion proof, win or lose, refund status | Receipt, published Merkle root + leaves |
| S11 | Audit | `/audit/{id}` | Paste or prefill the IVK, rescan in the browser (WASM), compare with the published result | IVK, params, block range, published root and result |
| S12 | Ephemeral wallet | `/wallet` | Balance, export seed, send out (shielded) or cash out (stretch) | WebZjs wallet |
| S13 | Console: dashboard | `/console` | The organiser's auctions and their states | Organiser key (local file), `GET /console/auctions` |
| S14 | Console: create auction | `/console/new` | Configure lot, mode, quantity, reserve, duration, trustees, payout UA | Form input |
| S15 | Console: auction detail | `/console/{id}` | DKG status, open, close, publish, refund batches, payout | Auction, trustee status, refund plan |
| S16 | Trustee signing | `/trustee/{id}` (or the CLI) | Review a batch (outputs, totals) and sign with the local FROST share | Signing package, local key share file |
| S17 | Docs / How it works | `/how-it-works` | Threat model in plain words, link to the full doc | Static |
| S18 | Error / not found | `/_error`, `/404` | Recover from bad links | — |

## 3.3 Primary journey (Zcash-native bidder)

**S2 Auction page** → tap **Bid with Zcash** → **S3 Bid amount** → enter amount, tap **Continue** → **S4 Pay with wallet** → scan the QR in Zodl/Zashi and send → (detected) → **S5 Receipt** → tap **Download receipt** → **success: "Bid sealed. Price public after close, you never are."**

After close: notification banner on **S2/S9** → **S10 Verify** → "Included ✓ · You won at 0.42 ZEC" or "Not a winner, refund in progress (3 of 5 notes sent)".

### Secondary primary journeys

- **Cross-chain bidder:** S2 → **Bid with USDC or ETH** → S6 (connect, quote, score) → **Confirm and sign** → S7 (backup) → S8 (progress) → success: "Bid placed privately" → S5 receipt.
- **Organiser:** CLI `sealforge init` → S14 create → trustees run DKG (S16/CLI) → S15 **Open** → … → **Close** → **Publish** → refund batches signed → **Payout** → status `settled`.
- **Auditor:** S11 → (IVK prefilled) → **Rescan** → success: "Match ✓: clearing price, volume, winners and root equal the published values".

## 3.4 Action specifications

| Screen · Action | Trigger | Validation | Loading state | Success state | Error state | Next screen |
|---|---|---|---|---|---|---|
| S2 · Bid with Zcash | Tap primary CTA | Auction `open` and current height < `close_height` | — | — | Closed: CTA disabled with "Bidding closed at block N" | S3 |
| S2 · Bid with USDC/ETH | Tap secondary CTA | Auction `open`, ShieldRamp enabled, ≥ 2 h before close (time needed for the delay) | — | — | < 2 h to close: disabled with "Too close to the deadline for a private cross-chain bid" | S6 |
| S3 · Continue | Tap button / Enter | Amount ≥ reserve, ≤ 21M, at most 8 decimals, canonical rounding accepted or overridden with a warning | Button spinner while generating a fresh refund UA (only when using the ephemeral wallet) | — | Inline field error (e.g. "Below the 0.01 ZEC reserve") | S4 |
| S4 · Generate payment | Screen mount | Memo ≤ 512 bytes. Refund UA is a valid Ironwood UA | Skeleton QR | QR + URI + "Open in wallet" deep link | "Could not build payment request" + retry | stays |
| S4 · Copy URI | Tap copy | — | — | Toast "Copied" | Clipboard denied: select the text manually | stays |
| S4 · Wait for detection | Poll every 10 s | Commitment matches | "Waiting for your payment… (usually < 1 min after it confirms)" | Auto-advance | After 30 min: "Not seen yet" help panel (check memo, check network, payment still valid until close) | S5 |
| S5 · Download receipt | Tap | — | — | `sealforge-receipt-{auction}-{short}.json` saved. Toast | Download blocked: show JSON to copy | S5 |
| S6 · Connect wallet | Tap | Supported chain (Ethereum, Base, Arbitrum) | Wallet modal | Address chip | Rejected: "Connection cancelled" | stays |
| S6 · Get quote | Enter amount / asset change (debounced 500 ms) | Amount ≥ 1Click minimum. Balance sufficient | Quote skeleton + "Fetching route" | Quote, fees, ETA, Linkability Score, rounding suggestion | Quote failed: retry. Insufficient balance: inline | stays |
| S6 · Confirm and sign | Tap | Score acknowledged (checkbox for Medium/High). Quote not expired | Wallet signature pending | — | Rejected: back to quote. Expired: auto re-quote | S7 |
| S7 · Back up wallet | Tap "I've saved it" | Passphrase ≥ 12 chars. Seed confirmation (3 random words) | — | Wallet saved (encrypted) | Wrong words: inline error | S8 |
| S8 · Progress | Auto | — | Step list: Swap sent → ZEC received → Shielded → Waiting (random delay, countdown) → Bid sent → Detected | "Bid placed privately" | Swap failed/refunded: show 1Click refund status. Shield failed: retry button. Tab closed: resumes on return | S5 |
| S10 · Load receipt | Drop file / pick from S9 | Valid receipt JSON, matching auction | "Checking inclusion…" | Included ✓ + outcome + refund progress | Not found: "This bid isn't in the published set". Shows contact and evidence | stays |
| S11 · Rescan | Tap | IVK parses. Block range ≤ published range | Progress bar (blocks scanned / total), cancel | Match ✓ table | Mismatch: diff table (field, published, recomputed). Node unreachable: switch endpoint | stays |
| S14 · Create auction | Submit | Required fields. 3 trustee public keys. Duration ≥ 1 h. Payout UA valid | Button spinner | Draft created | Field errors. Signature invalid: "Organiser key not recognised" | S15 |
| S15 · Open | Tap, confirm dialog | DKG complete. Fee buffer funded | Spinner | Status `open`, public page live | "DKG incomplete (2/3)" | S15 |
| S15 · Close | Automatic at `close_height`, or manual early close (only if allowed by the rules) | Height reached | "Closing book…" | Status `closed`, result computed | Retry | S15 |
| S15 · Publish | Tap, confirm | Result computed | Spinner | Status `published`, audit link copied | Retry | S15 |
| S16 · Sign batch | Tap "Sign" | Package hash matches what the CLI shows. Local share decrypts | "Signing…" | "Signature share submitted (2/3)" | Wrong passphrase. Package changed: abort | S15 |

## 3.5 Alternate journeys

| Journey | Path |
|---|---|
| **Cancel bid before paying** | S4 → **Cancel** → S2. Nothing was sent, so nothing to undo (the receipt draft is discarded) |
| **Edit bid amount** | S4 → **Change amount** → S3 (generates a new commitment and memo). A payment already sent counts as a separate bid. Warned in S4 |
| **Raise a bid** | Send a second bid. Both are counted as separate units (documented. Multi-unit merge is out of scope) |
| **Return later** | S9 lists receipts. A ShieldRamp session resumes from IndexedDB at the step it stopped |
| **Skip receipt download** | Allowed. The receipt stays in IndexedDB with a persistent "Not backed up" badge in S9 |
| **Skip ShieldRamp delay** | Not allowed below 20 min. The user can extend it up to 6 h for a better score |
| **ShieldRamp swap refunded by Intents** | S8 shows "Swap refunded to your EVM wallet". No bid placed |
| **Bid arrives late** | S5 shows "Arrived after close, full refund scheduled" |
| **Organiser aborts before open** | S15 → **Delete draft** |
| **Organiser cancels an open auction** | Allowed only before the first bid. After that, cancellation = close with no winners and full refunds |

## 3.6 Navigation rules

- **Top bar:** logo (→ `/`), Auctions, Verify, How it works, `My bids` (badge with active receipts). Console link only when an organiser key is loaded.
- **Mobile:** the same items in a bottom sheet menu. The bid flow (S3–S5) is a full-screen sheet with its own back button.
- **Back behaviour:** S3 → S2. S4 → S3 (with the "payment may already be sent" warning if the user waited > 30 s). S5 → S2 (never back into S4, to prevent duplicate payments). ShieldRamp steps can't go back after signing, only forward or "close and resume later".
- **Deep links:** `/a/{id}`, `/a/{id}?ramp=1`, `/a/{id}/receipt/{commitment}`, `/audit/{id}?ivk=…` (the IVK is public after close, so it may appear in the URL), `/verify`. ZIP-321 `zcash:` URIs open the user's wallet.
- **URL privacy:** commitments, refund UAs and amounts never appear in URLs or query strings that reach the server, except the commitment on the client-only receipt route (rendered from IndexedDB, not fetched by commitment).

## 3.7 Empty and blocked states

| State | Where | What the user sees |
|---|---|---|
| No auctions | S1 | "No live auctions right now." + link to "Run your own auction" guide |
| No bids yet | S2 | "Be the first sealed bid." Bid count hidden until ≥ 5 bids (to avoid leaking early activity) |
| No receipts | S9 | "Your sealed bids will appear here. They're stored only on this device." |
| Auction not found | S2 | 404 with search |
| Result not yet published | S10/S11 | "Results publish after close (block N, ~time)." |
| Wallet connection denied | S6 | "Permission denied in your wallet. Try again or use a Zcash wallet instead." |
| Unsupported browser for WebZjs (no WASM threads / IndexedDB) | S6 | "ShieldRamp needs a modern browser. Use Chrome, Firefox or Safari 17+." |
| Offline | Global | Banner "You're offline. Bids already sent are safe. We'll refresh when you're back." ShieldRamp pauses timers safely |
| Node/coordinator unreachable | S2/S4 | "Live status is delayed." The QR still works (payment doesn't depend on our server) |
| Private browsing (IndexedDB unavailable) | S4/S7 | Warning: "Receipts can't be saved in private mode. Download it now." ShieldRamp is blocked |

## 3.8 First-use journey

**Bidder (first visit):**
1. S2 shows a one-line explainer ("Your bid is sealed in Zcash's shielded pool. After close, anyone can verify the price, nobody can see who bid.") and a "How it works" link.
2. On first **Bid**, a 3-card intro (dismissible, shown once): *Sealed* → *Auditable* → *Refunds arrive in pieces, on purpose*.
3. No account. The first receipt creates local storage. S9 explains "stored only on this device" and nudges to download.

**Cross-chain bidder (first ShieldRamp use):**
1. Explains in one card what happens (swap → private wallet in your browser → delayed bid).
2. Shows the Linkability Score legend once.
3. Mandatory seed backup + passphrase before any funds move.

**Organiser (account setup = key setup):**
1. `cargo install --git https://github.com/Udoka-AM/sealForge sealforge-cli && sealforge init` creates `~/.sealforge/organiser.key` and registers the public key with the coordinator (invite code for the hackathon instance).
2. `sealforge auction new` (or S14) creates the draft and emails/sends trustees an invite code out of band.
3. Each trustee runs `sealforge trustee join <invite>` → DKG rounds → done. S15 shows ✓ for each trustee.
4. Fund the fee buffer → **Open**.
