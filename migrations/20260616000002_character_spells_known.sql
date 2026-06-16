-- Add `known` flag to character_spells for known-spell caster classes
-- (Sorcerer, Bard, Warlock, Ranger, Rogue). Per PHB: known-spell casters
-- learn a fixed subset of spells; the slot-pool check is the only gate.
-- Without this column, players with slots can cast any spell in the DB.
alter table character_spells add column known boolean not null default false;
