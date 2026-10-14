-- Refunds are paid in canonical notes only. The sub-0.0001 ZEC remainder
-- ("dust", < 10,000 zat) is not sent: it costs more in fees than it is worth,
-- and a non-canonical note would link the refund to the bid
-- (see docs/evidence/linkability.md).
CREATE OR REPLACE FUNCTION check_refund_total() RETURNS trigger AS $$
DECLARE
  due   BIGINT;
  total BIGINT;
BEGIN
  IF NEW.bid_id IS NULL THEN
    RETURN NULL;
  END IF;
  SELECT refund_due_zat INTO due FROM bids WHERE id = NEW.bid_id;
  SELECT COALESCE(SUM(denomination_zat), 0) INTO total FROM refund_notes WHERE bid_id = NEW.bid_id;
  IF due IS NULL OR total <> due - (due % 10000) THEN
    RAISE EXCEPTION 'refund notes for bid % sum to %, expected % (due % minus dust)',
      NEW.bid_id, total, due - (due % 10000), due;
  END IF;
  RETURN NULL;
END;
$$ LANGUAGE plpgsql;
