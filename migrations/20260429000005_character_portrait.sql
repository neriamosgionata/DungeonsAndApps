-- Character portrait — used for combat tokens and anywhere a small avatar
-- is rendered next to the character.
alter table characters
    add column if not exists portrait_url text;
