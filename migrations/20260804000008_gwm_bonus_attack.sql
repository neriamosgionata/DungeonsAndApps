-- GWM bonus attack: granted when a GWM attack crits or kills (PHB p.167);
-- the bonus attack is a weapon attack via bonus action, consumed once.
alter table combatants
    add column if not exists gwm_bonus_attack_available bool not null default false;
