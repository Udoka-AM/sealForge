# 8. API and SDK reference

## Coordinator REST API

Status: `Testnet`. Router: `crates/sealforge-coordinator/src/routes/mod.rs`. Request and response types: `crates/sealforge-api/src/types.rs` (type names below). No OpenAPI file is generated yet. The Rust types are the schema.

**Auth column:** *public* = no auth. *organiser* / *trustee* = `SF-Sig` signed with that role's key ([4 Architecture §API authentication](04-architecture.md#api-authentication)); the coordinator then checks the key owns the auction or holds a trustee slot in it.

| Method and path | Auth | Request → response | Notes |
|---|---|---|---|
| `GET /healthz` | public | → `ok` | Liveness |
| `GET /readyz` | public | → readiness | Reports chain and scanner lag (`readiness_reports_chain_and_scanner_lag`) |
| `GET /v1/chain` | public | → `ChainView { tip }` | |
| `POST /v1/organisers` | signed with the new key | `RegisterOrganiser { display_name, invite_code }` → `OrganiserView` | Invite codes from `SF_INVITE_CODES`. `409 already_registered` |
| `GET /v1/organisers/me` | organiser | → `OrganiserView` | |
| `POST /v1/auctions` | organiser | `CreateAuction` → `CreatedAuction { auction, trustee_invites }` | Validation: `crates/sealforge-coordinator/src/validate.rs` |
| `GET /v1/auctions` | public | → `[AuctionView]` | Public statuses only (`open` … `settled`) |
| `GET /v1/auctions/{code}` | public | → `AuctionView` | `bid_count` is `null` below 5 bids (`MIN_PUBLIC_BID_COUNT`) so a count can't single out early bidders |
| `PATCH /v1/auctions/{code}` | organiser | `UpdateAuction` → `AuctionView` | Only in `draft`, `dkg`, `ready` (`409 not_editable`) |
| `DELETE /v1/auctions/{code}` | organiser | → 204 | `draft` only (`409 not_draft`) |
| `POST /v1/auctions/{code}/open` | organiser | → `AuctionView` | `409 dkg_incomplete`; `422` if `close_height < tip + 144` |
| `POST /v1/auctions/{code}/cancel` | organiser | → `AuctionView` | `409 has_bids` if open with bids |
| `POST /v1/auctions/{code}/publish` | organiser | → `AuctionView` | `closed → published`; releases the UIVK |
| `GET /v1/auctions/{code}/result` | public | → `PublishedResult` | [2.3](02-protocol/wire-formats.md#published-result) |
| `GET /v1/auctions/{code}/refunds` | public | → refund progress | Aggregate counts only, never per bid |
| `GET /v1/console/auctions` | organiser | → `[AuctionView]` with `owner` fields | |
| `GET /v1/console/auctions/{code}` | organiser | → `AuctionView` | |
| `GET /v1/console/auctions/{code}/batches` | organiser | → `[ConsoleBatch]` | |
| `POST /v1/trustees/join` | signed with the trustee's new key | `JoinAsTrustee { invite_code, enc_pubkey }` → `TrusteeJoined` | `409 already_trustee` |
| `GET /v1/auctions/{code}/dkg` | trustee | → `DkgState` | Round-1 packages, round-2 packages addressed to the caller |
| `POST /v1/auctions/{code}/dkg/round1` | trustee | `PostRound1 { payload, signature }` → `DkgState` | `409 not_in_dkg`, `package_exists`; `400 invalid_request` if the package signature doesn't verify |
| `POST /v1/auctions/{code}/dkg/round2` | trustee | `PostRound2 { packages: [Round2Out] }` → `DkgState` | Sealed and signed per recipient |
| `POST /v1/auctions/{code}/dkg/complete` | trustee | `CompleteDkg { public_key_package }` → `DkgState` | `409 dkg_mismatch` if packages differ |
| `GET /v1/auctions/{code}/batches` | trustee | → `[BatchView]` | `sighash`, `spends [{action_index, alpha}]`, `total_out_zat`, `output_count`, signer state. **No PCZT** |
| `POST /v1/batches/{id}/commitments` | trustee | `PostCommitments { commitments }` → `BatchView` | `409 not_signing` |
| `POST /v1/batches/{id}/shares` | trustee | `PostShares { shares }` → `BatchView` | `409 not_a_signer` |

### Errors

Every non-2xx response has the body `ApiErrorBody { code, error }` (`crates/sealforge-coordinator/src/error.rs`):

| HTTP | `code` |
|---|---|
| 400 | `invalid_request` |
| 401 | `unauthorized` (missing, malformed, stale or replayed `SF-Sig`) |
| 403 | `forbidden` |
| 404 | `not_found` |
| 409 | One of `already_registered`, `already_trustee`, `dkg_incomplete`, `dkg_mismatch`, `has_bids`, `invalid_transition`, `not_a_signer`, `not_draft`, `not_editable`, `not_in_dkg`, `not_signing`, `package_exists`, `status_changed` |
| 422 | `validation_failed` |
| 503 | `unavailable` (e.g. chain height unknown) |
| 500 | `internal` |

## `@sealforge/bid` (TypeScript)

Status: `Testnet` (the web bid flow uses it). Source: `packages/bid/src`. Dependency: `@noble/hashes`. Amounts are `bigint` zatoshi.

| Export | Purpose |
|---|---|
| `bidCommitment(code, refundUa, salt)` | Deposit-mode commitment ([2.4](02-protocol/commitments.md)) |
| `priceCommitment(code, refundUa, priceZat, salt)` | Sealed-reveal commitment |
| `bidMemo(code, commitment, refundUa)` | `SF1` memo, ≤ 512 bytes |
| `revealMemo(code, commitment, priceZat, salt)` | `SF1R` memo |
| `paymentUri(address, amountZat, memo)`, `base64url` | ZIP 321 URI |
| `prepareBid`, `prepareSealedBid`, `prepareReveal` | Salt (from `crypto.getRandomValues`), memo, URI and receipt in one call |
| `receiptMemo`, `verifyReceipt` | Rebuild the memo from a receipt; check a receipt is self-consistent |
| `formatZec`, `parseZec`, `canonicalDenominations`, `isCanonical`, `roundingOptions` | Amount handling without floats |
| `isValidAuctionCode`, `looksLikeUnifiedAddress` | Client-side checks. The coordinator does the real address validation |
| `REVEAL_AMOUNT_ZAT`, `MAX_MEMO_BYTES`, `MIN_DENOMINATION`, `COIN`, `MAX_MONEY` | Constants |

**Parity guarantee:** commitments, memos and URIs are byte-identical to `sealforge-core`. Both suites check the same vector file in CI ([2.4 §Parity](02-protocol/commitments.md#rust-and-typescript-parity)).

## Rust crates as libraries

All crates are `MIT OR Apache-2.0` and unpublished (use them as git or path dependencies). Ones that are useful on their own:

| Crate | Reusable for | Notes |
|---|---|---|
| `sealforge-escrow` | FROST 2-of-3 custody of Ironwood notes: DKG, rerandomized signing over PCZT `(sighash, α)`, escrow FVK/UA/UFVK/UIVK from a group key | Feature `address` (default) pulls in `orchard`/`zcash_keys`; off for a signer-only build. Carries the [upstream FROST risk](03-zcash-integration.md#dependencies-and-upstream-risk) |
| `sealforge-scan` | Auditing any address's incoming payments with a UIVK, natively or in WASM. Orchard and Ironwood | Pure; you feed it compact blocks and raw transactions |
| `sealforge-wallet` | An in-tab Ironwood wallet: BIP-39/ZIP 32 keys, scanning with witness checks, PCZT payments | Spends Ironwood notes only |
| `sealforge-core` | Sealed-bid selection, uniform/first-price clearing, Merkle inclusion proofs, canonical denominations | Pure |
| `sealforge-auditor` | Recomputing and comparing a sealForge result | Library, CLI and WASM |
