-- Add damage_type column to spells table.
-- Pre-fix: backend/src/routes/combat/spells/cast.rs:108 selected
-- `damage_type` from `spells` but the column was never created, so any
-- `cast_spell` request hit a 500 ("column damage_type does not exist").
-- The column is nullable and the code falls through to template-based
-- `detect_damage_type` when the value is None, so SRD data (which
-- doesn't carry it) is unaffected.
alter table spells add column if not exists damage_type text;
