-- Battle map on encounters + token position on combatants.
-- Tokens use percentage coords (0-100) over the uploaded map image.
alter table encounters
    add column map_image text,
    add column map_grid_size int not null default 50;

alter table combatants
    add column token_x real,
    add column token_y real,
    add column token_color text,
    add column token_on_map boolean not null default false;
