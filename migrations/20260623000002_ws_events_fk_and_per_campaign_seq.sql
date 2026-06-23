-- M-F6 part 2 follow-up: ws_events integrity.
-- Two bugs in the original migration (20260623000001):
--   1) ws_events_seq_per_campaign is a GLOBAL sequence. The trigger
--      function comment claims per-campaign monotonic but the sequence
--      doesn't know about campaign_id. The (campaign_id, seq) unique
--      index prevents duplicate (campaign, seq) pairs but global seq
--      values can still cause confusing gaps after concurrent activity.
--   2) ws_events.campaign_id has no FK to campaigns(id). A campaign
--      delete leaves orphan rows that can never be replayed.
-- Fix: replace the global sequence with a per-campaign advisory-lock +
-- MAX(seq)+1 query. Add the FK with ON DELETE CASCADE.
-- Migration is idempotent: drops and re-creates the trigger function
-- and the trigger itself if they already exist.

-- 1. Add FK to campaigns. CASCADE on delete so the table doesn't grow
--    with orphans. DELETE first to satisfy the constraint in case the
--    original migration left rows pointing at deleted campaigns.
DELETE FROM ws_events
 WHERE campaign_id NOT IN (SELECT id FROM campaigns);

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'ws_events_campaign_fk'
  ) THEN
    ALTER TABLE ws_events
      ADD CONSTRAINT ws_events_campaign_fk
      FOREIGN KEY (campaign_id) REFERENCES campaigns(id) ON DELETE CASCADE;
  END IF;
END $$;

-- 2. Replace the global sequence + trigger with a per-campaign advisory-lock
--    implementation. Drop the old trigger + sequence; recreate the function.
DROP TRIGGER IF EXISTS ws_events_set_seq ON ws_events;
DROP SEQUENCE IF EXISTS ws_events_seq_per_campaign;

CREATE OR REPLACE FUNCTION ws_events_next_seq() RETURNS TRIGGER AS $$
DECLARE
  next_val BIGINT;
BEGIN
  -- Per-campaign advisory lock so concurrent inserts to the same campaign
  -- serialize. hashtext gives a stable 32-bit hash; two campaigns can share
  -- a lock bucket occasionally but that just causes harmless contention.
  PERFORM pg_advisory_xact_lock(hashtext(NEW.campaign_id::text));
  SELECT COALESCE(MAX(seq), 0) + 1 INTO next_val
  FROM ws_events
  WHERE campaign_id = NEW.campaign_id;
  NEW.seq := next_val;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER ws_events_set_seq
  BEFORE INSERT ON ws_events
  FOR EACH ROW
  EXECUTE FUNCTION ws_events_next_seq();
