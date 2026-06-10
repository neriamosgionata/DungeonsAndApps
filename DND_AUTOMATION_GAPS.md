# D&D 5e PHB/DMG Automation Gaps

> Generated: 2026-04-30 | Last updated: 2026-06-09 (hit die defaults, onboarding tooltips, test suite expansion)
> Scope: Combat engine + character sheet + rest mechanics vs PHB/DMG

---

## Combat Automation (24 items)

| # | Feature | Status | Detail |
|---|---------|--------|--------|
| 1 | Auto AC from equipped gear | ✅ | Frontend: armor type selector (light/medium/heavy/unarmored_barbarian/monk/mage_armor/draconic/natural), AC base, max DEX cap, shield toggle. Backend: `compute_ac_from_sheet()` in combat_engine.rs handles all armor types with DEX caps, unarmored defense (Barb 10+DEX+CON, Monk 10+DEX+WIS), mage armor (13+DEX), natural armor, plus shield bonus (+2). Magic armor +1/+2/+3 via attunement `bonuses.ac`. |
| 2 | Attack calculation | ✅ | Prof bonus + ability mod (STR melee, DEX ranged, max STR/DEX finesse) auto. Fighting Styles auto (Archery +2, Dueling +2, GWF reroll 1–2, TWF). Power Attack (−5/+10) via `power_attack: true`. Flanking auto-apply advantage. Bless +1d4, Bardic Inspiration +1d6–12 added to roll. Magic weapon +1/+2/+3 via attunement `bonuses.attack`. |
| 3 | Damage calculation | ✅ | Crit doubles dice. Resistances/immunities work. Extra damage (`extra_damage_expression`) handles Sneak Attack, Smite, Rage. Auto ability mod on weapon damage via `compute_weapon_damage_expression`. Versatile auto-selection (two-handed mode). Magic weapon +1/+2/+3 via attunement `bonuses.damage`. Attunement ability score bonuses (str/dex) applied to attack_bonus and damage_bonus. |
| 4 | Save calculation | ✅ | Ability mod + proficiency + effect bonuses. |
| 5 | Skill check | ✅ | Proficiency + expertise work. Reliable Talent (Rogue 11+): floor-10 enforced in `resolve_skill_check`. Jack of All Trades (Bard 2+): `pb/2` added to non-proficient skills in `resolve_skill_check`. Frontend `hasJackOfAllTrades` threshold fixed to 2. |
| 6 | Action economy | ✅ | Action, bonus action, reaction, movement, legendary, lair all tracked. Legendary actions reset per turn (DMG RAW: "at the start of the creature's turn") not per round. |
| 7 | Opportunity attacks / reactions / ready / delay | ✅ | All implemented with proper economy checks. |
| 8 | Conditions auto-applied | ⚠️ | Blinded, Paralyzed, Restrained, Frightened, Poisoned, Grappled, Invisible, Surprised handled. Prone: attacker dis on ALL attacks (incl. ranged); target prone → melee adv / ranged dis. Timed conditions `name:N` tick down at turn start. Condition immunity enforced by creature type. Grapple auto-releases on incapacitation. Cover auto-computed from token positions (blockers between attacker/target). Flanking auto-applies advantage. **Missing:** Dim-light/darkness beyond overlay zones. |
| 9 | Death saves | ✅ | Nat 20 → stabilize + 1 HP. Nat 1 → 2 failures. Tracked correctly. |
| 10 | Concentration checks | ✅ | Auto-roll CON save, DC = max(10, dmg/2). Auto-break on fail. |
| 11 | Multiattack | ✅ | Batch manual attacks. `parse_multiattack` reads NPC multiattack action → sub-attacks (attack expression, damage, type). `multiattack` endpoint auto-populates attack expressions from parsed NPC multiattack when targets lack custom expressions. |
| 12 | Spell attacks / saves | ✅ | `spell_attack_bonus` and `spell_save_dc` auto-computed from prof + casting mod. |
| 13 | Temporary HP | ✅ | Absorbs damage first. Only replaces if new > current (PHB rule enforced in `update_combatant`). |
| 14 | Resistance/immunity/vulnerability | ✅ | Half/zero/double damage. Supports "nonmagical" variants. |
| 15 | Invisible attackers | ✅ | Attacker advantage, target causes disadvantage. |
| 16 | Prone attackers | ✅ | Disadvantage on ALL attack rolls when attacker prone. |
| 17 | Range increments | ✅ | Long-range disadvantage automated in `attack()` handler. Long range maximum enforced (attack blocked beyond long range). Range check also added to TWF off-hand attacks. |
| 18 | Ammunition tracking | ✅ | Arrows/bolts/bullets auto-decrement. Thrown weapons (daggers, javelins, handaxes) auto-decrement on attack and TWF. `skip_ammo` flag to bypass. |
| 19 | Two-weapon fighting | ✅ | Bonus-action off-hand attack via `/two-weapon-fight`. Ability mod stripped unless TWF fighting style. `twf_style` auto-detected. Range check + thrown weapon tracking added. |
| 20 | Dodge / Disengage | ✅ | Dodge = attackers disadvantaged. Disengage = no opportunity attacks. |
| 21 | Help action | ✅ | Gives advantage on next attack + advantage on next skill check (`save_advantage` modifier applied to target via combatant_effect). |
| 22 | Hide action | ✅ | `hide` applies Hidden effect. `contested_hide` endpoint (`/combatants/{id}/contested-hide`) rolls Stealth vs Passive Perception of all enemies, only applies Hidden if unseen by all observers. |
| 23 | Surprise round | ✅ | `surprised` condition blocks full turn (action+BA+movement set to max at turn start, condition removed). Auto Stealth vs Passive Perception check via `/encounters/{id}/surprise-auto` — rolls Stealth for ambushers, compares to PP of defenders, applies surprised condition. |
| 24 | Darkvision / dim light | ⚠️ | Overlay zones (magical_darkness, low_visibility) cause disadvantage if attacker lacks darkvision. **Missing:** dim-light/darkness beyond overlay zones. |
| 25 | Battle Map fog of war | ✅ | Fog of war overlay (`zone_type = 'fog_of_war'`) with circle/cube shapes. Renders as dark (rgba 0,0,0,0.75) fill on map. GM places/removes via toolbar button. |
| 26 | Wall obstacles / line-of-sight | ✅ | Wall overlay (`zone_type = 'wall'`) with line shape. Renders as thick brown line on map. Attack handler checks if wall segment intersects attacker-target line → blocks attack. GM places via toolbar button. `segments_intersect()` in combat.rs. |
| 27 | Token auras / status HUD | ✅ | Tokens with active effects show pulsing aura ring (brass-gold). Effect badges displayed below token. HP bar color-coded by ratio. Uploadable token images. |

---

## Character Sheet Automation (47 items)

| # | Feature | Status | Detail |
|---|---------|--------|--------|
| 1 | Store STR/DEX/CON/INT/WIS/CHA | ✅ | In `sheet.abilities` JSONB. |
| 2 | Auto-calculate ability mods | ✅ | `floor((score-10)/2)` frontend and backend. |
| 3 | Override ability mods | ⚠️ | Frontend supports `abilities_override`. **Backend ignores overrides** → combat rolls disagree with sheet. |
| 4 | Proficiency bonus auto-scale | ✅ | `2 + floor((level-1)/4)` both sides. |
| 5 | Multiclass prof bonus | ⚠️ | Uses `level_total` field. **Not auto-summed** from class levels — user must maintain manually. |
| 6 | All 18 PHB skills listed | ✅ | Hardcoded array. |
| 7 | Skill bonus auto-calc | ✅ | Ability mod + prof (or 2× prof for expertise). |
| 8 | Skill ability mapping | ✅ | Hardcoded. |
| 9 | Tool proficiencies | ✅ | Structured `{name, ability, proficient, expert}` with auto-bonus display. Ability mod + prof + expertise computed inline. |
| 10 | All 6 saves listed | ✅ | |
| 11 | Save bonus auto-calc | ✅ | Ability mod + prof if proficient. |
| 12 | Conditional save bonuses | ⚠️ | Backend checks effect modifiers. Frontend shows static total only. |
| 13 | Initiative from DEX | ⚠️ | Backend auto from DEX. Frontend defaults to DEX but user can override — can diverge. |
| 14 | AC from armor + shield + DEX | ✅ | Armor type selector (light/medium/heavy/unarmored/mage/draconic/natural) auto-syncs `sheet.ac` via `computeAC()`. Shield toggle, ac_base, max_dex all auto-apply. |
| 15 | Unarmored defense | ⚠️ | Backend parses `"10+dex+con"` from effects. **Not auto-applied** by selecting barbarian/monk class. |
| 16 | Mage armor | ⚠️ | Backend parses `"13+dex"` from effects. No mage armor toggle. |
| 17 | Max HP from hit dice | ✅ | `computedMaxHP()` syncs upward on class-level change. Per-class HD + CON + Hill Dwarf + Tough feat automated. |
| 18 | Current HP / temp HP | ✅ | Tracked and synced to combatants. |
| 19 | Hit dice pool | ✅ | `hit_dice.current/max/die` + per-class pools via `hit_dice.pools[]`. Multiclass with correct per-class die types. `hitDieFor()` returns official die per class (Barbarian d12, Fighter/Paladin/Ranger d10, Sorcerer/Wizard d6, etc.). |
| 20 | Spellcasting ability per class | ✅ | `detectSpellcastingAbility()` auto-detects from classes: INT (Wizard/Artificer), WIS (Cleric/Druid/Ranger), CHA (Bard/Paladin/Sorcerer/Warlock). Auto-detect button in magic tab. |
| 21 | Spell save DC auto-calc | ⚠️ | Backend auto (`8 + prof + mod`). Frontend manual field — not auto-filled. |
| 22 | Spell attack bonus auto-calc | ⚠️ | Backend auto (`prof + mod`). Frontend manual field — not auto-filled. |
| 23 | Spell slots tracking | ✅ | `sheet.slots` for levels 1–9. |
| 24 | Auto-seed spell slots | ✅ | `computeBaselineSlots()` uses PHB multiclass rules + warlock pact magic. |
| 25 | Prepared vs known spells | ✅ | `prepared` boolean on each spell. Enforced in `cast_spell` for Wizard/Cleric/Druid/Paladin/Artificer (non-masters). Known-spell classes skip. |
| 26 | Ritual casting | ✅ | `cast_as_ritual: true` + `spell.ritual = true` → slot not consumed. UI shows "Cast as Ritual" checkbox for ritual spells. |
| 27 | Concentration tracking | ✅ | `sheet.concentration` with spell name + timestamp. Backend checks on damage. |
| 28 | Class resources (ki, rage, etc.) | ✅ | Auto-seed from `templatesForClass`: Ki, Rage, Channel Divinity, Superiority Dice, Sorcery Points, Wild Shape, Bardic Inspiration, Lay on Hands, Second Wind, Action Surge, Indomitable, Mystic Arcanum, Infusions. |
| 29 | Short-rest resource regain | ✅ | Backend resets resources/features with `reset='short'` or `'long'`. Warlock pact slots refilled server-side. Frontend no longer required. |
| 30 | Long-rest resource regain | ✅ | Frontend resets all `reset !== 'none'`. Backend resets HP, hit dice, exhaustion, death saves, spell slots. |
| 31 | Darkvision range | ✅ | `sheet.senses.darkvision`. Backend also from effects. |
| 32 | Racial resistances | ⚠️ | Backend supports via effects. Frontend has no racial trait database. |
| 33 | Racial ability bonuses | ✅ | `racialAbilityBonus` covers 40+ subraces, auto-applied via `abilityScoreWithRacial()`. `RACIAL_DEFAULTS` covers 35+ entries with speed/darkvision/resistances/flags — auto-seeded on race change. Drow racial spells auto-seeded (Dancing Lights/Faerie Fire/Darkness). |
| 34 | Feat selector | ✅ | Full UI with prerequisites and config. |
| 35 | Feat mechanical effects | ⚠️ | `applyFeatEffects` handles: ability +1, init/speed/PP bonus, save/armor prof, resource creation (Lucky → Luck Points). **Many major feats empty:** Sharpshooter, GWM, Crossbow Expert, Sentinel, Polearm Master, War Caster — listed for reference, mechanics NOT enforced. |
| 36 | Equipment/inventory section | ✅ | `sheet.equipment` array with name, qty, weight, equipped flag, coin purse. |
| 37 | Equipped armor/weapons/shields | ⚠️ | Weapons have `equipped` toggle. **No structured armor type** — armor is generic equipment row with no AC automation. |
| 38 | Magic item bonuses applied | ⚠️ | `sheet.attunement` stores `bonuses` object. **NOT mechanically applied** — neither frontend display nor backend combat engine reads them. Reference only. |
| 39 | Attunement limit (max 3) | ✅ | Frontend enforces. Warns at limit. |
| 40 | Carrying capacity | ✅ | `STR × 15` computed. Total weight summed. |
| 41 | Alignment | ✅ | Create form has 9 PHB alignment options. Story tab displays alignment with i18n. Stored in `sheet.alignment`. |
| 42 | Background | ⚠️ | Free-form text areas (backstory, personality, ideals, bonds, flaws). No structured picker with mechanical effects. |
| 43 | Inspiration | ✅ | Binary toggle. |
| 44 | Passive Perception | ✅ | `10 + perception mod + bonus`. Backend computes for all skills. |
| 45 | Speed (race/armor/monk) | ✅ | `computedSpeed()` applies Barbarian Fast Movement (+10), Monk Unarmored Movement (+10–30), Mobile feat (+10), heavy armor STR penalty (−10). Auto-syncs on class change. Display with apply button. |
| 46 | Languages | ✅ | Free-form text field with auto-seed from race (Common + racial language). 35+ race entries with PHB languages. |
| 47 | Encumbrance penalties | ✅ | Variant encumbrance rules (PHB p.176): speed −10/−20 at STR×5/×10 thresholds. Displayed in equipment tab with i18n. |

---

## Rest Mechanics (3 items)

| # | Feature | Status | Detail |
|---|---------|--------|--------|
| 1 | Short rest | ✅ | Backend rolls hit dice, resets HP, resets `reset === 'short'/'long'` resources + features, refills warlock pact slots. No frontend patch needed. |
| 2 | Long rest | ✅ | Backend: full heal, hit dice to half max (rounded up), spell slots restored, exhaustion −1, death saves reset, resources/features reset for `reset !== 'none'`. |
| 3 | Short vs long rest resource tracking | ✅ | Resources have `reset` field (`'short'|'long'|'none'`). Backend `short_rest`/`long_rest` respect it. |

---

## Summary by Severity

All former 🔴 Critical Gaps are now closed. See ✅ Previously Critical below.

### ✅ Previously Critical — Now Fixed

| Gap | Fix |
|-----|-----|
| Fighting Styles | Auto from `sheet_raw.fighting_styles[]`: Archery +2, Dueling +2, GWF reroll, TWF no-mod. UI toggle in combat tab. |
| Sneak Attack / Smite / Rage | `extra_damage_expression` + `extra_damage_type` on AttackReq |
| Two-weapon fighting | `/two-weapon-fight` endpoint, TWF style auto-detected |
| Prone attacker disadvantage | `attack_disadvantage = true` when attacker prone (all attacks) |
| Ritual casting | `cast_as_ritual: true` skips slot consumption |
| Spell preparation | Enforced for prepared-list classes in `cast_spell` |
| Temp HP highest-wins | Enforced in `update_combatant` SQL |
| Massive damage | Single hit ≥ hp_max → instant death |
| Death save reset | Heal at 0 HP resets saves |
| Surprised enforcement | Blocks full turn at turn start |
| Regeneration | Auto HP restore at turn start from `hp_regen_per_turn` effects |
| Condition immunity | Enforced by creature type + NPC `condition_immunities` on apply |
| Condition durations | `name:N` format, ticked down at turn start |
| Grapple auto-release | On incapacitation/unconscious |
| Cantrip damage scaling | ×1/×2/×3/×4 at levels 1/5/11/17 |
| Spell attack roll | `use_spell_attack: true` path with crit/miss |
| Spell component checks | V blocked by `silenced`; S blocked by `no_somatic` |
| Spell range validation | Token distance vs `range_text` in feet |
| Hazard zone damage | Per-turn at turn start from `encounter_overlays` |
| Shield reaction gating | Requires pending hit (`last_hit_attack_total`) |
| Counterspell gating | Requires active `spell_being_cast` |
| Ready action auto-execute | `trigger_event`: `target_attacks` / `target_casts` / `target_enters_range` |
| **Rage** | Real `combatant_effect` applied: BPS resistance, `damage_bonus` scales by Barbarian level (+2/+3/+4), `attack_advantage`. |
| **Fast Movement** | Barbarian 5+ → +10ft in `compute_stats`, skipped in heavy armor. |
| **Unarmored Movement** | Monk 2+ → +10–30ft based on level in `compute_stats`, only when unarmored and no shield. |
| **Reliable Talent** | Rogue 11+: `resolve_skill_check` floors d20 to 10 for prof/expert skills. |
| **Lay on Hands** | `lay_on_hands` class_feature: reads `sheet.resources` pool, heals target, decrements pool. |
| **Reckless Attack** | `reckless: true` on attack → attacker advantage + counter-effect (`{"attack_advantage_against": true}`) giving enemies advantage. Frontend checkbox in attack form. |
| **Cunning Action** | Rogue Dash/Disengage/Hide consume bonus action via `use_bonus_action: true` flag. Frontend auto-detects Rogue class, shows BA variants. |
| **Magic Item Bonuses** | `attunement[].bonuses` (ac/attack/damage/spell_dc/initiative/speed) now read by `compute_stats`. No longer reference-only. |
| **Opportunity Attack Reach** | `checkOpportunityAttacks` checks equipped weapons for `reach` property; 10ft reach = 2.5 cell OA range vs default 1.5. |
| **Cover Auto-Calc** | Cover auto-checked via `$effect` when attack target changes; result auto-populates cover selector. |
| **Flanking Auto-Apply** | Attack handler checks flanking geometry before each attack; auto-sets `adv = true` when attacker + ally flank target. |
| **Jack of All Trades** | `resolve_skill_check` adds `pb/2` for non-proficient skills when Bard 2+. |
| **Long-Range Disadvantage** | Ranged/thrown weapons parsed for normal/long range; target beyond normal → `dis = true`. |
| **Thrown Weapon Tracking** | Daggers/javelins/etc decremented from equipment.qty on throw, like ammunition. |
| **Auto Damage from Weapon Stats** | Frontend auto-fills attack/damage expressions from weapon stats + ability mod + fighting styles on weapon select. Backend auto-computes when expressions are None. |
| **NPC Multiattack Parsing** | `GET /combatants/{id}/parse-multiattack` parses "2 claws + 1 bite" / "makes two attacks: one with its bite..." into structured sub-attacks. Frontend "Parse" button in multiattack form auto-fills attack rows. |
| **Backend Overrides Sync** | `ability_mod()` checks `abilities_override` before base abilities. Save mods check `saves_override`. Matches frontend `abilityScore()`/`saveMod()` behavior. |
| **Alignment** | Added to create form (9 PHB options) + story tab display with i18n. Stored in `sheet.alignment`. |
| **Tool Proficiencies** | Structured `{name, ability, proficient, expert}` rows with auto-calculated bonus display (ability mod + prof + expertise). |
| **Per-class Spellcasting Ability** | `detectSpellcastingAbility()` auto-detects from classes (INT→Wiz/Art, WIS→Clr/Drd/Rgr, CHA→Brd/Pal/Sor/Wlk). Auto-detect button in magic tab. |
| **Jack of All Trades (Bard 2+)** | Fixed threshold to level 2 (was 3). |
| **Auto AC from Gear** | Armor type selector auto-syncs `sheet.ac` on type/base/max_dex/shield change via `computeAC()`. Shield toggle also auto-syncs. No more manual "apply" needed. |
| **Auto Max HP from Hit Dice** | `computedMaxHP()` auto-syncs upward on class-level change in the class `$effect`. Player levels up → HP auto-increases. |
| **Racial Traits Auto-Application** | Speed/darkvision/resistances/flags auto-seeded on race change via `raceSeedSigs` $effect. Racial ability bonuses auto-applied via `abilityScoreWithRacial()`. Racial spells auto-seeded (Tiefling + Drow). Race data covers 35+ subraces. |
| **Death Save 3-Success HP** | Fixed bug where stabilized character had 0 HP instead of 1 (PHB p.197). |
| **Armor Stealth Disadvantage** | Added checkbox in combat tab to toggle `armor.stealth_disadvantage`. Badge shown when active. |
| **Spellcasting One-Click** | Auto-detect button now chains casting ability + spell attack + save DC in one click. Shows computed values inline. |
| **Speed from Class Features** | `computedSpeed()` adds Barbarian Fast Movement (+10 at 5+) and Monk Unarmored Movement (+10-30 at 2+) + Mobile feat. Auto-syncs in class-change `$effect`. Display with apply button. |
| **Encumbrance Speed Penalty** | Variant encumbrance (PHB p.176): speed -10/-20 at STR×5/×10 thresholds. Displayed in equipment tab with i18n. |
| **Backend Resource Reset on Rest** | `short_rest`/`long_rest` now iterate `sheet.resources` and `sheet.features`, resetting current=max for matching reset type. No longer relies on frontend-only PATCH. |
| **Default Sheet on Create** | New characters get default abilities (10 each), HP (1/1), AC 10, hit_dice (1/d8), death_saves {0,0}, alive, inspiration, exhaustion 0. Frontend no longer required to send full sheet. |
| **Non-magical DR Display** | `nonmagical_damage_reduction` (from Heavy Armor Master) now displayed in combat tab. |
| **Artificer Caster Type** | Fixed: Artificer now correctly classified as half-caster (was wrongly full-caster). Affects slot progression, max spell level, and multiclass slot calculation. |
| **featPrereqsMet Caster Check** | Fixed: Artificer added to caster class list. Changed `.includes()` substring match to exact + first-word match for robust detection. |
| **Death Save 3-Success Stable** | Changed to PHB p.197: stable at 0 HP (unconscious), not conscious with 1 HP. |
| **Dual Wielder AC Dynamic** | +1 AC now applies only when wielding 2+ melee weapons (checked in `computedAC`), not unconditionally via `ac_bonus`. |
| **Feat Removal Edge Cases** | `save_prof` removal no longer deletes key — prevents stripping same prof from other sources. |
| **Language Auto-Seed** | All races auto-seed their PHB languages (Common + racial) on create/race change. 35+ entries with languages. |
| **Swim/Fly/Climb Speed Auto-Seed** | Aarakocra fly 50, Tortle/Triton/Lizardfolk/Water Genasi swim, Tabaxi climb 20. Added swim/fly/climb to RACIAL_DEFAULTS. |
| **Backend Warlock Pact Slots** | `short_rest` now refills warlock pact slots server-side (was frontend-only). |
| **Heavy Armor STR Requirement** | `computedSpeed` reduces speed -10 when wearing heavy armor with STR < 15 (PHB p.144). |
| **Inspiration Mechanic** | 'Use' button rolls 2d20kh1 (advantage) and consumes inspiration. Toggle on/off by player, reset by GM. |
| **Subclass Feature Seeding** | Selecting a subclass from SubclassAutocomplete auto-seeds its features from subclasses.ts as reference entries. |
| **Multiclass Hit Dice Pools** | Per-class HD pools auto-populated from classes. Backend short/long rest handles pools format. Fallback to legacy single-die for old sheets. |
| **Dual Wielder AC Dynamic** | +1 AC now applies only when wielding 2+ melee weapons (checked in `computedAC`), not unconditionally. |
| **Death Save Stable** | 3 successes = stable at 0 HP (unconscious, PHB p.197). |
| **SRD Equipment Catalog** | `items.ts` with 60+ SRD items (armor, weapons, shields, gear). Import + addFromCatalog function ready. |
| **Hit Die Defaults by Class** | `hitDieFor()` in `dnd/classes.ts` returns correct PHB die per class: Barbarian d12, Fighter/Paladin/Ranger d10, Sorcerer/Wizard d6, others d8. Custom classes default to d8 (user-changeable). Stored `hit_die` field overrides lookup. Applied in HP pools, max HP computation, and class dropdown default. |

All former 🟡 High Gaps are now closed. See ✅ Previously Critical below.

### 🟢 Medium Gaps (nice-to-have)

| Gap | Impact |
|-----|--------|
| Background structured picker | Free-form only |
| Flying speed 0 → fall damage | Paralyzed/stunned fliers not grounded |
| Mounted combat | No rider/mount relationship |

All former 🟢 Medium Gaps closed: Hide contested roll ✅ (2026-04), Surprise auto-check ✅ (2026-04).

---

## Class Implementation Status (as of 2026-05-04)

| Class | Resources | Spell Slots | Mechanical Features | Subclass Mechanics |
|-------|-----------|-------------|--------------------|--------------------|
| **Artificer** | ✅ Infusions Known, Infused Items | ✅ Half caster (fixed: was full-caster) | ✅ INT-based preparation, ✅ spellPrepCount, ✅ detectSpellcastingAbility | ❌ All subclasses reference only |
| **Barbarian** | ✅ Rages (correct max by level) | — | ✅ Rage (BPS resist + dmg bonus + adv), ✅ Fast Movement (5+), ✅ Unarmored Defense armor types, ✅ Reckless Attack (adv on attack, enemies have adv vs you) | Champion Crit: ❌ (Fighter only). Berserker Frenzy: ❌ |
| **Bard** | ✅ Bardic Inspiration (manual max) | ✅ Full caster | ✅ Die scaling display (d6→d12), ✅ Jack of All Trades (2+, pb/2 to non-proficient skills in resolve_skill_check + frontend `hasJackOfAllTrades` threshold fixed) | ❌ All subclasses reference only |
| **Cleric** | ✅ Channel Divinity, Divine Intervention | ✅ Full caster | ✅ Aura of Protection displayed | ❌ All domains reference only |
| **Druid** | ✅ Wild Shape, Natural Recovery | ✅ Full caster | Wild Shape: resource tracked, no beast stats | ❌ All circles reference only |
| **Fighter** | ✅ Second Wind ✅ Action Surge ✅ Indomitable | — | ✅ Second Wind (rolls 1d10+level), ✅ Action Surge (resets action), ✅ Fighting Styles, ✅ Champion crit 19–20 | Battle Master maneuvers: ❌ |
| **Monk** | ✅ Ki | — | ✅ Evasion (7+), ✅ Unarmored Movement (2+, +10–30ft), ✅ Unarmored Defense (AC=10+DEX+WIS) | ❌ All ways reference only |
| **Paladin** | ✅ Channel Divinity, Lay on Hands, Cleansing Touch | ✅ Half caster | ✅ Lay on Hands (pool heal), ✅ Aura of Protection displayed, ✅ Draconic AC (wrong class but shares) | ❌ All oaths reference only |
| **Ranger** | — | ✅ Half caster | Fighting Styles ✅ | ❌ All archetypes reference only |
| **Rogue** | — | — | ✅ Evasion (7+), ✅ Reliable Talent (11+, floor-10), ✅ Cunning Action (BA Dash/Disengage/Hide), Sneak Attack: manual via extra_damage | ❌ All archetypes reference only |
| **Sorcerer** | ✅ Sorcery Points | ✅ Full caster | ✅ Draconic AC (13+DEX auto-set) | Metamagic: ❌ |
| **Warlock** | ✅ Invocations, Mystic Arcanum | ✅ Pact magic | — | Invocation effects: ❌ |
| **Wizard** | ✅ Arcane Recovery | ✅ Full caster | — | All schools: ❌ |

**Key:** ✅ mechanical | ⚠️ partial | ❌ reference only

### Class features still missing (high priority)
- **Sneak Attack**: use `extra_damage_expression` in attack form (e.g. `3d6` radiant) — already works, just not automatic
- **Divine Smite**: same — `extra_damage_expression: Xd8` on hit
- **Battle Master Maneuvers**: pool tracked, zero effects
- **Metamagic**: Sorcery Points stored, options have no engine
- **Eldritch Invocations**: pool tracked, effects inert
- **Stunning Strike**: Ki cost not automated, no CON save trigger
- **Wild Shape**: forms have no stat block database

---

## Architecture Note

The app has transitioned from **"manual entry with computed display"** to **"auto-computed with manual overrides"** for most core stats.

- Backend combat engine (`combat_engine.rs`) is solid for resolving attacks/saves/damage **when given expressions**.
- AC, max HP, spellcasting ability all auto-compute and auto-sync on relevant changes (armor, class levels).
- Frontend overrides exist for cases where auto-computed values disagree.

**Previously critical gaps now closed:**
1. ✅ Auto AC from equipped gear (armor type selector auto-syncs)
2. ✅ Auto max HP from class hit dice (auto-syncs upward on level-up)
3. ✅ Per-class spellcasting ability (detectSpellcastingAbility)
4. ✅ Racial trait auto-application (speed/darkvision/resistances/flags/spells)
5. ✅ Alignment tracking (create form + story tab)
6. ✅ Structured tool proficiencies (with auto-bonus display)

**Minor (low priority):**
1. Equipment catalog picker HTML could be added to loot tab (SRD items.ts exists, import + addFromCatalog done, just needs HTML)
2. Hit dice pools UI display could be added to vitals tab (backend + script changes done)
3. Fill remaining empty feat effect handlers (14 feats reference-only — require combat engine changes)
4. Additional subclass mechanics (all subclasses reference-only — intentonal scope boundary)

---

*End of audit. Use for feature planning priority.*
