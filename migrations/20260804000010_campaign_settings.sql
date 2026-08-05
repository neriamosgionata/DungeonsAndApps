-- Campaign settings / house rules (jsonb, master-editable).
alter table campaigns
    add column if not exists settings jsonb not null default '{}'::jsonb;
