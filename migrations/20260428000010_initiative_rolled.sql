-- Track whether a combatant has rolled initiative yet.
-- Auto-added party characters start with initiative_rolled = false; they
-- cannot take turns until their owner rolls.
alter table combatants
    add column if not exists initiative_rolled boolean not null default true;

-- Past rows are considered already rolled (default true).
