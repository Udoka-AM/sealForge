-- Stream 4: a trustee's long-term Ed25519 key vouches for its key in each
-- auction (sealforge-trustee-cert-v1 over auction, slot, per-auction key and
-- where the trustee publishes its long-term key). Public, like the
-- attestations. One long-term key may hold only one seat per auction.
ALTER TABLE trustees
  ADD COLUMN identity_pubkey BYTEA,
  ADD COLUMN identity_published_at TEXT,
  ADD COLUMN identity_cert BYTEA;
CREATE UNIQUE INDEX trustees_identity_one_seat
  ON trustees (auction_id, identity_pubkey) WHERE identity_pubkey IS NOT NULL;
