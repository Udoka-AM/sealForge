# 2.3 Wire formats

Status: `Testnet`, except where a row says otherwise. Example values are from the checked-in vector file `crates/sealforge-core/tests/fixtures/vectors.json` ([Appendix: test vectors](../appendix-test-vectors.md)).

All memos are ZIP 302 text memos in the 512-byte memo field ([ZIP 302](https://zips.z.cash/zip-0302)). On the receive side, text is extracted by `zcash_protocol::memo::Memo::from_bytes`, and only `Memo::Text` is considered (`crates/sealforge-scan/src/lib.rs`, `crates/sealforge-chain/src/wallet.rs`). `sealforge-core` then trims trailing `0x00` bytes before parsing (`trim_end_matches('\0')`), so NUL padding from any wallet is accepted.

## Auction code

| Property | Value | Code |
|---|---|---|
| Alphabet | Crockford base32 without `I L O U`: `0123456789ABCDEFGHJKMNPQRSTVWXYZ` | `crates/sealforge-coordinator/src/codes.rs` `ALPHABET` |
| Length | 10 characters | `sealforge_core::memo::AUCTION_CODE_LEN` |
| Entropy | 10 × log2(32) = **50 bits**, each character drawn uniformly with `OsRng.gen_range(0..32)` | `codes.rs` `random_code` |
| Uniqueness scope | **Per coordinator database**: `auctions.code` is `UNIQUE`; creation retries up to 5 times on conflict (`ON CONFLICT (code) DO NOTHING`). Two coordinators can issue the same code | `migrations/20261007000000_init.sql`; `routes/auctions.rs` `create` |
| Validation | `^[0-9A-HJKMNP-TV-Z]{10}$` | `memo::is_valid_auction_code`, `packages/bid/src/memo.ts` `isValidAuctionCode`, DB `CHECK` |

The code is not a security parameter for funds: a bid only counts if paid to that auction's own escrow address ([2.4](commitments.md#binding)).

## Bid memo: `SF1`

Grammar (ABNF, [RFC 5234](https://www.rfc-editor.org/rfc/rfc5234)):

```text
bid-memo   = "SF1" ":" code ":" commitment ":" refund-ua *%x00
code       = 10crockford
commitment = 64HEXDIG          ; 32 bytes. Encoders emit lowercase; the decoder accepts either case
refund-ua  = 1*(%x01-39 / %x3B-FF)   ; any byte except NUL and ":"; must be a valid UA (checked by the coordinator)
```

| Field | Bytes | Notes |
|---|---|---|
| `SF1` | 3 | Version prefix. Anything else is rejected with `BadPrefix` |
| `:` | 1 | |
| code | 10 | |
| `:` | 1 | |
| commitment | 64 | Hex of the 32-byte commitment. Deposit mode: `receipt::commitment`. Sealed-reveal mode: `receipt::price_commitment` ([2.4](commitments.md)) |
| `:` | 1 | |
| refund UA | variable | Remaining bytes. Must not contain `:` |

Limits: the encoded memo (before padding) MUST be ≤ 512 bytes (`MAX_MEMO_BYTES`). With the fixed 80-byte prefix, the refund UA can be at most 432 bytes. Decoding uses `splitn(4, ':')`, so the refund UA is everything after the third colon, and a refund UA containing `:` is rejected.

The coordinator then checks the refund address: it MUST decode as a Unified Address for the auction's network with an Orchard receiver (`crates/sealforge-coordinator/src/validate.rs` `shielded_ua`). A bid with an unusable refund address still counts; its refund goes to manual review (SECURITY-REVIEW F2).

Code: `sealforge_core::memo::BidMemo::{encode, decode}`; TS `bidMemo` in `packages/bid/src/memo.ts`. Tests: `memo::tests::*`.

## Reveal memo: `SF1R` (sealed-reveal mode)

```text
reveal-memo = "SF1R" ":" code ":" commitment ":" price ":" salt *%x00
price       = "0" / (%x31-39 *DIGIT)   ; canonical decimal zatoshi: no sign, no leading zeros
salt        = 64HEXDIG                 ; 32 bytes
```

- Split on `:` into exactly 5 fields, else `BadShape`.
- `price` MUST be canonical decimal: empty, signed (`-1`), or leading-zero (`042000000`) values are rejected (`reveal_memo_round_trips`). It MUST parse as `u64`. Selection then requires `1 ≤ price ≤ deposit` ([2.5](selection.md#sealed-reveal-mode)).
- The reveal is a separate payment to the escrow. The web flow pays `REVEAL_AMOUNT_ZAT = 10_000` zat (`packages/bid/src/receipt.ts`); a counting reveal's amount is refunded with the deposit (see the testnet tables in [evidence/testnet-genesis.md](../../evidence/testnet-genesis.md)).

Code: `sealforge_core::memo::RevealMemo`; TS `revealMemo`.

## Fee-buffer memo: `SF1FEE`

```text
fee-memo = "SF1FEE" ":" code *%x00
```

The organiser's optional deposit to the escrow for ZIP 317 fees. It is never a bid (`BidMemo::decode` rejects it, test `fee_buffer_memo_round_trips_and_is_not_a_bid`). What isn't spent on fees returns with the payout. Code: `memo::fee_buffer_memo`, `memo::parse_fee_buffer_memo`.

## ZIP 321 payment URI

```text
zcash:<escrow_ua>?amount=<ZEC decimal>&memo=<base64url(memo bytes), no padding>
```

- `amount`: ZEC decimal from integer zatoshi, trailing zeros trimmed (`format_zec`: `100_000_000 → "1"`, `1 → "0.00000001"`).
- `memo`: base64url without padding (RFC 4648 §5), as [ZIP 321](https://zips.z.cash/zip-0321) specifies for the `memo` parameter.
- One URI per bid, single payment. Payable from any wallet that supports ZIP 321 and shielded memos.

Code: `crates/sealforge-core/src/zip321.rs` `payment_uri`; TS `packages/bid/src/zip321.ts`. Tests: `zip321::tests::*`, the TS vector test.

## Receipt JSON

Status: `Testnet`. Created and kept in the browser (IndexedDB, `apps/web/src/lib/receipts.ts`) and offered as a download (`sealforge-receipt-*.json`). Never sent to the coordinator.

| Field | Type | Notes |
|---|---|---|
| `version` | integer | `1` deposit bid, `2` sealed-reveal bid (`packages/bid/src/receipt.ts`) |
| `auction_code` | string | |
| `amount_zat` | integer | Version 1: the bid. Version 2: the deposit |
| `refund_ua` | string | |
| `salt` | hex, 32 bytes | Never sent to the server |
| `commitment` | hex, 32 bytes | |
| `price_zat` | integer, optional | Version 2 only: the hidden price. Omitted when absent (`skip_serializing_if`) |

Rust type: `sealforge_core::receipt::Receipt`. The web app's stored form may add `wallet_address_index` and `paid_txid` ([WALLET-INTEGRATION.md](../../WALLET-INTEGRATION.md)).

## Published result

`GET /v1/auctions/{code}/result` on the coordinator (also proxied by the site at `/api/auctions/{code}/result`). Available once the auction is `published`, `refunding` or `settled`. Type: `sealforge_api::types::PublishedResult` (`crates/sealforge-api/src/types.rs`).

| Field | Notes |
|---|---|
| `auction_code`, `network` | |
| `rules` | `mode`, `quantity`, `reserve_zat`, `bid_mode`, and (sealed-reveal) `close_height` |
| `scan_from_height`, `scan_to_height` | The block range the coordinator scanned. The auditor's default range |
| `uivk` | The escrow's Unified Incoming Viewing Key (ZIP 316) |
| `algorithm_version` | `sealforge-core/<crate version>/clearing-v1` (`clearing::ALGORITHM_VERSION`) |
| `clearing_price_zat` | `null` when no bid met the reserve |
| `total_volume_zat`, `valid_bid_count`, `winner_count` | |
| `merkle_root` | Hex, [2.4](commitments.md#merkle-tree) |
| `bids[]` | Per included bid: `txid`, `output_index`, `block_height`, `amount_zat` (the bid or revealed price), `deposit_zat` and `reveal_zat` (sealed-reveal), `commitment`, `leaf_hash`, `is_winner`, `price_paid_zat`, `refund_due_zat` |

## Versioning policy

- Every memo starts with a version prefix. `SF1`, `SF1R` and `SF1FEE` are the only prefixes defined.
- **Who rejects unknown prefixes:** `sealforge-core` decoders. `BidMemo::decode` returns `BadPrefix` for anything but `SF1` (test `rejects_bad_input` uses `SF2:`), and the selection rules classify such a payment `invalid(bad_memo)` ([2.5](selection.md)). An unparseable memo yields no refund address, so the payment goes to manual review rather than an automatic refund (`crates/sealforge-coordinator/src/refunds.rs` module doc). The coordinator, CLI auditor and WASM auditor all use the same decoder, so they agree.
- **How `SF2` would coexist:** a future version would get a new prefix and its own decoder, and the auction would record which version it accepts (as `bid_mode` does today). An `SF1` auction MUST keep treating `SF2` memos as invalid, so its result never changes when new versions ship. Commitment personalizations carry their own version (`_v1`, `leaf1`), so a format change also changes every hash. The published `algorithm_version` names the clearing rules. `Specified`: there is no `SF2` today.
