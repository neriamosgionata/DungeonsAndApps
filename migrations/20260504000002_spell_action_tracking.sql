alter table combatants
    add column if not exists action_spell_level   smallint not null default 0,
    add column if not exists bonus_action_spell_level smallint not null default 0;
