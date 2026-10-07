-- sealForge initial schema. Source of truth: docs/05-BACKEND-SCHEMA.md
CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TYPE network_kind   AS ENUM ('regtest', 'testnet', 'mainnet');
CREATE TYPE auction_mode   AS ENUM ('uniform_price', 'first_price');
CREATE TYPE bid_mode       AS ENUM ('deposit', 'commit_reveal');
CREATE TYPE auction_status AS ENUM ('draft', 'dkg', 'ready', 'open', 'closed', 'published', 'refunding', 'settled', 'cancelled');
CREATE TYPE dkg_status     AS ENUM ('invited', 'round1', 'round2', 'complete');
CREATE TYPE bid_status     AS ENUM ('valid', 'invalid', 'late', 'under_reserve');
CREATE TYPE batch_kind     AS ENUM ('refund', 'payout');
CREATE TYPE batch_status   AS ENUM ('building', 'awaiting_signatures', 'signed', 'broadcast', 'confirmed', 'failed');
CREATE TYPE note_status    AS ENUM ('planned', 'batched', 'broadcast', 'confirmed', 'failed');

CREATE TABLE organisers (
  id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  display_name  TEXT NOT NULL CHECK (char_length(display_name) BETWEEN 1 AND 80),
  pubkey        BYTEA NOT NULL UNIQUE CHECK (octet_length(pubkey) = 32),
  created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  disabled_at   TIMESTAMPTZ
);

CREATE TABLE auctions (
  id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  code                TEXT NOT NULL UNIQUE CHECK (code ~ '^[0-9A-HJKMNP-TV-Z]{10}$'),
  organiser_id        UUID NOT NULL REFERENCES organisers(id),
  title               TEXT NOT NULL CHECK (char_length(title) BETWEEN 1 AND 120),
  description         TEXT NOT NULL DEFAULT '' CHECK (char_length(description) <= 4000),
  lot_image_url       TEXT,
  network             network_kind NOT NULL DEFAULT 'testnet',
  mode                auction_mode NOT NULL DEFAULT 'uniform_price',
  bid_mode            bid_mode NOT NULL DEFAULT 'deposit',
  quantity            INTEGER NOT NULL DEFAULT 1 CHECK (quantity BETWEEN 1 AND 100000),
  reserve_zat         BIGINT NOT NULL DEFAULT 0 CHECK (reserve_zat BETWEEN 0 AND 2100000000000000),
  open_height         INTEGER,
  close_height        INTEGER NOT NULL,
  reveal_close_height INTEGER,
  threshold           SMALLINT NOT NULL DEFAULT 2 CHECK (threshold = 2),
  max_signers         SMALLINT NOT NULL DEFAULT 3 CHECK (max_signers = 3),
  escrow_ua           TEXT,
  escrow_ufvk_enc     BYTEA,
  escrow_ivk          TEXT,
  payout_ua           TEXT NOT NULL,
  fee_buffer_zat      BIGINT NOT NULL DEFAULT 0 CHECK (fee_buffer_zat >= 0),
  status              auction_status NOT NULL DEFAULT 'draft',
  created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
  published_at        TIMESTAMPTZ,
  settled_at          TIMESTAMPTZ,
  CHECK (escrow_ivk IS NULL OR status IN ('published', 'refunding', 'settled'))
);
CREATE INDEX auctions_organiser_idx ON auctions (organiser_id);
CREATE INDEX auctions_status_idx ON auctions (status);
CREATE INDEX auctions_network_status_idx ON auctions (network, status);

CREATE TABLE trustees (
  id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  auction_id       UUID NOT NULL REFERENCES auctions(id) ON DELETE CASCADE,
  slot             SMALLINT NOT NULL CHECK (slot BETWEEN 1 AND 3),
  label            TEXT NOT NULL CHECK (char_length(label) BETWEEN 1 AND 80),
  pubkey           BYTEA CHECK (pubkey IS NULL OR octet_length(pubkey) = 32),
  dkg_status       dkg_status NOT NULL DEFAULT 'invited',
  invite_code_hash BYTEA NOT NULL,
  joined_at        TIMESTAMPTZ,
  UNIQUE (auction_id, slot),
  UNIQUE (auction_id, pubkey)
);

CREATE TABLE dkg_packages (
  id         BIGSERIAL PRIMARY KEY,
  auction_id UUID NOT NULL REFERENCES auctions(id) ON DELETE CASCADE,
  round      SMALLINT NOT NULL CHECK (round IN (1, 2)),
  from_slot  SMALLINT NOT NULL,
  to_slot    SMALLINT,
  payload    BYTEA NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX dkg_packages_unique_idx ON dkg_packages (auction_id, round, from_slot, COALESCE(to_slot, 0));

CREATE TABLE bids (
  id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  auction_id     UUID NOT NULL REFERENCES auctions(id),
  txid           BYTEA NOT NULL CHECK (octet_length(txid) = 32),
  output_index   INTEGER NOT NULL CHECK (output_index >= 0),
  pool           TEXT NOT NULL,
  block_height   INTEGER NOT NULL,
  block_time     TIMESTAMPTZ NOT NULL,
  amount_zat     BIGINT NOT NULL CHECK (amount_zat > 0),
  memo_version   SMALLINT,
  commitment     BYTEA CHECK (commitment IS NULL OR octet_length(commitment) = 32),
  refund_ua_enc  BYTEA,
  status         bid_status NOT NULL,
  invalid_reason TEXT,
  leaf_hash      BYTEA CHECK (leaf_hash IS NULL OR octet_length(leaf_hash) = 32),
  is_winner      BOOLEAN,
  price_paid_zat BIGINT CHECK (price_paid_zat IS NULL OR price_paid_zat >= 0),
  refund_due_zat BIGINT CHECK (refund_due_zat IS NULL OR refund_due_zat >= 0),
  detected_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (txid, output_index)
);
CREATE INDEX bids_auction_status_idx ON bids (auction_id, status);
CREATE INDEX bids_auction_commitment_idx ON bids (auction_id, commitment);
CREATE INDEX bids_auction_height_idx ON bids (auction_id, block_height);

CREATE TABLE clearing_results (
  auction_id         UUID PRIMARY KEY REFERENCES auctions(id),
  algorithm_version  TEXT NOT NULL,
  clearing_price_zat BIGINT,
  total_volume_zat   BIGINT NOT NULL,
  valid_bid_count    INTEGER NOT NULL,
  winner_count       INTEGER NOT NULL,
  merkle_root        BYTEA NOT NULL CHECK (octet_length(merkle_root) = 32),
  scan_from_height   INTEGER NOT NULL,
  scan_to_height     INTEGER NOT NULL,
  computed_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE tx_batches (
  id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  auction_id           UUID NOT NULL REFERENCES auctions(id),
  kind                 batch_kind NOT NULL,
  status               batch_status NOT NULL DEFAULT 'building',
  pczt                 BYTEA,
  signing_package_hash BYTEA,
  fee_zat              BIGINT,
  txid                 BYTEA,
  confirmed_height     INTEGER,
  created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
  broadcast_at         TIMESTAMPTZ
);
CREATE INDEX tx_batches_auction_status_idx ON tx_batches (auction_id, status);
CREATE UNIQUE INDEX tx_batches_txid_idx ON tx_batches (txid) WHERE txid IS NOT NULL;

CREATE TABLE refund_notes (
  id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  auction_id       UUID NOT NULL REFERENCES auctions(id),
  bid_id           UUID REFERENCES bids(id),
  kind             batch_kind NOT NULL DEFAULT 'refund',
  denomination_zat BIGINT NOT NULL CHECK (denomination_zat > 0),
  not_before       TIMESTAMPTZ NOT NULL,
  batch_id         UUID REFERENCES tx_batches(id),
  status           note_status NOT NULL DEFAULT 'planned'
);
CREATE INDEX refund_notes_due_idx ON refund_notes (auction_id, status, not_before);
CREATE INDEX refund_notes_bid_idx ON refund_notes (bid_id);
CREATE INDEX refund_notes_batch_idx ON refund_notes (batch_id);

CREATE TABLE signature_shares (
  batch_id     UUID NOT NULL REFERENCES tx_batches(id),
  trustee_id   UUID NOT NULL REFERENCES trustees(id),
  commitments  BYTEA,
  share        BYTEA,
  submitted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (batch_id, trustee_id)
);

CREATE TABLE scanner_state (
  auction_id  UUID PRIMARY KEY REFERENCES auctions(id),
  last_height INTEGER NOT NULL,
  last_hash   BYTEA NOT NULL,
  updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE api_nonces (
  pubkey  BYTEA NOT NULL,
  nonce   BYTEA NOT NULL CHECK (octet_length(nonce) = 16),
  seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (pubkey, nonce)
);

CREATE TABLE audit_log (
  id         BIGSERIAL PRIMARY KEY,
  auction_id UUID,
  actor_kind TEXT NOT NULL CHECK (actor_kind IN ('organiser', 'trustee', 'system')),
  actor_id   UUID,
  action     TEXT NOT NULL,
  detail     JSONB NOT NULL DEFAULT '{}',
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX audit_log_auction_idx ON audit_log (auction_id, created_at);

-- Refund notes for a bid must sum exactly to its refund due (checked when the
-- planner finishes, so it is deferred to the end of the transaction).
CREATE FUNCTION check_refund_total() RETURNS trigger AS $$
DECLARE
  due   BIGINT;
  total BIGINT;
BEGIN
  IF NEW.bid_id IS NULL THEN
    RETURN NULL;
  END IF;
  SELECT refund_due_zat INTO due FROM bids WHERE id = NEW.bid_id;
  SELECT COALESCE(SUM(denomination_zat), 0) INTO total FROM refund_notes WHERE bid_id = NEW.bid_id;
  IF due IS DISTINCT FROM total THEN
    RAISE EXCEPTION 'refund notes for bid % sum to %, expected %', NEW.bid_id, total, due;
  END IF;
  RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER refund_total_matches
  AFTER INSERT OR UPDATE ON refund_notes
  DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION check_refund_total();
