-- Shrink the visibility scheme to just 'master' and 'players'.
-- 'private' ⇒ 'master' (master-only).
-- 'public' ⇒ 'players' (all members see it; we no longer distinguish
--                       in-campaign vs world-public).
--
-- Postgres enum migration: add new values, migrate data via ALTER TABLE with
-- USING cast, swap defaults, drop the old enum.

-- 1. create the new enum
create type visibility_new as enum ('master', 'players');

-- 2. convert every column to the new enum
alter table campaign_sessions
    alter column visibility drop default,
    alter column visibility type visibility_new using (
        case visibility::text
            when 'private' then 'master'::visibility_new
            when 'public'  then 'players'::visibility_new
            else visibility::text::visibility_new
        end
    ),
    alter column visibility set default 'master'::visibility_new;

alter table factions
    alter column visibility drop default,
    alter column visibility type visibility_new using (
        case visibility::text
            when 'private' then 'master'::visibility_new
            when 'public'  then 'players'::visibility_new
            else visibility::text::visibility_new
        end
    ),
    alter column visibility set default 'master'::visibility_new;

alter table npcs
    alter column visibility drop default,
    alter column visibility type visibility_new using (
        case visibility::text
            when 'private' then 'master'::visibility_new
            when 'public'  then 'players'::visibility_new
            else visibility::text::visibility_new
        end
    ),
    alter column visibility set default 'master'::visibility_new;

alter table lore_entries
    alter column visibility drop default,
    alter column visibility type visibility_new using (
        case visibility::text
            when 'private' then 'master'::visibility_new
            when 'public'  then 'players'::visibility_new
            else visibility::text::visibility_new
        end
    ),
    alter column visibility set default 'master'::visibility_new;

alter table news_entries
    alter column visibility drop default,
    alter column visibility type visibility_new using (
        case visibility::text
            when 'private' then 'master'::visibility_new
            when 'public'  then 'players'::visibility_new
            else visibility::text::visibility_new
        end
    ),
    alter column visibility set default 'master'::visibility_new;

alter table maps
    alter column visibility drop default,
    alter column visibility type visibility_new using (
        case visibility::text
            when 'private' then 'master'::visibility_new
            when 'public'  then 'players'::visibility_new
            else visibility::text::visibility_new
        end
    ),
    alter column visibility set default 'master'::visibility_new;

alter table map_pins
    alter column visibility drop default,
    alter column visibility type visibility_new using (
        case visibility::text
            when 'private' then 'master'::visibility_new
            when 'public'  then 'players'::visibility_new
            else visibility::text::visibility_new
        end
    ),
    alter column visibility set default 'master'::visibility_new;

alter table quests
    alter column visibility drop default,
    alter column visibility type visibility_new using (
        case visibility::text
            when 'private' then 'master'::visibility_new
            when 'public'  then 'players'::visibility_new
            else visibility::text::visibility_new
        end
    ),
    alter column visibility set default 'players'::visibility_new;

-- 3. retire the old enum and rename
drop type visibility;
alter type visibility_new rename to visibility;
