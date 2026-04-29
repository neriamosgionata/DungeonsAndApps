-- Master decides XP vs Milestone leveling per campaign. When 'milestone' is
-- set, the character sheet hides the XP input and the master levels players
-- manually.
create type leveling_mode as enum ('xp', 'milestone');
alter table campaigns
    add column if not exists leveling leveling_mode not null default 'xp';
