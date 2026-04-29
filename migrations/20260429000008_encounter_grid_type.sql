-- Grid type for the battle map: square (default) or hex.
alter table encounters
    add column if not exists grid_type text not null default 'square';

do $$ begin
  alter table encounters add constraint encounters_grid_type_check
    check (grid_type in ('square', 'hex'));
exception when duplicate_object then null;
end $$;
