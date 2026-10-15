-- Task 3.10: a signing round that stalls (a chosen signer never sends shares) is reset.
ALTER TABLE tx_batches ADD COLUMN signers_selected_at TIMESTAMPTZ;
