-- Task 3.9: interactive 2-of-3 FROST signing of refund/payout batches.

-- What trustees sign: the shielded sighash and, per escrow spend, the action index and alpha.
ALTER TABLE tx_batches
  ADD COLUMN sighash BYTEA CHECK (sighash IS NULL OR octet_length(sighash) = 32),
  ADD COLUMN spend_auth JSONB NOT NULL DEFAULT '[]',
  -- Total paid out and the number of outputs, shown to trustees before they sign.
  ADD COLUMN total_out_zat BIGINT,
  ADD COLUMN output_count INTEGER,
  -- The two trustees whose commitments were accepted (FROST signer set).
  ADD COLUMN signer_slots SMALLINT[];

-- One row per trustee per batch: round 1 commitments, then round 2 shares (one per spend, hex).
ALTER TABLE signature_shares
  ADD COLUMN commitments_json JSONB,
  ADD COLUMN shares_json JSONB;
