-- Encounter templates: reusable NPC compositions (spawned into encounters).
create table if not exists encounter_templates (
    id          uuid primary key default gen_random_uuid(),
    campaign_id uuid not null references campaigns(id) on delete cascade,
    name        text not null,
    combatants  jsonb not null default '[]'::jsonb,
    created_at  timestamptz not null default now()
);

-- Sessions can be pinned to an in-game calendar date.
alter table campaign_sessions
    add column if not exists calendar_date text;
