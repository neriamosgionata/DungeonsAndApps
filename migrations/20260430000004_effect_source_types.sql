-- Expand combatant_effects source_type to include 'weapon'

alter table combatant_effects drop constraint if exists combatant_effects_source_type_check;

alter table combatant_effects add constraint combatant_effects_source_type_check
  check (source_type in ('spell','ability','item','weapon','manual','condition'));
