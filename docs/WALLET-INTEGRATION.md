# The in-browser wallet and the bidder journey (task 4.7)

How the bidder's wallet fits into sealForge without changing what already works. Written before the web integration, and kept as the compatibility contract.

## What exists, and stays as it is

| Component | Role | Contract the wallet relies on |
|---|---|---|
| `sealforge-core` | Pure auction logic: memos, commitments, clearing, refund denominations | `SF1:` bid memo, `SF1R:` reveal memo, `price_commitment`, canonical refund notes. Used through `@sealforge/bid` (TypeScript, same test vectors). **Unchanged** |
| Coordinator (`sealforge-coordinator`) | Scans each escrow, closes, publishes, plans and batches refunds, runs FROST signing rounds | Public reads only: `GET /v1/auctions`, `/v1/auctions/{code}`, `/result`, `/refunds`, `/v1/chain`, through the site's proxy. **No new endpoints, no new data sent** |
| Escrow (`sealforge-escrow`, `sealforge-chain::spend`) | 2-of-3 FROST escrow, PCZT spends | Bids are ordinary shielded payments to the escrow UA with the bid memo. **Unchanged** |
| Auditor (`sealforge-auditor`, WASM) | Recompute results; receipt inclusion proofs | `checkReceipt(result, receipt)`. **Unchanged** |
| Scanner (`sealforge-scan`, WASM) | Browser chain audit | **Unchanged** |
| Receipts (`apps/web/src/lib/receipts.ts`) | IndexedDB, never sent to a server | Same `StoredReceipt` shape. Two optional fields added: `wallet_address_index` (which wallet address the refund goes to) and `paid_txid` (the wallet payment that funded it). Old receipts work unchanged |
| Bid flow (`components/bid-flow.tsx`) | Amount → ZIP-321 QR → receipt | The QR / external-wallet path stays the default for anyone without the built-in wallet |
| gRPC-web (`lib/grpc-web.ts`, Envoy) | Compact blocks for the browser audit | Adds `GetLatestBlock`, `GetTreeState` and `SendTransaction` beside the existing calls |

## What's new

**`crates/sealforge-wallet`** (Rust → WebAssembly, `scripts/build-wallet-wasm.sh`). Built only from librustzcash crates at the versions the escrow already uses (`docs/VERSIONS.md`). No primitive is implemented here.

| Need | Provided by |
|---|---|
| Recovery phrase | BIP-39 (`bip0039`), 24 words from 32 bytes of `crypto.getRandomValues` |
| Keys and addresses | ZIP 32 via `zcash_keys::UnifiedSpendingKey::from_seed`. A fresh diversified Orchard address per bid |
| Finding notes | Compact-block trial decryption (`orchard`, `zcash_note_encryption`), external and change scopes |
| Witnesses | `incrementalmerkletree` from the birthday's tree state (`GetTreeState`). Tree size is checked against every block's `chain_metadata` |
| Spends seen | Nullifiers in compact actions |
| Payments | `zcash_primitives` builder (Ironwood), then the `pczt` IO finalizer, prover, signer and extractor, the same roles as the escrow's spends |

The wallet holds Ironwood notes and spends them. Orchard-pool notes it receives are counted and labelled, but not spent.

**Browser layer** (`apps/web/src/lib/wallet/`):
- The WASM runs in a Web Worker (`public/wallet/worker.js`), so proving doesn't freeze the page.
- The phrase and the wallet state are encrypted with WebCrypto (PBKDF2-SHA-256, 600,000 iterations, then AES-GCM-256) under the user's passphrase and stored in IndexedDB.
- Nothing about the wallet is ever sent to the sealForge server. The lightwallet server sees which blocks are requested and transactions being broadcast, as with any light wallet.

## Evidence rules for every status shown

| Status | Shown only when |
|---|---|
| Payment confirmed | The wallet saw the payment's nullifiers in a mined block (height shown). External-wallet payments can't be seen by the page: they show "Paid from another wallet: the bid appears in the published result". |
| Bid counted | The published result contains the receipt's commitment and its Merkle proof checks in the browser (`checkReceipt`). |
| Won / lost / refund due | From the published result, through `checkReceipt`. |
| Refund received | Notes that arrived at that bid's wallet refund address, mined (height shown). Compared with the refund due. |
| Refunds in progress | The auction's public refund progress (`/refunds`): aggregate counts, never per bid. |
| Settled | Auction status `settled` from the coordinator. |
| Balance | Sum of the wallet's unspent notes it has decrypted. Pending payments are listed separately until they confirm or expire. |

When something can't be known, the page says so. Examples: the coordinator is unreachable, there's no lightwallet endpoint, cross-chain funding isn't available, or a bid was paid from another wallet.

## Cross-chain funding (ShieldRamp)

`/api/shieldramp/quote` returns 503 (not configured) or 501 (not built). The funding panel asks it and shows cross-chain funding as **unavailable** until it returns a quote. Native ZEC funding never depends on it.

## Privacy notes shown to users

- The phrase and spending key never leave the browser. Losing the phrase and the device loses the funds.
- Each bid gets a fresh refund address, so refunds can't be linked to each other or to the funding address. The amounts and timing of the funding payment can still link a bidder to their deposits. Funding well before bidding, in round amounts, helps.
- The lightwallet server learns the IP address and when the wallet syncs. Use one you trust, or Tor.
- Paying a bid from the wallet reveals nothing extra to the auction server: the bid is the same shielded payment and memo as from any wallet.

## Verified (2026-10-17, testnet)

- **Native harness** (`examples/wallet_testnet.rs`, same code as the browser): synced, then paid 0.035 TAZ spending two notes. The network accepted it, the wallet saw it confirm at block 4,510,054, and the exact change came back.
- **Browser, full journey** (private coordinator, sealed auction `VRQA54RNWG`), each status from on-chain evidence:
  1. Restored a wallet from its phrase twice: once from an exact birthday, once from "last day" (about 3,500 blocks in about 15 s).
  2. Unlocked it inline and bid 0.004 TAZ hidden, 0.005 deposit. The refund went to a fresh wallet address.
  3. Paid the deposit from the wallet: proved in the worker, confirmed at 4,510,116 (the coordinator recorded it at the same height).
  4. Revealed from the wallet, confirmed at 4,510,148.
  5. The result was published at a 0.004 TAZ price. The Merkle proof checked in the browser, and the bid won.
  6. The 0.0011 TAZ refund arrived at the bid's address in 2 notes (4,510,169 and 4,510,177). My Bids showed "Won · settled".
- **Production build** on `sealforge.udokaam.dev` (production CSP, public gRPC-web): created a wallet (phrase, three-word check, passphrase rules), and it synced.
- **Errors:**
  - A wrong passphrase, a wrong confirmation word and a short passphrase are all refused.
  - A transient 503 from the lightwallet server is shown, then retried.
  - The vault in IndexedDB holds only ciphertext.
  - No wallet data reaches the sealForge server: only public GETs, checked in the network log.
- **Tests:**
  - `sealforge-wallet`: reorg, gap and tree-size refusals; payment refusals; address counter.
  - `@sealforge/bid`: `receiptMemo` reproduces the original memo.

## Known limits

- Ironwood notes only are spent. Orchard-pool notes are shown, not spent.
- No mempool view: incoming funds appear once mined. Paying is blocked during the first sync of a restore.
- Payments from another wallet can't be seen by the page until the result is published.
- The wallet locks on every full page load, because keys live only in memory. In-app navigation keeps it unlocked.
- Proving takes about 30–40 s for the first payment in a page (building the proving key), then a few seconds.
- ShieldRamp (USDC/ETH) is not built. The UI says so.
