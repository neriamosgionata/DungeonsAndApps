-- In-game calendar (world date tracking, PHB-agnostic).
create table if not exists campaign_calendar (
    campaign_id uuid primary key references campaigns(id) on delete cascade,
    year   int not null default 1492,
    month  int not null default 1,
    day    int not null default 1,
    days_per_month int not null default 30,
    months jsonb not null default '["Hammer","Alturiak","Ches","Tarsakh","Mirtul","Kythorn","Flamerule","Eleasis","Eleint","Marpenoth","Uktar","Nightal"]'::jsonb,
    weekdays jsonb not null default '["Monday","Tuesday","Wednesday","Thursday","Friday","Saturday","Sunday"]'::jsonb,
    notes  text not null default '',
    updated_at timestamptz not null default now()
);
