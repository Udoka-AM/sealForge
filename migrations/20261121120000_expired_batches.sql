-- D3: an escrow transaction that wasn't mined by its expiry height can never
-- be mined. The batch is marked expired (kept for the audit trail) and its
-- notes move to a fresh batch of the same kind, which is built and signed again.
-- Distinct from 'failed', which needs an operator and fails the health check.
ALTER TYPE batch_status ADD VALUE IF NOT EXISTS 'expired';
