-- Map pins may carry a raster/vector icon URL for richer tokens.
alter table map_pins add column if not exists icon_url text;
