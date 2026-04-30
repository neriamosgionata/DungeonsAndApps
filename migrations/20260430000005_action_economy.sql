-- Action economy tracking for 5e combat

alter table combatants
  add column action_used boolean not null default false,
  add column bonus_action_used boolean not null default false,
  add column reaction_used boolean not null default false,
  add column movement_used_ft int not null default 0,
  add column legendary_actions_max int not null default 0,
  add column legendary_actions_used int not null default 0,
  add column legendary_resistances_max int not null default 0,
  add column legendary_resistances_used int not null default 0;

-- Lair action at encounter level
alter table encounters
  add column lair_action_used boolean not null default false;
