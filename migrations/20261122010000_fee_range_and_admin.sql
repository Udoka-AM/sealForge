-- Protocol fee rates are 0 (no fee) or 5% to 15% (sealforge_core::fee), and
-- the operator's admin may set an auction's rate before it opens, so the
-- audit log gains an 'admin' actor.
ALTER TABLE auctions DROP CONSTRAINT auctions_fee_bps_check;
ALTER TABLE auctions ADD CONSTRAINT auctions_fee_bps_check
  CHECK (fee_bps = 0 OR fee_bps BETWEEN 500 AND 1500);
ALTER TABLE audit_log DROP CONSTRAINT audit_log_actor_kind_check;
ALTER TABLE audit_log ADD CONSTRAINT audit_log_actor_kind_check
  CHECK (actor_kind IN ('organiser', 'trustee', 'system', 'admin'));
