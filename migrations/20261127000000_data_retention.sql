-- Retention (#65, docs/05-BACKEND-SCHEMA.md §5.8). When the coordinator erases
-- a finished auction's secrets (its escrow viewing key, its bidders' refund
-- addresses and its key-generation packages), it records when here. The
-- published result, the bids and the audit log stay: they are public evidence.
ALTER TABLE auctions ADD COLUMN data_erased_at TIMESTAMPTZ;
