-- Add effect templates to spells for auto-casting buffs/debuffs
alter table spells add column if not exists effects jsonb not null default '[]'::jsonb;
