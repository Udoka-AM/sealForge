# 7. Guides

Short, role-by-role. The step-by-step setup with commands is [QUICKSTART.md](../QUICKSTART.md). The in-browser wallet contract is [WALLET-INTEGRATION.md](../WALLET-INTEGRATION.md). Neither is repeated here. Testnet only: no mainnet auction has run.

## Organiser

Status: `Testnet` (CLI and `/console`).

1. Register: `sealforge init --invite <code> --name <name>` creates `~/.sealforge/organiser.key` (owner-only), or load a key in `/console`.
2. Describe the auction in a TOML file (`examples/auction.toml`): mode (`uniform_price` | `first_price`), quantity, reserve, close height, payout UA (Orchard receiver required), three trustee labels. For sealed-reveal, set `bid_mode = "commit_reveal"` and `reveal_close_height ≥ close_height + 144`.
3. `sealforge auction new <file>` prints three one-time trustee invites. Send each over a private channel.
4. After the trustees finish the DKG, `sealforge auction open <CODE>`. The close height must be at least the tip + 144 ([2.2 Lifecycle](02-protocol/lifecycle.md#height-rules)).
5. Optional: `sealforge auction fund-fees <CODE>` prints a ZIP 321 request for a fee buffer (`SF1FEE`). Useful if the auction may end with no winners.
6. After close, **you must publish**: `sealforge auction publish <CODE>`. Refunds don't start until you do.
7. The payout (proceeds plus unspent fee buffer plus refund dust, minus fees) goes to your payout UA after the last refund confirms.

Choosing a mode: deposit mode is simpler for bidders (one payment) but the coordinator sees amounts. Sealed-reveal hides prices from the coordinator until the reveal window, at the cost of a second payment per bidder and a longer auction ([2.1 Roles](02-protocol/roles.md)).

## Trustee

Status: `Testnet`. Code: `crates/sealforge-cli/src/trustee.rs`, browser `/trustee/<CODE>` (`crates/sealforge-signer-wasm`).

1. `sealforge trustee join <invite>`, then `sealforge trustee dkg <CODE> --wait`. Your key share is written to your disk (owner-only).
2. **Compare the fingerprint** with the other two trustees on a call or in person. If they differ, stop.
3. When batches are due: `sealforge trustee sign <CODE> --wait`. You commit; if you're one of the first two, you sign.

What you are, and aren't, checking when you sign: you sign a sighash and α per escrow spend. The CLI shows the coordinator's stated output count and total, but **you can't see the transaction's outputs** ([5 Security, trust assumption 3](05-security.md#trust-assumptions)). Treat the coordinator as trusted for what it asks you to sign until trustee-side PCZT verification ships.

**CLI or browser.** The browser trustee page holds your key share in JavaScript memory while it's open, the same trust as the CLI on that machine plus the browser's attack surface. Use the CLI for large auctions. Never run more than one trustee on one machine outside test networks: that defeats the threshold.

## Bidder

Status: `Testnet`.

- **Any shielded wallet:** the bid page shows a ZIP 321 QR (escrow UA, amount, memo). Pay it from a wallet that supports ZIP 321 and memos and holds Ironwood or Orchard funds. Use a **fresh refund address** with an Orchard receiver. It becomes readable to everyone after publish.
- **In-browser wallet:** `/wallet` creates a 24-word phrase and a passphrase-encrypted vault in your browser, and gives each bid a fresh refund address ([WALLET-INTEGRATION.md](../WALLET-INTEGRATION.md)).
- **Keep the receipt.** It holds your salt, the only copy. In sealed-reveal mode you need it to reveal. Without a reveal, your deposit is refunded as `not_revealed`.
- **Sealed-reveal:** after the bidding close, send the reveal (10,000 zat, `SF1R` memo) before `reveal_close_height`.
- **Verify** at `/verify` once the result is published: inclusion, outcome and refund plan, checked in your browser.
- **Refunds** arrive as several canonical notes at different times. Don't move them all out at once ([2.7](02-protocol/refunds.md#known-residuals)).

## Auditor

Status: `Testnet`. See [6 Verification](06-verification.md#audit-any-auction). Use a lightwallet server you choose, not the auction's. To look for bids outside the published range, widen it with `--from` / `--to`.

## Integrator

- **Embed card** (`Tested`, manually): add `<script src="https://<sealforge-site>/embed.js" data-auction="CODE" async></script>`. It renders in a shadow root, sets no cookies, reads only public fields, and opens bidding on the sealForge site in a new tab (`apps/web/public/embed.js`).
- **Card data:** `GET /api/embed/{code}` (`apps/web/src/app/api/embed/[code]/route.ts`), with CORS `*` and a 30 s cache. Fields: `code`, `title`, `network`, `status`, `accepting_bids`, `sealed_reveal`, `mode`, `quantity`, `reserve_zat`, `close_height`, `blocks_left`, `bid_count` (null below 5 bids), `url`, `audit_url`.
- **Your own bid flow:** use `@sealforge/bid` ([8 API](08-api.md#sealforgebid-typescript)) to build commitments, memos and ZIP 321 URIs client-side. Never send the salt to a server.
- **White-labelled audit page:** `Specified`, not built. Today the audit page is served by the sealForge site; anyone can run `sealforge-audit` or embed the WASM auditor themselves.
