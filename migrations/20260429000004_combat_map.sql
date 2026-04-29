-- Battle map on encounters + token position on combatants.
-- Tokens use percentage coords (0-100) over the uploaded map image.
alter table encounters
    add column if not exists map_image text,
    add column if not exists map_grid_size int not null default 50;

alter table combatants
    add column if not exists token_x real,
    add column if not exists token_y real,
    add column if not exists token_color text,
    add column if not exists token_on_map boolean not null default false;
