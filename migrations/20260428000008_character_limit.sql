-- Per-membership cap on how many characters a player can own in the campaign.
-- Default 1 (death→new character replaces the old one). Master can raise per
-- player. Campaign masters still create/manage many via role check in app code.
alter table memberships add column if not exists character_limit int not null default 1;
