-- Issue #41: no escrow transaction may spend more notes than ZIP 218 lets a
-- block hold. Oversized refunds are preceded by a consolidation (oldest notes
-- merged into one at the escrow's internal address), and the payout may take
-- several sweeps.
ALTER TYPE batch_kind ADD VALUE IF NOT EXISTS 'consolidate';

-- The payout sweep stopped at the per-transaction note limit; another payout
-- follows once it confirms.
ALTER TABLE tx_batches ADD COLUMN more_after BOOLEAN NOT NULL DEFAULT false;
