-- Task 4.6: commit-reveal mode.
--   memo_refund_ua_enc: the refund address exactly as written in the deposit memo
--     (encrypted). It is part of the price commitment, so it's needed to check reveals
--     even when it isn't usable for paying a refund (refund_ua_enc is then NULL).
--   reveal_price_zat:  on a reveal payment, the price it claims; on a deposit, the
--     price that counted once the reveal window closed.
--   reveal_salt:       on a reveal payment, the salt it discloses.
ALTER TABLE bids
  ADD COLUMN memo_refund_ua_enc BYTEA,
  ADD COLUMN reveal_price_zat BIGINT CHECK (reveal_price_zat IS NULL OR reveal_price_zat >= 0),
  ADD COLUMN reveal_salt BYTEA CHECK (reveal_salt IS NULL OR octet_length(reveal_salt) = 32);
