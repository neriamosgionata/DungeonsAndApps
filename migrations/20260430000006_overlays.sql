-- Encounter overlays: spell AoE shapes + persistent dynamic zones

create table encounter_overlays (
  id                    uuid primary key default gen_random_uuid(),
  encounter_id          uuid not null references encounters(id) on delete cascade,
  kind                  text not null check (kind in ('aoe','zone')),
  shape                 text not null check (shape in ('circle','cone','line','cube','polygon')),
  origin_x              float not null,
  origin_y              float not null,
  end_x                 float,
  end_y                 float,
  radius_ft             int,
  length_ft             int,
  width_ft              int,
  angle_deg             float,
  points                jsonb,                      -- polygon vertices [{x,y}]
  color                 text not null default 'rgba(255,0,0,0.25)',
  label                 text,
  zone_type             text check (zone_type in (
    'difficult_terrain','low_visibility','no_visibility',
    'magical_darkness','fire','ice','water','poison'
  )),
  active                boolean not null default true,
  expires_at_round      int,
  expires_at_turn       int,
  source_spell_slug     text references spells(slug) on delete set null,
  created_by_combatant_id uuid references combatants(id) on delete set null,
  created_at            timestamptz not null default now()
);

create index idx_overlays_encounter on encounter_overlays(encounter_id) where active = true;
