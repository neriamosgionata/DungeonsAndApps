-- Extend overlays zone_type enum to include hazard, and add per-turn damage fields
alter table encounter_overlays
    drop constraint if exists encounter_overlays_zone_type_check;

alter table encounter_overlays
    add constraint encounter_overlays_zone_type_check
    check (zone_type in (
        'difficult_terrain','low_visibility','no_visibility',
        'magical_darkness','fire','ice','water','poison','hazard'
    ));

alter table encounter_overlays
    add column if not exists hazard_damage_expression text,
    add column if not exists hazard_damage_type       text,
    add column if not exists hazard_save_ability      text,
    add column if not exists hazard_save_dc           int,
    add column if not exists hazard_half_on_save      boolean not null default false;
