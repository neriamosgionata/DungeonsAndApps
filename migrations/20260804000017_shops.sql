-- Shops / merchants: GM-created vendors with inventories.
create table if not exists shops (
    id          uuid primary key default gen_random_uuid(),
    campaign_id uuid not null references campaigns(id) on delete cascade,
    name        text not null,
    description text not null default '',
    npc_id      uuid references npcs(id) on delete set null,
    visibility  visibility not null default 'players',
    created_at  timestamptz not null default now()
);
create table if not exists shop_items (
    id          uuid primary key default gen_random_uuid(),
    shop_id     uuid not null references shops(id) on delete cascade,
    name        text not null,
    price_gp    numeric(12,2) not null default 0,
    quantity    int,  -- null = unlimited
    item_slug   text,
    created_at  timestamptz not null default now()
);
