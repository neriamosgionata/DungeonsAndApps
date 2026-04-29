-- Track which round a combatant's token was last moved by a player.
-- NULL = not yet moved this combat. Compared against encounters.round to
-- enforce the once-per-round player movement rule.
alter table combatants
    add column token_moved_round int;
