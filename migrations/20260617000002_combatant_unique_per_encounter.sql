-- LOW-4: prevent duplicate combatants in the same encounter.
--
-- A character or NPC can be added to multiple encounters, but only ONCE per
-- encounter. Without this constraint, a master could drag the same Boblin
-- onto the initiative tracker twice and double-count HP/damage.
--
-- Partial unique indexes handle the nullable character_id/npc_id cleanly:
--   - character_id is not null → character uniqueness
--   - npc_id is not null → NPC uniqueness
-- The constraints don't cover cross-encounter duplicates (intentional).

create unique index if not exists combatants_encounter_character_uniq
    on combatants (encounter_id, character_id)
    where character_id is not null;

create unique index if not exists combatants_encounter_npc_uniq
    on combatants (encounter_id, npc_id)
    where npc_id is not null;
