-- Allow ad-hoc combatants (e.g. master quick-adds a monster without creating
-- a persistent NPC). Rules:
--   ref_type = 'character' → character_id NOT NULL, npc_id NULL
--   ref_type = 'npc'       → character_id NULL (npc_id may be null for ad-hoc)
alter table combatants drop constraint if exists combatants_check;
alter table combatants add constraint combatants_check check (
    (ref_type = 'character' and character_id is not null and npc_id is null)
    or
    (ref_type = 'npc' and character_id is null)
);
