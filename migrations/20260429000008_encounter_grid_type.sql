-- Grid type for the battle map: square (default) or hex.
alter table encounters
    add column grid_type text not null default 'square'
    check (grid_type in ('square', 'hex'));
