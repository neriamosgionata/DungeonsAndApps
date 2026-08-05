-- Tags (campaign-scoped labels, taggable resources).
create table if not exists tags (
    id          uuid primary key default gen_random_uuid(),
    campaign_id uuid not null references campaigns(id) on delete cascade,
    name        text not null,
    color       text not null default '#8b6914',
    unique (campaign_id, name)
);
create table if not exists taggings (
    tag_id        uuid not null references tags(id) on delete cascade,
    resource_type text not null,
    resource_id   uuid not null,
    primary key (tag_id, resource_type, resource_id)
);
create index on taggings(resource_type, resource_id);

-- Calendar weather (free-form text, e.g. "Rainy, cold").
alter table campaign_calendar
    add column if not exists weather text not null default '';
