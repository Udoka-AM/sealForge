-- Stream 2c: before releasing shares, each chosen signer states the round 2
-- inputs it will sign over (sighash, randomizers, both signers' commitments)
-- as a digest signed with its trustee key. Its co-signer refuses to sign if
-- the two digests differ, which would mean the coordinator showed them
-- different commitment sets. {"digest": hex, "signature": hex}.
ALTER TABLE signature_shares ADD COLUMN inputs_json JSONB;
