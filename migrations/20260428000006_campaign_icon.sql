-- Campaigns may carry an icon/banner image URL.
alter table campaigns add column if not exists icon_url text;
