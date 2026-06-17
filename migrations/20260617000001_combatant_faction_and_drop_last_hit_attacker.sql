-- HIGH-3: heal friendly-only check via per-combatant faction
-- HIGH-6: drop unused last_hit_attacker column
--
-- A combatant is healed by a non-master only if both source and target share
-- the same faction. Faction defaults to 'auto' which derives from ref_type
-- (character=ally, npc=enemy). Master can override per-combatant via the
-- `faction` field on update_combatant.
--
-- The last_hit_attacker column is dead data — set in attack() but never read
-- (pass-2 audit HIGH-6). Drop it. last_hit_attack_total and last_hit_damage
-- stay — they're read by Shield/Uncanny Dodge via the legacy fallback.

alter table combatants add column faction text not null default 'auto';

alter table combatants drop column last_hit_attacker;
