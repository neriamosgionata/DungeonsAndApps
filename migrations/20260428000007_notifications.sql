-- Per-user notifications — messages, whispers, combat events, news, etc.
-- `kind` is free-form so the app can evolve without enum migrations.
-- `ref_kind` + `ref_id` let the UI deep-link ("open whisper", "open encounter").

create table notifications (
    id           uuid primary key default gen_random_uuid(),
    user_id      uuid not null references users(id) on delete cascade,
    campaign_id  uuid references campaigns(id) on delete cascade,
    kind         text not null,
    title        text not null,
    body         text,
    ref_kind     text,
    ref_id       uuid,
    read_at      timestamptz,
    created_at   timestamptz not null default now()
);

create index on notifications (user_id, created_at desc);
create index on notifications (user_id) where read_at is null;
create index on notifications (campaign_id);
