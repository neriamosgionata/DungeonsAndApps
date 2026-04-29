-- Toggle grid overlay visibility on the battle map.
alter table encounters
    add column show_grid boolean not null default false;
