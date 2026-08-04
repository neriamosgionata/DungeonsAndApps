-- Extra Attack tracking: attacks made with the Attack action this turn.
-- Turn-start reset (see routes/combat/encounters/turns.rs) zeroes it.
alter table combatants
    add column if not exists attacks_made_this_turn int not null default 0;
