-- A player may die and roll a new character in the same campaign, so the
-- 1-char-per-player constraint is lifted. Characters retain campaign + owner
-- scoping; sorting/filtering on those columns still uses the plain indexes.

drop index if exists characters_one_per_player;
