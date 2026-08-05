-- Homebrew spells: campaign-scoped spell catalog (master-editable).
create table if not exists campaign_spells (
    campaign_id uuid not null references campaigns(id) on delete cascade,
    slug        text not null,
    name        text not null,
    level       smallint not null default 0,
    school      text not null default 'Evocation',
    casting_time text,
    range_text   text,
    components   text,
    duration     text,
    classes     text[] not null default '{}',
    ritual      boolean not null default false,
    concentration boolean not null default false,
    description text not null default '',
    higher_levels text,
    source      text not null default 'homebrew',
    effects     jsonb not null default '{}'::jsonb,
    created_at  timestamptz not null default now(),
    updated_at  timestamptz not null default now(),
    primary key (campaign_id, slug)
);

-- Campaign archive/restore (soft delete).
alter table campaigns
    add column if not exists archived_at timestamptz null;
