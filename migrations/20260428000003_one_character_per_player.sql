-- A player may own at most one character per campaign.
-- Masters can still create many (e.g. managing NPCs as "characters")
-- so the uniqueness is scoped by (campaign, owner) with no role distinction —
-- enforcement for masters happens in app code when creating for another owner.

create unique index characters_one_per_player
    on characters (campaign_id, owner_id);
