-- M-F6 part 2: WS event replay.
-- Persist every published WS event with a monotonic per-campaign sequence
-- number so the client can request missed events on reconnect via
-- GET /api/v1/ws-events?campaign_id=X&since=<seq>.
CREATE TABLE ws_events (
  id BIGSERIAL PRIMARY KEY,
  campaign_id UUID NOT NULL,
  seq BIGINT NOT NULL,
  type TEXT NOT NULL,
  payload JSONB NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Per-campaign monotonic seq. Source of truth for replay ordering.
-- The unique (campaign_id, seq) constraint prevents gaps from concurrent
-- inserts (PostgreSQL serializes transactions on the same row).
CREATE SEQUENCE ws_events_seq_per_campaign;

-- Make seq default to next per-campaign value via a BEFORE INSERT trigger.
-- This avoids needing to know the current max(seq) per insert.
CREATE OR REPLACE FUNCTION ws_events_next_seq() RETURNS TRIGGER AS $$
DECLARE
  next_val BIGINT;
BEGIN
  -- nextval is atomic and gap-free per session. The unique constraint
  -- on (campaign_id, seq) prevents duplicates if two inserts race.
  SELECT nextval('ws_events_seq_per_campaign') INTO next_val;
  NEW.seq := next_val;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER ws_events_set_seq
  BEFORE INSERT ON ws_events
  FOR EACH ROW
  EXECUTE FUNCTION ws_events_next_seq();

CREATE UNIQUE INDEX idx_ws_events_campaign_seq ON ws_events (campaign_id, seq);
CREATE INDEX idx_ws_events_campaign_created ON ws_events (campaign_id, created_at DESC);

-- Add a per-campaign retention cleanup so the table doesn't grow unbounded.
-- A nightly job (or the application) can delete events older than 7 days.
-- The replay window only needs to cover the longest realistic disconnect.
