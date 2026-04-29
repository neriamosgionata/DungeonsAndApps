-- Toggle grid overlay visibility on the battle map.
alter table encounters
    add column if not exists show_grid boolean not null default false;
