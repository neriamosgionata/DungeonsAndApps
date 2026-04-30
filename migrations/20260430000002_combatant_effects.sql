-- Combatant effects (buffs, debuffs, conditions with precise 5e duration tracking)

create type effect_kind as enum ('buff', 'debuff', 'neutral', 'condition');
create type duration_unit as enum ('rounds', 'minutes', 'hours', 'permanent');
create type tick_trigger as enum (
  'round_end',
  'target_turn_start',
  'target_turn_end',
  'caster_turn_start',
  'caster_turn_end',
  'never'
);

create table combatant_effects (
  id                    uuid primary key default gen_random_uuid(),
  combatant_id          uuid not null references combatants(id) on delete cascade,
  name                  text not null,
  kind                  effect_kind not null,
  icon                  text not null default 'circle-dot',
  duration_unit         duration_unit not null,
  duration_value        int,
  remaining             int,
  tick_trigger          tick_trigger not null default 'round_end',
  concentration         boolean not null default false,
  caster_combatant_id   uuid references combatants(id) on delete set null,
  source_type           text check (source_type in ('spell','ability','item','manual','condition')),
  source_name           text,
  source_spell_slug     text references spells(slug) on delete set null,
  applied_at_round      int not null,
  applied_at_turn_index int not null,
  modifiers             jsonb not null default '{}'::jsonb,
  active                boolean not null default true,
  created_at            timestamptz not null default now()
);

create index idx_effects_combatant_active on combatant_effects(combatant_id) where active = true;
create index idx_effects_concentration on combatant_effects(caster_combatant_id) where concentration = true and active = true;
