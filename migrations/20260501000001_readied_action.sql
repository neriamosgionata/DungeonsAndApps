-- Add readied_action field to combatants
alter table combatants add column readied_action jsonb default null;

-- Add cover_bonus field (for auto-calculated cover)
alter table combatants add column cover_bonus int default 0;

-- Add delayed_turn flag
alter table combatants add column delayed_turn boolean default false;
