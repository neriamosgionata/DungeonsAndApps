-- Per-round reaction trigger tracking
alter table combatants
    add column if not exists last_hit_attack_total  int,      -- most recent attack roll that hit this combatant this round
    add column if not exists last_hit_damage        int,      -- pending damage from that hit (before Shield)
    add column if not exists last_hit_attacker      uuid references combatants(id) on delete set null,
    add column if not exists spell_being_cast       text;     -- slug of spell currently being cast (set during cast_spell window)
