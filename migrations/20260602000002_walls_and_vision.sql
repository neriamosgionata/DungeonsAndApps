-- Add wall zone_type for line-of-sight blocking obstacles
alter table encounter_overlays
    drop constraint if exists encounter_overlays_zone_type_check;

alter table encounter_overlays
    add constraint encounter_overlays_zone_type_check
    check (zone_type in (
        'difficult_terrain','low_visibility','no_visibility',
        'magical_darkness','fire','ice','water','poison','hazard',
        'fog_of_war','wall'
    ));

-- Add vision_range to combatants for token sight radius (in feet, null = default 60)
alter table combatants
    add column if not exists vision_range int;
