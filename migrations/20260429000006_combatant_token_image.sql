-- Per-combatant token image override. Falls back to character.portrait_url
-- for character combatants, npc.image_key for NPC combatants, then initials.
alter table combatants
    add column token_image text;
