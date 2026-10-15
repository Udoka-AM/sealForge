# 5. Backend Schema: sealForge

**Purpose:** Define stored data, relationships, and access rules.

Store: **PostgreSQL 16** (`pgcrypto` extension). Migrations live in [`migrations/`](../migrations) and run with `sqlx migrate run`. Amounts are always integer **zatoshi** (`BIGINT`, 1 ZEC = 100,000,000 zat). Times are `TIMESTAMPTZ` (UTC). Chain positions are block heights (`INTEGER`).

Related: [02-TRD.md](02-TRD.md)

---

## 5.1 Entities

| Entity | Represents |
|---|---|
| `organisers` | An organiser identity, an Ed25519 public key that signs API calls |
| `auctions` | One sealed-bid auction: lot, rules, chain window, escrow, lifecycle status |
| `trustees` | One of the (default 3) FROST signers of an auction's escrow |
| `dkg_packages` | Relayed FROST DKG round messages (round 2 messages are encrypted to the recipient) |
| `bids` | One shielded payment detected in the escrow, with its parsed memo and outcome |
| `clearing_results` | The deterministic result of an auction: price, volume, winners, Merkle root, scan range |
| `refund_notes` | One denominated refund (or payout) output to be paid to a bidder's refund UA |
| `tx_batches` | One escrow transaction (refund batch or payout) built as a PCZT and co-signed |
| `signature_shares` | A trustee's FROST signature share for a batch |
| `scanner_state` | Last scanned block per auction escrow (resume point) |
| `api_nonces` | Replay protection for signed API requests |
| `audit_log` | Append-only log of organiser/trustee/system actions |

Not stored server-side by design: receipt salts, ShieldRamp sessions, EVM addresses, ephemeral wallet seeds, bidder IPs.

## 5.2 Tables

### `organisers`
| Field | Type | Required | Default | Notes |
|---|---|---|---|---|
| `id` | `UUID` | ✓ | `gen_random_uuid()` | PK |
| `display_name` | `TEXT` | ✓ | — | 1–80 chars |
| `pubkey` | `BYTEA` | ✓ | — | Ed25519, 32 bytes, **unique** |
| `created_at` | `TIMESTAMPTZ` | ✓ | `now()` | |
| `disabled_at` | `TIMESTAMPTZ` | | `NULL` | Soft disable |

Indexes: `UNIQUE (pubkey)`.

### `auctions`
| Field | Type | Required | Default | Notes |
|---|---|---|---|---|
| `id` | `UUID` | ✓ | `gen_random_uuid()` | PK |
| `code` | `TEXT` | ✓ | — | Public 10-char Crockford base32 ID used in memos and URLs. **Unique** |
| `organiser_id` | `UUID` | ✓ | — | FK → `organisers.id` |
| `title` | `TEXT` | ✓ | — | 1–120 chars |
| `description` | `TEXT` | | `''` | ≤ 4,000 chars, Markdown |
| `lot_image_url` | `TEXT` | | `NULL` | https URL |
| `network` | `network_kind` | ✓ | `'testnet'` | `regtest` · `testnet` · `mainnet` |
| `mode` | `auction_mode` | ✓ | `'uniform_price'` | `uniform_price` · `first_price` |
| `bid_mode` | `bid_mode` | ✓ | `'deposit'` | `deposit` · `commit_reveal` |
| `quantity` | `INTEGER` | ✓ | `1` | 1–100,000 units |
| `reserve_zat` | `BIGINT` | ✓ | `0` | ≥ 0 |
| `open_height` | `INTEGER` | | `NULL` | Set when opened |
| `close_height` | `INTEGER` | ✓ | — | Last height whose bids count |
| `reveal_close_height` | `INTEGER` | | `NULL` | Commit-reveal only |
| `threshold` | `SMALLINT` | ✓ | `2` | FROST *t* |
| `max_signers` | `SMALLINT` | ✓ | `3` | FROST *n* |
| `escrow_ua` | `TEXT` | | `NULL` | Set after DKG |
| `escrow_ufvk_enc` | `BYTEA` | | `NULL` | `pgp_sym_encrypt` with `SF_DATA_KEY` |
| `escrow_ivk` | `TEXT` | | `NULL` | Plaintext **only after publish** (it is public then) |
| `escrow_group_key` | `BYTEA` | | `NULL` | FROST group verifying key (Orchard `ak`), 32 bytes, set when DKG completes |
| `payout_ua` | `TEXT` | ✓ | — | Organiser payout address (Ironwood UA) |
| `fee_buffer_zat` | `BIGINT` | ✓ | `0` | Funded by the organiser for ZIP-317 fees |
| `status` | `auction_status` | ✓ | `'draft'` | See lifecycle below |
| `created_at` / `updated_at` | `TIMESTAMPTZ` | ✓ | `now()` | |
| `published_at` / `settled_at` | `TIMESTAMPTZ` | | `NULL` | |

Lifecycle: `draft → dkg → ready → open → closed → published → refunding → settled`, plus `cancelled` (from `draft`, `dkg`, `ready`, or `open` with zero bids).

Indexes: `UNIQUE (code)`, `(organiser_id)`, `(status)`, `(network, status)`.

### `trustees`
| Field | Type | Required | Default | Notes |
|---|---|---|---|---|
| `id` | `UUID` | ✓ | `gen_random_uuid()` | PK |
| `auction_id` | `UUID` | ✓ | — | FK → `auctions.id` `ON DELETE CASCADE` |
| `slot` | `SMALLINT` | ✓ | — | 1..`max_signers`. Also the FROST identifier |
| `label` | `TEXT` | ✓ | — | e.g. "Organiser", "Independent auditor", "Community signer" |
| `pubkey` | `BYTEA` | ✓ | — | Ed25519 API key, 32 bytes |
| `dkg_status` | `dkg_status` | ✓ | `'invited'` | `invited` · `round1` · `round2` · `complete` |
| `invite_code_hash` | `BYTEA` | ✓ | — | SHA-256 of the one-time invite |
| `enc_pubkey` | `BYTEA` | | `NULL` | X25519 key registered at join. Round 2 packages are sealed to it |
| `public_key_package` | `BYTEA` | | `NULL` | FROST public key package the trustee derived. All three must match |
| `joined_at` | `TIMESTAMPTZ` | | `NULL` | |

Indexes: `UNIQUE (auction_id, slot)`, `UNIQUE (auction_id, pubkey)`.

### `dkg_packages`
| Field | Type | Required | Default | Notes |
|---|---|---|---|---|
| `id` | `BIGSERIAL` | ✓ | — | PK |
| `auction_id` | `UUID` | ✓ | — | FK |
| `round` | `SMALLINT` | ✓ | — | 1 or 2 |
| `from_slot` | `SMALLINT` | ✓ | — | |
| `to_slot` | `SMALLINT` | | `NULL` | `NULL` = broadcast (round 1) |
| `payload` | `BYTEA` | ✓ | — | Round 2 is sealed (X25519 sealed box) to the recipient, so the coordinator can't read it |
| `signature` | `BYTEA` | ✓ | — | Ed25519 signature by the sending trustee over `sealforge-dkg-v1 ‖ code ‖ round ‖ from ‖ to ‖ sha256(payload)`. Recipients verify it, so the coordinator can't forge packages |
| `created_at` | `TIMESTAMPTZ` | ✓ | `now()` | |

Indexes: `UNIQUE (auction_id, round, from_slot, to_slot)`.

### `bids`
| Field | Type | Required | Default | Notes |
|---|---|---|---|---|
| `id` | `UUID` | ✓ | `gen_random_uuid()` | PK |
| `auction_id` | `UUID` | ✓ | — | FK |
| `txid` | `BYTEA` | ✓ | — | 32 bytes |
| `output_index` | `INTEGER` | ✓ | — | Action/output index within the tx |
| `pool` | `TEXT` | ✓ | — | `ironwood` (`sapling`/`orchard` accepted only if the escrow UA has the receiver) |
| `block_height` | `INTEGER` | ✓ | — | |
| `block_time` | `TIMESTAMPTZ` | ✓ | — | |
| `amount_zat` | `BIGINT` | ✓ | — | > 0 |
| `memo_version` | `SMALLINT` | | `NULL` | `1` for `SF1` |
| `commitment` | `BYTEA` | | `NULL` | 32 bytes |
| `refund_ua_enc` | `BYTEA` | | `NULL` | Encrypted. Wiped 30 days after settlement |
| `status` | `bid_status` | ✓ | — | `valid` · `invalid` · `late` · `under_reserve` |
| `invalid_reason` | `TEXT` | | `NULL` | `bad_memo`, `wrong_auction` (set by the scanner from the raw memo), `before_open`, `duplicate_commitment` (set at close by `sealforge-core::selection`), `fee_buffer` (the organiser's fee deposit, memo `SF1FEE:<code>`: never refunded, left out of clearing, added to `auctions.fee_buffer_zat`) |
| `leaf_hash` | `BYTEA` | | `NULL` | Merkle leaf (valid bids only) |
| `is_winner` | `BOOLEAN` | | `NULL` | Set at clearing |
| `price_paid_zat` | `BIGINT` | | `NULL` | Winners only |
| `refund_due_zat` | `BIGINT` | | `NULL` | `amount − price_paid` (or full amount) |
| `detected_at` | `TIMESTAMPTZ` | ✓ | `now()` | |

Indexes: `UNIQUE (txid, output_index)`, `(auction_id, status)`, `(auction_id, commitment)`, `(auction_id, block_height)`.

### `clearing_results`
| Field | Type | Required | Default | Notes |
|---|---|---|---|---|
| `auction_id` | `UUID` | ✓ | — | PK, FK |
| `algorithm_version` | `TEXT` | ✓ | — | e.g. `sealforge-core/0.1.0/uniform-v1` |
| `clearing_price_zat` | `BIGINT` | | `NULL` | `NULL` if no valid bids |
| `total_volume_zat` | `BIGINT` | ✓ | — | Sum of all valid bid amounts |
| `valid_bid_count` | `INTEGER` | ✓ | — | |
| `winner_count` | `INTEGER` | ✓ | — | |
| `merkle_root` | `BYTEA` | ✓ | — | 32 bytes |
| `scan_from_height` / `scan_to_height` | `INTEGER` | ✓ | — | Exact range the auditor must scan |
| `computed_at` | `TIMESTAMPTZ` | ✓ | `now()` | |

### `tx_batches`
| Field | Type | Required | Default | Notes |
|---|---|---|---|---|
| `id` | `UUID` | ✓ | `gen_random_uuid()` | PK |
| `auction_id` | `UUID` | ✓ | — | FK |
| `kind` | `batch_kind` | ✓ | — | `refund` · `payout` |
| `status` | `batch_status` | ✓ | `'building'` | `building` · `awaiting_signatures` · `signed` · `broadcast` · `confirmed` · `failed` |
| `pczt` | `BYTEA` | | `NULL` | Serialized PCZT |
| `signing_package_hash` | `BYTEA` | | `NULL` | Shown to trustees for review |
| `fee_zat` | `BIGINT` | | `NULL` | ZIP-317 fee |
| `txid` | `BYTEA` | | `NULL` | After broadcast |
| `confirmed_height` | `INTEGER` | | `NULL` | |
| `created_at` / `broadcast_at` | `TIMESTAMPTZ` | | `now()` / `NULL` | |

Indexes: `(auction_id, status)`, `UNIQUE (txid) WHERE txid IS NOT NULL`.

### `refund_notes`
| Field | Type | Required | Default | Notes |
|---|---|---|---|---|
| `id` | `UUID` | ✓ | `gen_random_uuid()` | PK |
| `auction_id` | `UUID` | ✓ | — | FK |
| `bid_id` | `UUID` | | `NULL` | FK → `bids.id`. `NULL` for payout notes |
| `kind` | `batch_kind` | ✓ | `'refund'` | |
| `denomination_zat` | `BIGINT` | ✓ | — | Canonical 1/2/5 × 10^k only |
| `not_before` | `TIMESTAMPTZ` | ✓ | — | Random delay (CSPRNG) |
| `batch_id` | `UUID` | | `NULL` | FK → `tx_batches.id` |
| `status` | `note_status` | ✓ | `'planned'` | `planned` · `batched` · `broadcast` · `confirmed` · `failed` |

Indexes: `(auction_id, status, not_before)`, `(bid_id)`, `(batch_id)`.

Refund planning (task 3.9): when an auction is published, the coordinator plans `refund_notes` for every bid with a refund of at least 0.0001 ZEC and a stored refund address, each with a CSPRNG delay, and moves the auction to `refunding`. A batcher shuffles due notes and groups them into `tx_batches` (≤ 20 outputs, bidders interleaved). Due notes are held until at least 3 bidders' notes can share a batch, the oldest due note has waited 30 minutes, or no more notes are coming (`sealforge_core::refunds::ready_to_batch`). Payments that reach the escrow after close are stored as `late` with `refund_due_zat = amount_zat` and planned for a full refund with the same delays. Bids whose memo was unreadable have no refund address and are counted as `needs_manual_review` in `GET /v1/auctions/{code}/refunds` (aggregates only).

### `signature_shares`
| Field | Type | Required | Default | Notes |
|---|---|---|---|---|
| `batch_id` | `UUID` | ✓ | — | FK, part of PK |
| `trustee_id` | `UUID` | ✓ | — | FK, part of PK |
| `commitments` | `BYTEA` | | `NULL` | FROST round-1 signing commitments |
| `share` | `BYTEA` | | `NULL` | FROST round-2 signature share |
| `submitted_at` | `TIMESTAMPTZ` | ✓ | `now()` | |

PK `(batch_id, trustee_id)`.

### `scanner_state`
| Field | Type | Required | Default |
|---|---|---|---|
| `auction_id` | `UUID` (PK, FK) | ✓ | — |
| `last_height` | `INTEGER` | ✓ | — |
| `last_hash` | `BYTEA` | ✓ | — |
| `updated_at` | `TIMESTAMPTZ` | ✓ | `now()` |

### `api_nonces`
`pubkey BYTEA`, `nonce BYTEA` (16 bytes), `seen_at TIMESTAMPTZ DEFAULT now()`. PK `(pubkey, nonce)`. Rows older than 10 minutes are purged every minute.

### `audit_log`
`id BIGSERIAL PK`, `auction_id UUID NULL`, `actor_kind TEXT` (`organiser` · `trustee` · `system`), `actor_id UUID NULL`, `action TEXT`, `detail JSONB DEFAULT '{}'`, `created_at TIMESTAMPTZ DEFAULT now()`. Index `(auction_id, created_at)`. Append-only (no UPDATE/DELETE grants).

## 5.3 Relationships

| Relationship | Type | Foreign key |
|---|---|---|
| organiser → auctions | one-to-many | `auctions.organiser_id → organisers.id` |
| auction → trustees | one-to-many (exactly `max_signers`) | `trustees.auction_id` |
| auction → dkg_packages | one-to-many | `dkg_packages.auction_id` |
| auction → bids | one-to-many | `bids.auction_id` |
| auction → clearing_results | one-to-one | `clearing_results.auction_id` (PK) |
| auction → scanner_state | one-to-one | `scanner_state.auction_id` (PK) |
| bid → refund_notes | one-to-many | `refund_notes.bid_id` |
| tx_batch → refund_notes | one-to-many | `refund_notes.batch_id` |
| tx_batch ↔ trustees | many-to-many via `signature_shares` | `signature_shares.batch_id`, `signature_shares.trustee_id` |

## 5.4 User ownership

- An **organiser** owns their `auctions` and everything hanging off them (trustees, bids, results, batches, notes).
- A **trustee** owns nothing. They are attached to one auction and own only their `signature_shares` and `dkg_packages` rows.
- **Bidders** own no server rows. A bid row belongs to the auction. The bidder's proof of ownership is the receipt (commitment preimage), which only they hold.

## 5.5 Authentication flow

| Step | Flow |
|---|---|
| **Sign up (organiser)** | `sealforge init` generates an Ed25519 keypair locally → `POST /v1/organisers {display_name, pubkey, invite_code}` signed with the new key → row created. The hackathon instance is invite-only |
| **Sign up (trustee)** | Organiser adds trustee slots with labels → one-time invite codes are shared out of band → `sealforge trustee join <code>` generates a keypair and registers `pubkey` on the slot |
| **Sign in** | Stateless. Every request carries `Authorization: SF-Sig keyId=<b64 pubkey>, ts=<unix>, nonce=<b64 16 bytes>, sig=<b64 ed25519 sig>`. The signed message is `"SF-Sig-v1\n" + METHOD + "\n" + path_and_query + "\n" + ts + "\n" + nonce_b64 + "\n" + hex(sha256(body))` (implemented in `crates/sealforge-api/src/auth.rs`). The server checks the signature, a ±60 s window and nonce uniqueness. Unknown or other organisers' auctions return 404, so codes can't be probed |
| **Reset / lost key** | Organiser: the operator re-binds a new pubkey after out-of-band verification (logged in `audit_log`). Trustee: losing one share doesn't block signing (2-of-3). The auction settles and a new DKG is run for future auctions |
| **Session lifecycle** | No sessions or cookies on the API. The web console keeps the organiser key in memory only (loaded from file per visit). Optional: encrypted in IndexedDB with a passphrase |
| **Bidders** | No authentication. Receipts are client-side |

## 5.6 Authorization rules

| Entity | Read | Create | Update | Delete |
|---|---|---|---|---|
| `organisers` | Self | Anyone with a valid invite | Self (display name, key rotation) | Operator only (soft disable) |
| `auctions` (public fields: code, title, description, image, rules, status, heights, escrow UA, trustee labels) | **Public** once `open` | Organiser | Owner organiser, only in `draft`/`dkg`/`ready`. Status transitions per lifecycle | Owner, only in `draft` |
| `auctions.escrow_ivk` | Public after `published`. Never before | System (at publish) | — | — |
| `trustees` | Owner organiser + the auction's trustees. Labels are public | Owner organiser | Trustee (join, DKG status) | Owner, in `draft` |
| `dkg_packages` | Addressed trustee (or all trustees for broadcast) | Trustee of the auction | — | System (purged after DKG completes) |
| `bids` | Aggregate counts public (≥ 5). Full rows: owner organiser after close. Public after publish (minus `refund_ua_enc`) | System (scanner) | System (clearing) | System (retention wipe of `refund_ua_enc`) |
| `clearing_results` | Public after publish. Owner after close | System | — | — |
| `tx_batches`, `refund_notes` | Owner + trustees. Public aggregates (notes paid/total) | System | System. Trustees indirectly via signatures | — |
| `signature_shares` | Owner + trustees of the auction | Trustee (own row only) | — | — |
| `audit_log` | Owner (own auctions), operator | System | — | — |

Enforced in the coordinator handlers (role extractor + per-auction ownership check). The DB role used by the API has no `DELETE` on `audit_log` or `bids`.

## 5.7 Data validation

| Field | Rule |
|---|---|
| `auctions.code` | 10 chars, Crockford base32 (`[0-9A-HJKMNP-TV-Z]{10}`), unique |
| `title` | 1–120 chars, trimmed |
| `quantity` | 1–100,000 |
| `reserve_zat` | 0 ≤ x ≤ 2.1 × 10^15 |
| `close_height` | > current height + 144 (≥ 1 h on NU7) when opening |
| `threshold`, `max_signers` | v1 fixed at 2 and 3 |
| `payout_ua`, `escrow_ua` | Valid Unified Address for the auction's network, includes a shielded receiver, no transparent-only |
| Memo (bid) | ZIP-302 text memo, ≤ 512 bytes, grammar `SF1:<code>:<commitment-hex64>:<refund_ua>`. `code` must equal the auction's code. `refund_ua` must be a valid UA on the same network with a shielded receiver |
| Duplicate commitment in one auction | The second and later bids are `invalid` (`duplicate_commitment`) and fully refunded |
| Bid amount | > 0. `under_reserve` if < `reserve_zat` (full refund) |
| Late bid | `block_height > close_height` → `late` (full refund) |
| Refund total | Sum of a bid's `refund_notes` must equal `refund_due_zat` minus its dust (`refund_due_zat % 10000`), checked by a DB constraint trigger and in `sealforge-core` tests. Dust (< 0.0001 ZEC) is not sent |
| Denominations | Each note ∈ {1, 2, 5} × 10^k zat with 10^4 ≤ note (≥ 0.0001 ZEC). No non-canonical notes |
| Pubkeys | Exactly 32 bytes |

## 5.8 Retention and deletion

| Data | Kept | Purged / exported |
|---|---|---|
| Auctions, results, bids (minus refund UA), batches | Indefinitely. They're public audit evidence | Organiser can export `GET /v1/auctions/{id}/export` (JSON + CSV) |
| `bids.refund_ua_enc` | Until all notes are confirmed + 30 days | Overwritten with `NULL` by a daily job |
| `escrow_ufvk_enc` | Until settled + 30 days | Then `NULL` (IVK stays public) |
| `dkg_packages` | Until DKG completes | Deleted at `ready` |
| `api_nonces` | 10 minutes | Purged each minute |
| Server request logs | 7 days, no IPs on bid routes | Rotated |
| `audit_log` | Indefinitely | Export with the auction |
| Drafts | Until deleted by the organiser | Hard delete allowed in `draft` only |

## 5.9 Migration and seed data

- **Tool:** `sqlx migrate` with timestamped files in `migrations/` (`YYYYMMDDHHMMSS_name.sql`). Forward-only. A fix is a new migration.
- **Process:** write the migration → run the coordinator or `cargo test -p sealforge-coordinator` locally (both apply it) → PR review → applied on staging by the coordinator at startup (`sqlx::migrate!()`), then production on release. Queries are runtime-checked (`sqlx::query_as`, no compile-time macros), and the integration tests in `crates/sealforge-coordinator/tests/` run every query against a real Postgres in CI.
- **Initial migration:** `20261007000000_init.sql` creates the enums, all tables and indexes above, and the `refund_total_matches` check.
- **Seed data (`seed/dev.sql`, local and staging only):**
  - 1 organiser ("Demo Organiser", known test pubkey).
  - 1 `draft` auction "sealForge Zcon7 Pass" (10 units, 0.01 ZEC reserve, testnet).
  - 3 trustee slots (Organiser, Independent auditor, Community signer).
- **Test fixtures:** `crates/sealforge-core/tests/fixtures/` holds JSON bid sets (20, 500 and 20,000 bids) with expected clearing outputs, used by both Rust tests and the WASM auditor test.
