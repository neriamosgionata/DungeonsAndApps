-- Canonical conditions reference table
create table conditions (
    slug        text primary key,
    name        text not null,
    description text not null,
    mechanics   text -- brief description of mechanical effects
);

insert into conditions (slug, name, description, mechanics) values
    ('blinded', 'Blinded', 'A blinded creature can''t see and automatically fails any ability check that requires sight. Attack rolls against the creature have advantage, and the creature''s attack rolls have disadvantage.', 'attack_dis, attackers_adv'),
    ('charmed', 'Charmed', 'A charmed creature can''t attack the charmer or target them with harmful abilities. The charmer has advantage on social interactions.', 'cant_attack_charmer'),
    ('deafened', 'Deafened', 'A deafened creature can''t hear and automatically fails any ability check that requires hearing.', 'auto_fail_hearing_checks'),
    ('exhaustion', 'Exhaustion', 'Exhaustion has 6 levels. 1: disadvantage on ability checks. 2: speed halved. 3: disadvantage on attack rolls and saves. 4: hit point maximum halved. 5: speed 0. 6: death.', 'levels_1_to_6'),
    ('frightened', 'Frightened', 'A frightened creature has disadvantage on ability checks and attack rolls while the source of its fear is within line of sight. The creature can''t willingly move closer.', 'attack_dis, cant_approach_source'),
    ('grappled', 'Grappled', 'A grappled creature''s speed becomes 0, and it can''t benefit from any bonus to speed.', 'speed_0'),
    ('incapacitated', 'Incapacitated', 'An incapacitated creature can''t take actions or reactions.', 'no_actions, no_reactions'),
    ('invisible', 'Invisible', 'An invisible creature is impossible to see. Attack rolls against it have disadvantage, and its attack rolls have advantage.', 'attackers_dis, attack_adv'),
    ('paralyzed', 'Paralyzed', 'A paralyzed creature is incapacitated and can''t move or speak. Attack rolls against it have advantage. Any hit is a critical hit if the attacker is within 5 feet.', 'incapacitated, attackers_adv, auto_crit_5ft'),
    ('petrified', 'Petrified', 'A petrified creature is turned to stone. It is incapacitated, unaware of its surroundings, and has resistance to all damage. Attack rolls against it have advantage.', 'incapacitated, damage_resistance_all, attackers_adv'),
    ('poisoned', 'Poisoned', 'A poisoned creature has disadvantage on attack rolls and ability checks.', 'attack_dis, ability_dis'),
    ('prone', 'Prone', 'A prone creature can only crawl. Attack rolls against it have advantage if within 5 feet, disadvantage if farther. Its attacks have disadvantage.', 'melee_adv, ranged_dis, attack_dis'),
    ('restrained', 'Restrained', 'A restrained creature''s speed becomes 0. Attack rolls against it have advantage, and its attack rolls have disadvantage.', 'speed_0, attackers_adv, attack_dis'),
    ('stunned', 'Stunned', 'A stunned creature is incapacitated, can''t move, and can speak only falteringly. Attack rolls against it have advantage.', 'incapacitated, attackers_adv'),
    ('surprised', 'Surprised', 'A surprised creature can''t move or take an action on its first turn. It can''t take a reaction until after its first turn ends.', 'no_action_turn1, no_reaction_turn1'),
    ('unconscious', 'Unconscious', 'An unconscious creature is incapacitated, can''t move or speak, and is unaware of its surroundings. Attack rolls against it have advantage. Any hit is a critical if within 5 feet.', 'incapacitated, attackers_adv, auto_crit_5ft, prone'),
    ('grappling', 'Grappling', 'The creature is grappling another creature. Its speed is halved, and it can drag the grappled creature.', 'speed_halved, can_drag'),
    ('concentration', 'Concentration', 'Maintaining a spell requires concentration. Taking damage triggers a CON save. Casting another concentration spell ends the first.', 'broken_on_damage, one_at_a_time'),
    ('rage', 'Rage', 'The creature has advantage on STR checks and saves, bonus damage on STR-based attacks, and resistance to bludgeoning, piercing, and slashing damage.', 'str_adv, dmg_bonus, bps_resistance');
