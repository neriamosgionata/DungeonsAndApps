-- Wild Shape: store original combatant stats for revert
alter table combatants
    add column if not exists wild_shape_original jsonb;
