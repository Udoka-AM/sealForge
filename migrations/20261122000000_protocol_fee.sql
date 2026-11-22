-- Issue #74: sealForge's protocol fee. Each auction records the rate and the
-- fee address when it is created (from the coordinator's SF_FEE_BPS and
-- SF_FEE_UA; 0 means no fee), and both are published. The payout pays the fee
-- as a second output; tx_batches.protocol_fee_zat records what a payout paid,
-- so a fee is never paid twice.
ALTER TABLE auctions
  ADD COLUMN fee_bps SMALLINT NOT NULL DEFAULT 0 CHECK (fee_bps BETWEEN 0 AND 1000),
  ADD COLUMN fee_ua TEXT,
  ADD CONSTRAINT auctions_fee_has_address CHECK (fee_bps = 0 OR fee_ua IS NOT NULL);
ALTER TABLE tx_batches ADD COLUMN protocol_fee_zat BIGINT NOT NULL DEFAULT 0 CHECK (protocol_fee_zat >= 0);
