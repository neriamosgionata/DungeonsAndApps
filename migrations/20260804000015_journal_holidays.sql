-- Player journal: private per-author notes (recap companions).
create table if not exists journal_entries (
    id          uuid primary key default gen_random_uuid(),
    campaign_id uuid not null references campaigns(id) on delete cascade,
    author_id   uuid not null references users(id) on delete cascade,
    title       text not null,
    body        text not null default '',
    created_at  timestamptz not null default now(),
    updated_at  timestamptz not null default now()
);
create index on journal_entries(campaign_id, author_id);

-- Calendar holidays (fixed-date festivals) + moon phase names.
alter table campaign_calendar
    add column if not exists holidays jsonb not null default '[]'::jsonb,
    add column if not exists moon_phases jsonb not null default '["New Moon","Waxing Crescent","First Quarter","Waxing Gibbous","Full Moon","Waning Gibbous","Last Quarter","Waning Crescent"]'::jsonb;
