-- App-wide role rename: master → admin. Idempotent.

do $$
begin
  if exists (
    select 1 from pg_type t join pg_enum e on e.enumtypid = t.oid
    where t.typname = 'user_role' and e.enumlabel = 'master'
  ) then
    alter type user_role rename value 'master' to 'admin';
  end if;
end $$;

-- campaign invitations — awaiting accept/decline before becoming memberships
create table if not exists campaign_invitations (
    id              uuid primary key default gen_random_uuid(),
    campaign_id     uuid not null references campaigns(id) on delete cascade,
    user_id         uuid not null references users(id) on delete cascade,
    role            membership_role not null default 'player',
    invited_by      uuid references users(id) on delete set null,
    message         text,
    created_at      timestamptz not null default now(),
    responded_at    timestamptz,
    accepted        boolean,
    unique (campaign_id, user_id)
);
create index if not exists campaign_invitations_pending_idx on campaign_invitations(user_id) where responded_at is null;
create index if not exists campaign_invitations_campaign_idx on campaign_invitations(campaign_id);
