-- Add fog_of_war zone_type for battle map fog-of-war overlays
alter table encounter_overlays
    drop constraint if exists encounter_overlays_zone_type_check;

alter table encounter_overlays
    add constraint encounter_overlays_zone_type_check
    check (zone_type in (
        'difficult_terrain','low_visibility','no_visibility',
        'magical_darkness','fire','ice','water','poison','hazard',
        'fog_of_war'
    ));
