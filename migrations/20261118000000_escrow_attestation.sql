-- Trustees check the escrow address against their own FROST group key
-- before an auction can open (finding reported 2026-11-17).
--   escrow_view_sealed: nk || rivk of the escrow FVK, sealed to the trustee's
--     X25519 key by the coordinator when key generation completes.
--   escrow_attestation: the trustee's Ed25519 signature over
--     "sealforge-escrow-v1\n<code>\n<slot>\n<escrow UA>".
ALTER TABLE trustees
    ADD COLUMN escrow_view_sealed BYTEA,
    ADD COLUMN escrow_attestation BYTEA,
    ADD COLUMN escrow_verified_at TIMESTAMPTZ;
