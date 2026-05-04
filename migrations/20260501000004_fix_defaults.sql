-- Fix campaign_sessions.status default from 'played' to 'planned'
ALTER TABLE campaign_sessions ALTER COLUMN status SET DEFAULT 'planned';

-- Fix visibility defaults that were accidentally changed to 'master' in migration 20260429000003
-- These should be 'players' so that newly created content is visible to the party by default.
ALTER TABLE campaign_sessions ALTER COLUMN visibility SET DEFAULT 'players';
ALTER TABLE news_entries ALTER COLUMN visibility SET DEFAULT 'players';
ALTER TABLE maps ALTER COLUMN visibility SET DEFAULT 'players';
ALTER TABLE map_pins ALTER COLUMN visibility SET DEFAULT 'players';

-- Backfill existing rows that got the wrong default.
UPDATE campaign_sessions SET visibility = 'players' WHERE visibility = 'master';
UPDATE news_entries       SET visibility = 'players' WHERE visibility = 'master';
UPDATE maps               SET visibility = 'players' WHERE visibility = 'master';
UPDATE map_pins           SET visibility = 'players' WHERE visibility = 'master';
