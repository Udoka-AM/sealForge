-- Task 2.4: trustee encryption keys, signed DKG packages, and the escrow group key.
-- Source of truth: docs/05-BACKEND-SCHEMA.md

-- X25519 key each trustee registers at join; round 2 packages are sealed to it.
ALTER TABLE trustees
  ADD COLUMN enc_pubkey BYTEA CHECK (enc_pubkey IS NULL OR octet_length(enc_pubkey) = 32),
  -- Serialized FROST public key package the trustee derived. All three must match.
  ADD COLUMN public_key_package BYTEA;

-- Ed25519 signature by the sending trustee over the package (see sealforge-api dkg_package_message).
ALTER TABLE dkg_packages ADD COLUMN signature BYTEA NOT NULL CHECK (octet_length(signature) = 64);

-- FROST group verifying key (Orchard ak), set when DKG completes.
ALTER TABLE auctions ADD COLUMN escrow_group_key BYTEA CHECK (escrow_group_key IS NULL OR octet_length(escrow_group_key) = 32);
