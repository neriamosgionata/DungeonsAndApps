-- level_override lets the GM override the effective level used for proficiency bonus
-- in combat calculations (e.g. for custom NPCs without a structured stat block).
-- 0 = derive from character sheet / npc stats as usual.
ALTER TABLE combatants ADD COLUMN level_override int NOT NULL DEFAULT 0;
