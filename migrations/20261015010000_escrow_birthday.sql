-- Task 2.5: the chain height when the escrow address was derived (DKG completion).
-- The escrow wallet scans from here; no payment to the escrow can be older.
ALTER TABLE auctions ADD COLUMN escrow_birthday_height INTEGER;
