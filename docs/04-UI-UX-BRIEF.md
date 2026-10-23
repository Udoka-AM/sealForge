# 4. UI and UX Design Brief: sealForge

**Purpose:** Give the app a consistent, usable visual direction.

Related: [03-APP-FLOW.md](03-APP-FLOW.md)

---

## 4.1 Audience and tone

**Audience:** privacy-literate crypto users (Zcash community, NFT collectors, DeFi users arriving from EVM), auction organisers, and sceptical auditors and journalists. They read hashes, they distrust marketing, and many arrive on mobile from an X or Telegram link.

**Three design adjectives:** **Sealed** (calm, closed, nothing leaks), **Verifiable** (numbers, proofs and sources always one tap away), **Precise** (exact amounts, block heights, no hype).

**Voice:** plain, specific and honest about limits. Say "Your USDC deposit is public. We reduce the link, we don't remove it." Never "untraceable", "anonymous forever" or "100% private".

## 4.2 Reference products

| Product | Borrow | Avoid |
|---|---|---|
| [Zashi / Zodl wallet](https://electriccoin.co/zashi/) | Calm shielded-first language, the yellow accent used sparingly | Wallet-dense settings screens |
| [Uniswap swap widget](https://app.uniswap.org) | Compact quote card, clear fee and route breakdown | Token-list clutter |
| [Etherscan](https://etherscan.io) / [CipherScan](https://cipherscan.app) | Monospace hashes with copy buttons, block-height-as-time | Ad-heavy, dense tables on mobile |
| [Linear](https://linear.app) | Quiet dark UI, hairline borders, strong type hierarchy | Over-animated marketing pages |
| [Stripe Checkout](https://checkout.stripe.com) | Single-column payment flow, one primary action per step | Brand-heavy upsells |
| [Gnosis Safe](https://app.safe.global) | Signer progress ("2 of 3 signed") and transaction review | Jargon-heavy owner screens |

## 4.3 Color palette

Dark theme is the default (crypto audience, night use). Light theme has full parity. All colors are CSS variables in `apps/web/src/app/globals.css`.

| Token | Dark | Light | Use |
|---|---|---|---|
| `--bg` | `#0B0D10` | `#FAFAF7` | Page background |
| `--surface` | `#13161B` | `#FFFFFF` | Cards, sheets |
| `--surface-2` | `#1B1F26` | `#F2F1EC` | Inputs, nested panels |
| `--border` | `#262B33` | `#E3E1DA` | Hairlines |
| `--text` | `#ECEAE4` | `#16181C` | Primary text |
| `--text-muted` | `#9AA0A9` | `#5D636C` | Secondary text, labels |
| `--primary` (Seal gold) | `#F4B728` | `#B9850A` | Primary buttons, key numbers, focus ring (Zcash yellow, used sparingly) |
| `--primary-ink` | `#0B0D10` | `#FFFFFF` | Text on primary |
| `--accent` (Wax red) | `#D9573B` | `#B8432A` | The reveal phase (phase track, block clock). Never for errors |
| `--success` | `#3FB37F` | `#1F8A5B` | Included ✓, match ✓, refund paid |
| `--warning` | `#E5A50A` | `#A86F00` | Medium linkability, pending signatures |
| `--danger` | `#E5484D` | `#C62F35` | Errors, High linkability, mismatches |
| `--info` | `#3E8EDE` | `#2369B3` | Neutral notices, links |

**Linkability Score scale:** Low = `--success`, Medium = `--warning`, High = `--danger`, always paired with a text label and an icon (never color alone).

Contrast: all text pairs ≥ 4.5:1. Gold on `--bg` is 10.6:1. `--text-muted` on `--surface` ≥ 4.6:1.

## 4.4 Typography

| Role | Font | Size / line-height | Weight |
|---|---|---|---|
| Display (auction title, big numbers) | **IBM Plex Sans** | 40/44 (mobile 32/36) | 600 |
| H1 | IBM Plex Sans | 28/34 | 600 |
| H2 | IBM Plex Sans | 22/28 | 600 |
| H3 | IBM Plex Sans | 18/24 | 600 |
| Body | IBM Plex Sans | 16/24 | 400 |
| Small / labels | IBM Plex Sans | 14/20 | 500 |
| Caption | IBM Plex Sans | 12/16 | 500, +0.02em tracking |
| Numbers, hashes, addresses, block heights | **IBM Plex Mono** | inherits size | 400/500, `tabular-nums` |

Why Plex: an engineered, institutional voice that fits "auditable", and a mono sibling for hashes that matches metrics exactly. Loaded via `next/font` (self-hosted, no Google requests at runtime, which matters for privacy).

Scale ratio ≈ 1.25. Max reading width 68ch.

## 4.5 Components

| Component | Spec |
|---|---|
| **Button** | Variants: primary (gold fill), secondary (surface + border), ghost, danger. Sizes: md 44 px, lg 52 px (mobile primary CTAs). Radius 10 px. One primary per screen |
| **Input / amount field** | 48 px height, mono digits, unit suffix (ZEC/USDC), inline helper and error text below, rounding chip ("Round to 0.5 ZEC") |
| **QR card** | QR (≥ 240 px, quiet zone), "Open in wallet" button, copyable URI collapsed by default, memo preview in mono |
| **Hash / address chip** | Mono, middle-ellipsis (`u1qz…9f3k`), copy button, full value in tooltip and on long-press |
| **Auction card** | Title, lot image, status pill, countdown (block height + estimated time), bid count (hidden < 5) |
| **Status pill** | draft · ready · open · closed · published · refunding · settled, each with a fixed color + icon |
| **Countdown** | "Closes at block 3,412,000 · ~4 h 12 m", updates each block |
| **Linkability Score card** | Level badge, 3–5 reason rows (each: factor, impact, mitigation applied), "What does this mean?" disclosure |
| **Stepper (progress)** | Vertical on mobile, each step with state icon, timestamp, and an explanation for waits ("Random delay protects you: 37 min left") |
| **Receipt card** | Auction, commitment (chip), amount, status, Download + Verify actions, "Not backed up" badge |
| **Match table (audit)** | Field · Published · Recomputed · ✓/✗, mono numbers |
| **Signer progress** | "2 of 3 trustees signed" with avatars or initials |
| **Alerts / toasts** | Inline alert (info/warn/danger/success), toast for transient confirmations (bottom on mobile, top-right on desktop), 4 s |
| **Navigation** | Top bar (desktop), compact top bar + menu sheet (mobile), bid flow as full-screen sheet |
| **Modal / sheet** | ShieldRamp runs in a modal on desktop and a full-screen sheet on mobile |
| **Seal mark** | The logo, and the signature detail that "stamps" closed auctions and verified receipts (one 300 ms animation, respects reduced motion). A gold hexagonal ingot with an angular S cut through it: the S is a mirrored Z (for Zcash) with half-turn symmetry. Lit top facet, shaded bottom facet; from 40 px up, a broken outer hexagon. Geometry in `apps/web/src/lib/brand.ts`; favicon `app/icon.svg`, home-screen icon `app/apple-icon.tsx` |

## 4.6 Layout rules

- **Grid:** 12 columns, 24 px gutters on desktop. 4 columns, 16 px gutters on mobile. Max content width 1120 px. Bid flow max width 480 px.
- **Spacing scale (px):** 4, 8, 12, 16, 24, 32, 48, 64. Section spacing 48 (desktop) / 32 (mobile).
- **Breakpoints:** `sm` 640, `md` 768, `lg` 1024, `xl` 1280. Design mobile first at 375 px.
- **Radius:** 6 (chips), 10 (buttons, inputs), 14 (cards), 20 (sheets).
- **Elevation:** hairline borders, not shadows. A single soft shadow is reserved for modals.
- **Density:** comfortable on bidder screens, compact (14 px tables) on console and audit screens.

## 4.7 Screen notes

| Screen | Hierarchy (top → bottom) |
|---|---|
| S1 Home | One-line promise ("Sealed bids. Public price. Private bidders.") → live auctions grid → recently settled (with "Audit" links) → "Run your own auction" |
| S2 Auction | Lot title + image → status + countdown → rules summary (mode, units, reserve) → primary CTA **Bid with Zcash**, secondary **Bid with USDC/ETH** → privacy explainer → escrow + trustees (chips) → after close: result block (price, volume, winners, root) + **Audit this result** |
| S3 Bid amount | Amount field (large) → rounding suggestion → reserve and rules reminder → Continue |
| S4 Pay | QR (dominant) → "Open in wallet" → amount + memo preview → waiting indicator → cancel / change amount |
| S5 Receipt | Seal stamp + "Bid sealed" → receipt card → Download (primary) → what happens next (timeline) |
| S6 ShieldRamp quote | Asset + amount → quote (you pay, you bid, fees, ETA) → **Linkability Score card** → acknowledgement → Confirm and sign |
| S8 Progress | Stepper → current step explanation → "You can close this tab, we'll resume" |
| S10 Verify | Receipt drop zone → inclusion result → outcome (won/lost, price paid) → refund progress (notes paid / total) |
| S11 Audit | Inputs (IVK, range, prefilled) → Rescan → progress → match table → "How this works" + download the CLI |
| S15 Console detail | Status + next action (single primary) → trustees and DKG → live stats → refunds (batches, signer progress) → payout |

## 4.8 Accessibility

- WCAG 2.2 **AA** minimum. Text contrast ≥ 4.5:1, large text and UI components ≥ 3:1.
- Full keyboard access: logical tab order, visible 2 px gold focus ring with 2 px offset, Esc closes modals, focus trapped in modals and returned on close.
- Every icon button has an `aria-label`. Status pills and scores carry text, not color only.
- QR always has a text alternative (copyable URI + "Open in wallet").
- Live regions (`aria-live="polite"`) for bid detection, scan progress and refund progress.
- Touch targets ≥ 44 × 44 px. Primary mobile CTAs 52 px high.
- Respect `prefers-reduced-motion` (no stamp animation, no countdown ticking animation) and `prefers-color-scheme`.
- Long hashes are readable by screen readers in chunks (`aria-label` with spaced groups).
- Forms: labels always visible (no placeholder-only labels), errors linked with `aria-describedby`.

## 4.9 Interaction states

| State | Treatment |
|---|---|
| Hover | Buttons: +6% lightness. Cards: border → `--text-muted`. Links underline |
| Focus | 2 px `--primary` ring, 2 px offset, never removed |
| Active / pressed | −4% lightness, 98% scale (no scale when reduced motion) |
| Disabled | 40% opacity, `cursor: not-allowed`, plus a reason in helper text or a tooltip ("Bidding closed at block N") |
| Loading | Skeletons for data blocks, inline spinner inside buttons (button keeps its width), progress bars for scans with counts |
| Error | `--danger` border + icon + message that says what happened and what to do next. Never a raw error code alone |
| Success | `--success` icon + short confirmation. Major milestones get the seal stamp |
| Empty | Neutral illustration-free message + one next action |
| Pending (chain) | Pulsing dot + "Waiting for confirmation (~25 s per block)" |

## 4.10 Assets needed

| Asset | Format | Owner / source |
|---|---|---|
| sealForge logo (wordmark + hexagon S mark), light/dark | SVG | Done: `lib/brand.ts`, `app/icon.svg` |
| Favicon + app icons (16–512 px), OG image template | SVG/PNG | Design |
| Icon set | **Lucide** (MIT), 1.5 px stroke | npm `lucide-react` |
| Wallet logos (Zodl, Zashi, MetaMask, Rabby, WalletConnect, Phantom) | SVG | From each project's press kit, used per their brand rules |
| Status and score icons | Lucide subset | — |
| "How it works" diagram (bid → escrow → audit → refund) | SVG | Team C, by Oct 21 |
| Linkability chart (naive vs sealForge) | SVG/PNG from the harness | Team C, by Oct 23 |
| Demo lot artwork (Zcon7 pass) | PNG/WebP | Design, by Oct 19 |
| Screenshots for README and submission | PNG | Team C, Oct 25 |
| Demo video (≤ 3 min) | MP4 | Team C, Oct 26 |
