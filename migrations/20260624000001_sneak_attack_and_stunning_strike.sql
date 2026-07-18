-- Sneak Attack + Stunning Strike combat mechanics

-- Track whether rogue has used Sneak Attack this turn (PHB: once per turn)
alter table combatants
    add column if not exists sneak_attack_used_this_turn bool not null default false;
