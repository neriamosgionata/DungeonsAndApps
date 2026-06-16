-- Queue of pending reaction snapshots per combatant.
-- Populated on every successful hit; consumed by Shield / Uncanny Dodge reactions
-- (PHB p.195: a hit triggers a window where the target can react).
-- Cleared on turn-start reset.
-- Each entry: {attacker_id, attack_total, damage_total, created_at_round}
alter table combatants add column pending_hits jsonb not null default '[]'::jsonb;
