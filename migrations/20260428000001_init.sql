-- DungeonsAndApps initial schema
-- d&d 5e campaign manager

create extension if not exists "pgcrypto";
create extension if not exists "citext";

-- ============================================================
-- enums
-- ============================================================
create type user_role as enum ('user', 'admin');
create type membership_role as enum ('player', 'master');
create type visibility as enum ('private', 'players', 'public');
create type session_status as enum ('planned', 'played', 'cancelled');
create type encounter_status as enum ('planned', 'active', 'ended');
create type combatant_ref as enum ('character', 'npc');
create type message_scope as enum ('campaign', 'whisper');
create type quest_status as enum ('active', 'completed', 'failed', 'abandoned');
create type language_code as enum ('en', 'it');

-- ============================================================
-- users & auth
-- ============================================================
create table users (
    id              uuid primary key default gen_random_uuid(),
    email           citext not null unique,
    password_hash   text not null,
    display_name    text not null,
    role            user_role not null default 'user',
    language        language_code not null default 'en',
    avatar_url      text,
    created_at      timestamptz not null default now(),
    updated_at      timestamptz not null default now()
);

create table sessions_auth (
    id              uuid primary key default gen_random_uuid(),
    user_id         uuid not null references users(id) on delete cascade,
    refresh_hash    text not null,
    user_agent      text,
    ip              inet,
    expires_at      timestamptz not null,
    revoked_at      timestamptz,
    created_at      timestamptz not null default now()
);
create index on sessions_auth(user_id);
create index on sessions_auth(expires_at);

-- ============================================================
-- campaigns & membership
-- ============================================================
create table campaigns (
    id              uuid primary key default gen_random_uuid(),
    name            text not null,
    description     text,
    master_id       uuid not null references users(id) on delete restrict,
    created_at      timestamptz not null default now(),
    updated_at      timestamptz not null default now()
);
create index on campaigns(master_id);

create table memberships (
    id              uuid primary key default gen_random_uuid(),
    campaign_id     uuid not null references campaigns(id) on delete cascade,
    user_id         uuid not null references users(id) on delete cascade,
    role            membership_role not null,
    joined_at       timestamptz not null default now(),
    unique (campaign_id, user_id)
);
create index on memberships(user_id);
create index on memberships(campaign_id);

-- ============================================================
-- characters (5e sheet as jsonb + indexed key fields)
-- ============================================================
create table characters (
    id              uuid primary key default gen_random_uuid(),
    campaign_id     uuid not null references campaigns(id) on delete cascade,
    owner_id        uuid not null references users(id) on delete cascade,
    name            text not null,
    race            text,
    class_primary   text,
    level_total     smallint not null default 1 check (level_total between 1 and 20),
    sheet           jsonb not null default '{}'::jsonb,
    created_at      timestamptz not null default now(),
    updated_at      timestamptz not null default now()
);
create index on characters(campaign_id);
create index on characters(owner_id);
create index characters_sheet_gin on characters using gin (sheet jsonb_path_ops);

-- ============================================================
-- recap / sessions (narrative)
-- ============================================================
create table campaign_sessions (
    id              uuid primary key default gen_random_uuid(),
    campaign_id     uuid not null references campaigns(id) on delete cascade,
    title           text not null,
    session_number  int,
    played_at       date,
    status          session_status not null default 'played',
    recap           text,
    visibility      visibility not null default 'players',
    created_by      uuid not null references users(id),
    created_at      timestamptz not null default now(),
    updated_at      timestamptz not null default now()
);
create index on campaign_sessions(campaign_id);

-- ============================================================
-- maps (world state + pins)
-- ============================================================
create table maps (
    id              uuid primary key default gen_random_uuid(),
    campaign_id     uuid not null references campaigns(id) on delete cascade,
    name            text not null,
    description     text,
    image_key       text,
    width           int,
    height          int,
    visibility      visibility not null default 'players',
    created_at      timestamptz not null default now(),
    updated_at      timestamptz not null default now()
);
create index on maps(campaign_id);

create table map_pins (
    id              uuid primary key default gen_random_uuid(),
    map_id          uuid not null references maps(id) on delete cascade,
    label           text not null,
    kind            text not null,
    faction_id      uuid,
    is_party        boolean not null default false,
    x               double precision not null,
    y               double precision not null,
    color           text,
    note            text,
    visibility      visibility not null default 'players',
    created_at      timestamptz not null default now(),
    updated_at      timestamptz not null default now()
);
create index on map_pins(map_id);
create index on map_pins(faction_id);

-- ============================================================
-- world info: factions, npcs, lore, news
-- ============================================================
create table factions (
    id              uuid primary key default gen_random_uuid(),
    campaign_id     uuid not null references campaigns(id) on delete cascade,
    name            text not null,
    banner_color    text,
    description     text,
    attitude        text,
    visibility      visibility not null default 'private',
    created_at      timestamptz not null default now(),
    updated_at      timestamptz not null default now()
);
create index on factions(campaign_id);

alter table map_pins
    add constraint map_pins_faction_fk
    foreign key (faction_id) references factions(id) on delete set null;

create table npcs (
    id              uuid primary key default gen_random_uuid(),
    campaign_id     uuid not null references campaigns(id) on delete cascade,
    name            text not null,
    role            text,
    faction_id      uuid references factions(id) on delete set null,
    description     text,
    stats           jsonb not null default '{}'::jsonb,
    image_key       text,
    visibility      visibility not null default 'private',
    created_at      timestamptz not null default now(),
    updated_at      timestamptz not null default now()
);
create index on npcs(campaign_id);
create index on npcs(faction_id);

create table lore_entries (
    id              uuid primary key default gen_random_uuid(),
    campaign_id     uuid not null references campaigns(id) on delete cascade,
    title           text not null,
    category        text,
    body            text not null,
    visibility      visibility not null default 'private',
    created_at      timestamptz not null default now(),
    updated_at      timestamptz not null default now()
);
create index on lore_entries(campaign_id);

create table news_entries (
    id              uuid primary key default gen_random_uuid(),
    campaign_id     uuid not null references campaigns(id) on delete cascade,
    title           text not null,
    body            text not null,
    published_at    timestamptz not null default now(),
    visibility      visibility not null default 'players',
    created_at      timestamptz not null default now(),
    updated_at      timestamptz not null default now()
);
create index on news_entries(campaign_id);

-- ============================================================
-- spells (SRD 5.1 — ingested from shared/spells-srd.json)
-- global catalog, not per-campaign
-- ============================================================
create table spells (
    id              uuid primary key default gen_random_uuid(),
    slug            text not null unique,
    name            text not null,
    level           smallint not null check (level between 0 and 9),
    school          text not null,
    casting_time    text,
    range_text      text,
    components      text,
    duration        text,
    classes         text[] not null default '{}',
    ritual          boolean not null default false,
    concentration   boolean not null default false,
    description     text not null,
    higher_levels   text,
    source          text not null default 'SRD 5.1',
    i18n            jsonb not null default '{}'::jsonb
);
create index on spells(level);
create index on spells using gin (classes);

-- character-known spells (link)
create table character_spells (
    character_id    uuid not null references characters(id) on delete cascade,
    spell_id        uuid not null references spells(id) on delete cascade,
    prepared        boolean not null default false,
    notes           text,
    primary key (character_id, spell_id)
);

-- ============================================================
-- group / party shared state
-- ============================================================
create table parties (
    id              uuid primary key default gen_random_uuid(),
    campaign_id     uuid not null unique references campaigns(id) on delete cascade,
    cp              bigint not null default 0,
    sp              bigint not null default 0,
    ep              bigint not null default 0,
    gp              bigint not null default 0,
    pp              bigint not null default 0,
    shared_notes    text,
    updated_at      timestamptz not null default now()
);

create table loot_items (
    id              uuid primary key default gen_random_uuid(),
    party_id        uuid not null references parties(id) on delete cascade,
    name            text not null,
    description     text,
    quantity        int not null default 1 check (quantity >= 0),
    value_gp        numeric(12,2),
    claimed_by      uuid references characters(id) on delete set null,
    created_at      timestamptz not null default now()
);
create index on loot_items(party_id);

create table quests (
    id              uuid primary key default gen_random_uuid(),
    campaign_id     uuid not null references campaigns(id) on delete cascade,
    title           text not null,
    description     text,
    status          quest_status not null default 'active',
    reward          text,
    visibility      visibility not null default 'players',
    created_at      timestamptz not null default now(),
    updated_at      timestamptz not null default now()
);
create index on quests(campaign_id);

create table quest_npcs (
    quest_id        uuid not null references quests(id) on delete cascade,
    npc_id          uuid not null references npcs(id) on delete cascade,
    role            text,
    primary key (quest_id, npc_id)
);

-- ============================================================
-- messages (chat + whispers)
-- ============================================================
create table messages (
    id              uuid primary key default gen_random_uuid(),
    campaign_id     uuid not null references campaigns(id) on delete cascade,
    sender_id       uuid not null references users(id) on delete cascade,
    recipient_id    uuid references users(id) on delete cascade,
    scope           message_scope not null,
    body            text not null,
    created_at      timestamptz not null default now(),
    edited_at       timestamptz,
    deleted_at      timestamptz,
    check ((scope = 'whisper' and recipient_id is not null)
        or (scope = 'campaign' and recipient_id is null))
);
create index on messages(campaign_id, created_at desc);
create index on messages(recipient_id, created_at desc);
create index on messages(sender_id);

-- ============================================================
-- dice rolls (server-authoritative)
-- ============================================================
create table dice_rolls (
    id              uuid primary key default gen_random_uuid(),
    campaign_id     uuid not null references campaigns(id) on delete cascade,
    user_id         uuid not null references users(id) on delete cascade,
    character_id    uuid references characters(id) on delete set null,
    expression      text not null,
    results         jsonb not null,
    total           int not null,
    label           text,
    private         boolean not null default false,
    rolled_at       timestamptz not null default now()
);
create index on dice_rolls(campaign_id, rolled_at desc);
create index on dice_rolls(user_id);

-- ============================================================
-- initiative / combat
-- ============================================================
create table encounters (
    id              uuid primary key default gen_random_uuid(),
    campaign_id     uuid not null references campaigns(id) on delete cascade,
    name            text not null,
    status          encounter_status not null default 'planned',
    round           int not null default 0,
    turn_index      int not null default 0,
    notes           text,
    created_at      timestamptz not null default now(),
    updated_at      timestamptz not null default now()
);
create index on encounters(campaign_id);

create table combatants (
    id              uuid primary key default gen_random_uuid(),
    encounter_id    uuid not null references encounters(id) on delete cascade,
    ref_type        combatant_ref not null,
    character_id    uuid references characters(id) on delete set null,
    npc_id          uuid references npcs(id) on delete set null,
    display_name    text not null,
    initiative      int not null default 0,
    dex_tiebreaker  smallint not null default 10,
    hp_current      int not null default 0,
    hp_max          int not null default 0,
    temp_hp         int not null default 0,
    ac              int not null default 10,
    conditions      text[] not null default '{}',
    notes           text,
    is_visible      boolean not null default true,
    turn_order      int not null default 0,
    check (
        (ref_type = 'character' and character_id is not null and npc_id is null) or
        (ref_type = 'npc' and npc_id is not null and character_id is null)
    )
);
create index on combatants(encounter_id);

create table combat_events (
    id              uuid primary key default gen_random_uuid(),
    encounter_id    uuid not null references encounters(id) on delete cascade,
    round           int not null,
    actor_combatant uuid references combatants(id) on delete set null,
    target_combatant uuid references combatants(id) on delete set null,
    action          text not null,
    roll_id         uuid references dice_rolls(id) on delete set null,
    delta_hp        int,
    note            text,
    created_at      timestamptz not null default now()
);
create index on combat_events(encounter_id, created_at);

-- ============================================================
-- updated_at trigger
-- ============================================================
create or replace function touch_updated_at() returns trigger as $$
begin
    new.updated_at = now();
    return new;
end; $$ language plpgsql;

do $$
declare t text;
begin
    for t in
        select table_name from information_schema.columns
        where column_name = 'updated_at' and table_schema = 'public'
    loop
        execute format(
            'create trigger trg_%I_updated before update on %I for each row execute function touch_updated_at()',
            t, t
        );
    end loop;
end $$;
