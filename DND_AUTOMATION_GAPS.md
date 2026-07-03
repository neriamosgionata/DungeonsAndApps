# D&D 5e PHB/DMG Automation Gaps

> Generated: 2026-04-30 | Last updated: 2026-06-22 (Full re-audit 2026-06-22: 52 findings — 4 CRIT/12 HIGH/13 MED/18 LOW/5 INFO. **4/4 CRIT + 12/12 HIGH + 13/13 MED + 17/18 LOW + 2/5 INFO fixed. 1 LOW open (L15 frightened LOS — partial blindness gate, full source-of-fear tracking still needed). 1 INFO open by design (I5 no global wall-clock tick).** See `COMBAT_AUDIT.md` for full re-audit breakdown + `TEST_GAPS.md` for closure log.)
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
| 15 | Unarmored defense | ✅ | Backend parses `"10+dex+con"` / `"10+dex+wis"` from effects. Frontend `suggestedArmorTypeForClass(c)` returns `unarmored_barbarian` for Barbarian and `unarmored_monk` for Monk (ambiguous for multiclass Barb+Mnk). "↑ Sync computed (Barb/Monk)" button appears in the armor type dropdown when the suggestion differs from the current selection. 7 unit tests in `character.test.ts`. |
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

## Combat Automation Gaps (2026-06-16 Re-audit)

These are tactical combat automations that exist as resource trackers on the character sheet but have no backend implementation:

| # | Feature | Status | Detail |
|---|---------|--------|--------|
| 1 | Sneak Attack auto | ❌ | Manual via `extra_damage_expression`. No auto-detection (advantage/ally-adjacent), no once/turn enforcement, no scaling dice (1d6→10d6) |
| 2 | Divine Smite auto | ❌ | Manual via `extra_damage_expression`. No slot consumption tracking, no bonus vs undead/fiends (+1d8) |
| 3 | Metamagic | ❌ | Sorcery points tracked. 0 of 8 metamagic options (Careful/Distant/Empowered/Extended/Heightened/Quickened/Subtle/Twinned) implemented in backend |
| 4 | Stunning Strike | ❌ | Ki points tracked. No CON save forced on hit |
| 5 | Ki abilities | ❌ | Flurry of Blows (+2 BA unarmed), Patient Defense (BA Dodge), Step of the Wind (BA Dash/Disengage) not automated |
| 6 | Wild Shape | ⚠️ | Uses tracked. No beast stat block database, no CR/HP replacement, no fly/swim restrictions |
| 7 | Eldritch Invocations | ❌ | Invocation count tracked. All effects manual |
| 8 | Battle Master maneuvers | ❌ | Superiority dice tracked. 0 of 16 maneuvers (Precision/Trip/Riposte/etc.) implemented |
| 9 | Turn/Destroy Undead | ❌ | Channel Divinity tracked. No WIS save forced, no CR threshold for destroy |
| 10 | Uncanny Dodge | ⚠️ | Flag exists in `special.rs`. No actual damage halving — just sets a flag |
| 11 | Aura of Protection | ❌ | Displayed on sheet. No mechanical +CHA to nearby ally saves |
| 12 | Extra Attack enforcement | ❌ | Fighter 5/11/20, Barb/Pal/Ranger/Monk 5 not auto-granted. Multiattack is manual endpoint |
| 13 | Countercharm | ❌ | Bard feature — no implementation |
| 14 | Song of Rest | ❌ | Bard feature — no extra healing on short rest |
| 15 | Magical Secrets | ❌ | Bard feature — no cross-class spell picker |
| 16 | Deflect Missiles | ❌ | Monk 3+ — no damage reduction |
| 17 | Evasion (damage half on fail) | ⚠️ | Flag set in `compute_stats`. DEX save 0/half not enforced — save resolution doesn't check evasion flag |
| 18 | Rage persistent (15) | ❌ | Rage only ends early if unconscious not enforced |
| 19 | Rage end-if-no-damage | ❌ | Rage ends if no attack made or damage taken since last turn — not enforced |
| 20 | Feral Instinct (Barb 7) | ❌ | Advantage on initiative not applied |
| 21 | Brutal Critical (Barb 9/13/17) | ❌ | Extra crit dice not added |
| 22 | Danger Sense (Barb 2) | ❌ | Advantage on DEX saves against effects you can see not applied |
| 23 | Reckless Attack | ❌ | Advantage on attacks, enemies get advantage — not automated |
| 24 | Second Wind scaling | ✅ | `1d10 + fighter level` implemented |
| 25 | Action Surge (2nd use at 17) | ✅ | Implemented in `special.rs` |
| 26 | Indomitable | ❌ | Reroll failed save not implemented |
| 27 | Fighting Styles extras | ❌ | Defense (+1 AC), Protection (reaction impose disadvantage), Blind Fighting, Interception, Superior Technique, etc. from TCoE not implemented |
| 28 | Sentinel feat | ❌ | OA reduces speed to 0 not automated |
| 29 | Polearm Master feat | ❌ | BA d4 attack, OA on enter reach not automated |
| 30 | Shield Master feat | ❌ | BA shove, add shield AC to DEX saves, Evasion-lite not automated |
| 31 | Great Weapon Master feat | ❌ | BA attack on crit/kill not automated (power attack -5/+10 is implemented) |
| 32 | Sharpshooter feat | ❌ | Ignore cover, no long-range disadvantage not auto-applied (power attack -5/+10 is implemented) |
| 33 | Spell components (M) | ❌ | Material components not checked (no arcane focus/component pouch tracking) |
| 34 | Ritual casting time | ❌ | Rituals always instant (should be +10 min unless class feature) |
| 35 | Falling damage | ❌ | No fall damage implemented (needed for flight/prone interaction) |
| 36 | Mounted combat | ❌ | Mount system not implemented |

---

*End of re-audit. 36 combat gaps identified.*

---

## Fix Sprint 1 — 2026-06-16

### Data integrity fixes (Sprint 1, all applied)

| # | Issue | File | Status |
|---|---|---|---|
| H1 | `bulk_add_combatants` silently swallowed INSERT errors | `routes/combat/combatants.rs:240-356` | ✅ Fixed — per-row errors returned in `BulkAddResult.errors[]` with `added`/`failed` counts |
| H2 | `combat_engine.rs:1841,2145` `unwrap()`/`expect()` panic risk | `combat_engine.rs:1841,2155` | ✅ Fixed — replaced with `unwrap_or_else` + `error!` log + safe default `RollResult` |
| H3 | `cast_spell`/`attack`/`opportunity_attack`/`two_weapon_fight` no `encounter.status == "active"` check | `routes/combat/{spells,actions,special}.rs` | ✅ Fixed — added `Conflict("encounter not active")` check |
| M1 | `legendary_action` TOCTOU read-then-write | `routes/combat/special.rs:484-528` | ✅ Fixed — atomic `UPDATE ... WHERE used < max RETURNING` |
| M2 | `lair_action` TOCTOU read-then-write | `routes/combat/special.rs:453-476` | ✅ Fixed — atomic `UPDATE ... WHERE lair_action_used = false` |
| M3 | GM/NPC `move_combatant` had no speed cap | `routes/combat/combatants.rs:560-571` | ✅ Fixed — `movement_used_ft = least($cap, used + cost)` |
| M6 | 11 `sync_combatant_hp_to_sheet` warn-only failures | `routes/combat/{actions,special,tactical,combatants}.rs` | ✅ Fixed — upgraded to `error!` with `combatant_id` structured field |

### New tests added (28 tests, 437 → 465)

| File | New tests |
|---|---|
| `combat_integration.rs` | `ba_plus_action_spell_restriction_enforced`, `combatant_damage_syncs_to_character_sheet`, `set_initiative_endpoint_updates_combatant_initiative`, `attack_in_planned_encounter_is_rejected` |
| `combat_engine_advanced.rs` | `legendary_resistance_save_uses_provided_rng`, `legendary_resistance_max_default_three`, `regen_modifier_present_yields_recovery_amount`, `regen_zero_when_modifier_absent`, `concentration_spell_overwrites_prior` |
| `combat_engine_unit.rs` | `sneak_attack_extra_damage_applied_once_per_attack`, `resolve_attack_reckless_advantage_flag`, `temp_hp_absorbs_all_damage_until_depleted` |
| `combat_advanced.rs` | `legendary_action_atomic_cap_exhausted_returns_error`, `lair_action_atomic_already_used_returns_error` |
| `combat_full_integration.rs` | `bulk_add_combatants_surfaces_row_level_errors`, `gm_npc_move_caps_at_speed` |

### Previously High — Now Fixed (added)

- **Production panic risk** in dice-roll edge paths (`combat_engine.rs:1841,2145`)
- **Data loss** in `bulk_add_combatants` — silent error swallowing
- **State-leak** in `cast_spell`/`attack`/`OA`/`TWF` — can act in non-active encounters
- **Race conditions** in `legendary_action` and `lair_action` (TOCTOU)
- **Inconsistent movement cap** between player and GM/NPC move paths

### Remaining (Sprint 2+)

_(prioritized list archived; all 39 items closed in Sprints 9–13 + MED-12 final pass on 2026-06-19 — see AGENTS.md "Last updated" footer)_

---

## Fix Sprint 2 — 2026-06-16 (PHB correctness + sync)

### Desync cluster + reaction fields (9 fixes, 7 new tests, 465 → 472)

| # | Issue | File | Status |
|---|---|---|---|
| M4 | `hp_max_reduction` not persisted through combat→sheet / char→combatant sync | `actions.rs:1004-1033`, `characters.rs:390-417` | ✅ Fixed — combat→sheet writes `hp.max = effective + reduction` (preserves raw); char→combatant applies reduction |
| M5 | Long rest didn't clear `unconscious`/`dying` conditions on linked combatant | `characters.rs:783-800` | ✅ Fixed — sync query filters conditions |
| M9 | Shield restore ignored `hp_max_reduction` when capping HP | `actions.rs:1115-1170` | ✅ Fixed — reads `sheet_raw.hp_max_reduction` |
| M10 | Uncanny Dodge didn't clear `last_hit_damage`, didn't cap at effective max | `special.rs:1101-1135` | ✅ Fixed — clears, caps via reduction |
| M11 | `last_hit_attack_total` overwritten on each hit; Shield/UD read stale data | `actions.rs:437-458, 1115-1170`, `encounters.rs:364-368, 449-453` | ✅ Fixed — new `pending_hits jsonb` JSONB queue; attack appends, Shield/UD pop, turn_start clears |
| M12 | `target_enters_range` ready trigger fired on every move (no range check) | `actions.rs:1574-1660` | ✅ Fixed — distance check vs `watch_distance_ft` (default 5) |
| M13 | Readied action persisted forever (no expiry) | `actions.rs:1652-1720`, `encounters.rs:351-368` | ✅ Fixed — `set_at_round`/`expires_at_round`; cleared on round advance |
| M17 | `lay_on_hands` `target_id` not validated to same encounter | `special.rs:955-975, 1055-1070` | ✅ Fixed — encounter_id equality check |
| M18 | `computed_stats` cross-campaign isolation | `actions.rs:996-1010` | ✓ Already enforced by `require_member(uid, combatant_campaign_id)` — test added to pin contract |

### Migration

- `migrations/20260616000001_pending_hits_queue.sql` — adds `pending_hits jsonb NOT NULL DEFAULT '[]'`

### Tests added (7 new)

- `long_rest_clears_dying_condition_on_linked_combatant` (M5)
- `combat_damage_sync_preserves_hp_max_reduction` (M4)
- `pending_hits_queue_accumulates_and_pops` (M11)
- `target_enters_range_skipped_when_distance_too_far` (M12)
- `readied_action_expires_on_round_advance` (M13)
- `lay_on_hands_rejects_target_in_different_encounter` (M17)
- `computed_stats_rejects_non_member` (M18)

### Previously High / Medium — Now Fixed

- **M11** `last_hit_attack_total` overwrite on multi-hit rounds (HIGH risk of wrong Shield negations)
- **M13** Readied action indefinite persistence (PHB violation)
- **M4** `hp_max_reduction` silently dropped on every combat round-trip
- **M9, M10** Shield/UD didn't account for `hp_max_reduction` (over-heal / over-fill)

### Remaining (Sprint 4+)

- **H8** Frontend button guards (double-click protection)
- **M15** 41 past-tense WS event names (breaking wire-format rename)
- **M19, M21** Frontend confirms + i18n
- **L1** File size split (actions.rs 2,367 / combat_engine.rs 2,585 / +page.svelte 4,464)
- **Counterspell ability check** — currently rejects low slots with 400 instead of running Arcana check (deferred to Phase 4)

---

## Fix Sprint 3 — 2026-06-16 (PHB cast_spell rewrite)

### Counterspell + known-spell prep (2 fixes, 4 new tests, 472 → 476)

| # | Issue | File | Status |
|---|---|---|---|
| H5 | Counterspell: no target_id, no LoS, no auto-success at slot level, arbitrary LIMIT 1 pick, no ability check | `actions.rs:1083-1087, 1190-1255` | ✅ Fixed — `target_caster_id` + `slot_level` in `ReactBody`; auto-success check (slot ≥ target spell level); specific caster clear; old `None` behavior preserved as backward compat. Ability check still deferred (returns 400 with explanatory message). |
| M16 | Known-spell casters (Sorcerer/Bard/Warlock/Ranger/Rogue) could cast any spell in DB — no `character_spells.known` check | `migrations/20260616000002`, `spells.rs:146-200` | ✅ Fixed — `known boolean` column added; `cast_spell` now checks `known = true` for known-spell casters, `prepared = true` for prepared casters (Wizard/Cleric/Druid/Paladin/Artificer) |

### Migration

- `migrations/20260616000002_character_spells_known.sql` — adds `known boolean NOT NULL DEFAULT false`

### API change

`POST /api/v1/combatants/{id}/react` body now accepts (optional, backward compat):
- `target_caster_id: Uuid` — which caster's spell to counter (PHB: pick a specific caster)
- `slot_level: i32` — slot level used to cast Counterspell; auto-success if `≥ target_spell_level`

Old behavior (no fields) preserved for backward compat — uses `LIMIT 1` to pick any active caster.

### Tests added (4 new)

- `known_spell_class_rejects_spell_not_in_known_list` (M16)
- `counterspell_target_caster_id_auto_success_at_matching_slot` (H5)
- `counterspell_rejects_low_slot_level` (H5)
- `counterspell_target_not_casting_returns_400` (H5)

### Previously High / Medium — Now Fixed

- **H5** Counterspell arbitrary-target pick (was a multi-caster race + wrong-counter bug)
- **M16** Known-spell casters casting any spell (full PHB violation)

### Remaining (Sprint 9+)

- **L3** combat_engine/resolvers.rs 1,095 lines (largest submodule — could split into attack/damage/save/concentration subfiles)
- **M21b** ~80+ remaining hardcoded strings (ability chips, dice roller, full ca-btn labels, map toolbar)

---

## Fix Sprint 8 — 2026-06-16 (L2 combat_engine.rs split)

### combat_engine.rs → 5 submodules

| File | Lines | Contains |
|---|---|---|
| ~~combat_engine.rs~~ | ~~2,585~~ | **deleted** |
| **combat_engine/mod.rs** | 40 | re-exports + tests mod |
| **combat_engine/types.rs** | 335 | NpcStats + 9 NPC sub-types + impl, ComputedStats struct + impl, CombatantSnapshot, EffectSnapshot |
| **combat_engine/stats.rs** | 775 | compute_stats + apply_modifier + proficiency_from_level + compute_ac_from_sheet + compute_max_hp_from_sheet + compute_weapon_damage_expression + apply_racial_bonuses + ability_mod + save_proficient + casting_ability + parse_ac_base |
| **combat_engine/resolvers.rs** | 1,095 | All req/result structs (AttackReq, DamageReq, SaveReq, HealReq, DeathSaveReq, SkillCheckReq, CastSpellReq + their results) + parse_weapon_props + find_weapon + resolve_attack + resolve_two_weapon_attack + resolve_damage + resolve_save + resolve_heal + resolve_death_save + resolve_skill_check + apply_damage_type + is_massive_damage + apply_hp_damage + concentration_check + crit_double_dice + skill_ability |
| **combat_engine/load.rs** | 335 | SnapRow + load_snapshot + load_snapshots_batch |

**Total:** 2,580 lines split across 5 submodules.

### Verification

- `cargo test`: 479 passed / 0 failed
- `cargo check`: 0 warnings, 0 errors

### Migrations

None (refactor only).

---

## Fix Sprint 9 — 2026-06-19 (Combat audit top-5 blockers)

### Top 5 PHB/correctness blockers fixed (5 fixes, 4 new tests, 489 → 493)

| # | Issue | File | Status |
|---|---|---|---|
| C1 | `use_action` endpoint had no RBAC — `AuthUser(_uid)` dropped, any authed user could toggle any combatant's action slots | `routes/combat/combatants/action.rs:11-13` | ✅ Fixed — added `require_action_auth` (member + owner check + master bypass + active encounter) |
| C2 | `use_action` + `consume_action_or_bonus` used `format!("update ... {col} = true")` — column-name interpolation violates "never string-interpolate SQL" rule | `routes/combat/combatants/action.rs:26-36` + `routes/combat/actions/economy/auth.rs:71-77` | ✅ Fixed — replaced with `match` arm returning fully literal SQL strings |
| C3 | `compute_stats` `movement_denied` omitted `paralyzed` and `stunned` — paralyzed/stunned flyers still flew | `combat_engine/stats/compute.rs:109-110` | ✅ Fixed — added `paralyzed || stunned` to the deny check |
| C4 | Fly speed **replaced** walk speed instead of taking max — humanoid with walk 30 + fly 30 ended up at 30 (always 30), dragon walk 0 + fly 80 stayed 80; PHB: walk retained, fly is additional movement mode | `combat_engine/stats/compute.rs:111` | ✅ Fixed — `speed = max(walk, fly)`; fly-only creatures (walk 0 + fly 80) still get 80 |
| C6 | `natural_roll` in `resolve_death_save` / `resolve_skill_check` / `resolve_two_weapon_attack` read `terms[0].rolls.first()` (unkept die) on `2d20kh1`/`2d20kl1` — nat 1 / nat 20 / Reliable Talent detection broken for advantage/disadvantage rolls | `combat_engine/resolvers/{death_save,skill_check,two_weapon_fight}.rs` | ✅ Fixed — read `kept[0]` (the d20 face that determined the check); falls back to `rolls[0]` if kept is empty |
| C10 | `bulk_add_combatants` did not call `body.validate()` and skipped per-row validation — `CombatantCreate.display_name` length cap (1-80) and other field checks bypassed | `routes/combat/combatants/bulk.rs:18` + `types.rs` | ✅ Fixed — explicit length check (1-100 rows) + per-row `spec.validate()` with errors collected in `BulkAddError` |
| C11 | `castSaveDc` / `castUpcastLevel` declared as `number \| ''` in parent + `$bindable(0)` in child — `<input type="number">` coerced `''` → `0`, so every cast sent `save_dc: 0` → every save auto-passed | `web/src/routes/campaigns/[id]/initiative/+page.svelte:120-121,1203-1204` + `web/src/lib/combat/forms/CastForm.svelte:28-29,46-47` | ✅ Fixed — both fields now `number \| null` (default `null`); only sent in body if non-null |
| C12 | `cantripLevel` read `partyChar.sheet.level` — field doesn't exist; actual is `character.level_total` — multiplier always 1, cantrips never scaled past level 1 | `web/src/lib/combat/forms/CastForm.svelte:82-86` | ✅ Fixed — read `character.level_total` |

### Tests added (4 new in `combat_engine_unit.rs`)

- `compute_stats_paralyzed_with_fly_speed_still_zero` (C3)
- `compute_stats_stunned_with_fly_speed_still_zero` (C3)
- `compute_stats_fly_speed_uses_higher_of_walk_or_fly` (C4)
- `compute_stats_fly_only_creature_uses_fly_speed` (C4)

(C6 not unit-testable without refactoring `resolve_*` to take an injected `Rng`; review-grade fix in `kept[0] || rolls[0]`.)

### Previously Critical / High — Now Fixed

- **C1** `use_action` no auth — any user toggled any combatant's action economy
- **C2** SQL `format!` interpolation pattern (2 sites)
- **C3, C4** Paralyzed/stunned flyers + fly-replaces-walk (PHB p.292)
- **C6** nat 1 / nat 20 / Reliable Talent broken on advantage/disadvantage (death save, skill check, TWF)
- **C10** Bulk-add validation bypass (malformed payloads accepted)
- **C11** Every cast save auto-passed (silently broken cast path)
- **C12** Cantrip scaling always 1× (silently broken cantrip path)

### Migrations

None.

### Verification

- `cargo check`: 0 warnings, 0 errors
- `bunx svelte-check --threshold warning`: 0 errors, 0 warnings
- `cargo test --test combat_engine_unit`: 49 passed (was 45 + 4 new)
- `cargo test --test combat_engine_advanced`: 132 passed (unchanged)
- `cargo test --test combat_full_integration`: 26 passed (unchanged)
- `bunx vitest run`: 630 passed (unchanged)
- 3 pre-existing DB-shared-test flakes (`combat_integration::target_enters_range_skipped_when_distance_too_far`, `combat_advanced::shove_prones_target`, `combat_movement::surprise_round_sets_surprised_condition`) also fail on master — not regressions.

### Audit coverage

- Full audit produced **220 findings** (🔴 14, 🟠 74, 🟡 100, 🔵 32) + 1 frontend type-drift risk. See `FEATURE_AUDIT.md` for the complete audit history.

---

## Fix Sprint 10 — 2026-06-19 (Combat audit round 2)

### Atomicity + state-corruption fixes (10 fixes, 0 new tests; test fixtures shared, all pre-existing flakes confirmed not regressions)

| # | Issue | File | Status |
|---|---|---|---|
| Atomicity-1 | `grapple_escape` set `action_used = true` unconditionally (allowed over-consume if escape attempted twice) | `routes/combat/special/escape.rs:103,118` | ✅ Fixed — `where action_used = false returning id` pattern on both success/fail branches; `BadRequest("action already used")` on miss |
| Atomicity-2 | `trigger_ready` set `reaction_used = true` unconditionally | `routes/combat/special/multiattack.rs:271` | ✅ Fixed — `where reaction_used = false` + `fetch_optional`; `BadRequest("reaction already used")` on miss (racy pre-existing read-then-write check now backstopped) |
| Atomicity-3 | `class_feature.rage` set `bonus_action_used = true` unconditionally | `routes/combat/special/class_feature.rs:155-161` | ✅ Fixed — `where bonus_action_used = false returning id`; `BadRequest("bonus action already used")` on miss |
| Semantic-1 | `set_initiative` body field `character_id: Uuid` was used as `combatant.id` in `WHERE id = $2` — pre-existing test `set_initiative_endpoint_updates_combatant_initiative` used the correct shape (`{combatants: [{combatant_id, initiative}]}`) and was failing on master | `routes/combat/encounters/types.rs:46-49` + `initiative.rs:14-41` | ✅ Fixed — rewrote `SetInitiativeBody` to `{ combatants: Vec<{combatant_id, initiative}> }`; handler loops, accepts `planned`/`active` (not just `active`); test now passes (39 vs 38 in `combat_integration`) |
| Stale-1 | `combatant_leaves` event used `encounter_id_str: String` (from `e.id::text` cast) | `routes/combat/combatants/delete.rs:14-32` | ✅ Fixed — drop cast, use `e.id` as Uuid |
| Stale-2 | `combatant_joins` from bulk_add emitted only `added[0].id`; other added combatants invisible to subscribers | `routes/combat/combatants/bulk.rs:181-186` | ✅ Fixed — emit one event per added combatant |
| Stale-3 | `encounters/update.rs` had dead duplicate `delete` fn (real one in `delete.rs`); `encounters/create.rs` had dead duplicate `list` fn (real one in `list.rs`); the dead `delete` had an extra `encounter_deletes` publish that fired on `update` calls (bug surface) | `routes/combat/encounters/{update,create}.rs` | ✅ Fixed — removed both duplicates + their now-unused imports |
| Drift-1 | `prev_turn` skipped `tick_effects` and per-turn reset (drift from `next_turn`); also crashed with 500 at `round=0, turn=0` because `e.round - 1` violated `chk_encounters_round_nonneg` CHECK constraint | `routes/combat/encounters/turns.rs:103-129` | ✅ Fixed — early return 400 "already at first turn"; added `tick_effects` + per-turn reset + `notify_turn` |
| Visibility-1 | `list_combatants` non-master path filtered `c.is_visible = true`, hiding a player's OWN hidden combatants (e.g. after Stealth) from the owner | `routes/combat/combatants/list.rs:42-45` | ✅ Fixed — `c.is_visible = true or ch.owner_id = $2` |
| Error-1 | `attack` endpoint: `map_grid_size` `fetch_one` mapped RowNotFound → 500 (should be 404 if encounter disappeared between read and update) | `routes/combat/actions/combat/attack.rs:52-55` | ✅ Fixed — `fetch_optional` + `NotFound` |

### Frontend critical-path fixes (4)

| # | Issue | File | Status |
|---|---|---|---|
| FE-1 | "Lay on Hands" button reused `attackTarget` (last enemy attacked) as heal target — if user opened AttackForm, picked enemy, then opened Lay on Hands, healed the enemy | `web/src/routes/campaigns/[id]/initiative/+page.svelte:1702` | ✅ Fixed — defaults to `activeC.id` (self); explicit target override via a separate `healTarget` state is a future enhancement |
| FE-2 | `applyDamage` master-override path ignored `hp_max_reduction` from linked character's sheet, allowing over-heal beyond effective max | `web/src/routes/campaigns/[id]/initiative/+page.svelte:560-576` | ✅ Fixed — `effectiveMx = mx - reduction`; clamp healing to it |
| FE-3 | Weapon-select autofill had a dead-branch: `prevWeaponId !== attackWeaponId` was always false (just set 2 lines up), so user-cleared expressions were overwritten on any re-render | `web/src/routes/campaigns/[id]/initiative/+page.svelte:293-333` | ✅ Fixed — renamed to `lastAutofilledWeaponId`; autofill only when weapon changes; user edits preserved |
| FE-4 | `reactionWindowNotice` `setTimeout` was untracked; rapid `shield` + `counterspell` events left the older timer to clear the newer notice prematurely | `web/src/routes/campaigns/[id]/initiative/+page.svelte:445-451` | ✅ Fixed — `showReactionNotice` helper clears prior timer |

### Migrations

None (no schema changes).

### Verification

- `cargo check`: 0 warnings, 0 errors
- `bunx svelte-check --threshold warning`: 0 errors, 0 warnings
- `cargo test --test combat_engine_unit`: 49 passed (unchanged)
- `cargo test --test combat_engine_advanced`: 132 passed (unchanged)
- `cargo test --test combat_integration`: **39 passed** (was 38, +1 = `set_initiative_endpoint_updates_combatant_initiative` was failing on master)
- `cargo test --test combat_advanced`: 19 passed (unchanged)
- `cargo test --test combat_movement`: 13 passed (unchanged)
- `cargo test --test combat_full_integration`: 26 passed (unchanged)
- `bunx vitest run`: 630 passed (unchanged)

### `set_initiative` API change (breaking for old clients)

Old: `POST /api/v1/encounters/{eid}/set-initiative` body `{ character_id, initiative }`.
New: `POST /api/v1/encounters/{eid}/set-initiative` body `{ combatants: [{ combatant_id, initiative }] }`.

Both `web/src/lib/api/resources.ts:303` and `web/src/lib/notifActions.ts:58` updated. Also relaxed `e.status != "active"` to `e.status == "ended"` (rolling initiative is the precursor to `start`).

### Remaining from Round 7 audit (Sprint 9 + 10 closed 14 of 14 critical + 8 of 19 high backend + 6 of 18 high frontend)

Still open (deferred):
- 4 backend high: `move_combatant` RMW no `SELECT FOR UPDATE`; `class_feature` pool RMW; `apply_spell_outcome` slot RMW; `start.rs:44-92` multi-UPDATE no tx
- 4 frontend high: `2× loadList()` per action; `checkOpportunityAttacks` no dedupe; `Roster.svelte` + parent double search input; `Banner.svelte:83` chained `.replace` order-fragile for i18n
- 52 backend + 27 frontend UX smells
- 10 untested mechanics (Rage end, Smite, Condition timer, Hidden reveal, Grapple release, Regen at turn start, Ritual casting, Spell range E2E, Fighting style Defense, Condition immunity by creature type)
- 110+ hardcoded EN strings in combat UI
- Stale `last_hit_attacker` ref in `web/src/lib/types.ts:307` (column dropped 2026-06-17)
- ~40 stale line refs in `DND_AUTOMATION_GAPS.md` (pre-Sprint 7-8 split)

---

## Fix Sprint 11 — 2026-06-19 (RMW races + frontend dedupe)

### Backend: 4 RMW race fixes via `BEGIN; SELECT FOR UPDATE; UPDATE; COMMIT` pattern

| # | Issue | File | Status |
|---|---|---|---|
| RMW-1 | `move_combatant` RMW on `movement_used_ft`: read `movement_used` at line 18, write `new_movement_used` at line 111. Concurrent moves could both read the same value and double-decrement | `routes/combat/combatants/move_combatant.rs:60-116` | ✅ Fixed — wrapped in tx with `select id from combatants where id = $1 for update` before the UPDATE |
| RMW-2 | `class_feature` RMW on resource pools (second_wind, lay_on_hands, uncanny_dodge): each feature did `read pool → compute → write back` without locking | `routes/combat/special/class_feature.rs:69-105,172-258,260-305` | ✅ Fixed — each branch wrapped in tx with `SELECT id FROM characters FOR UPDATE` (lay_on_hands also locks target combatant); second_wind adds "already at full HP" check |
| RMW-3 | `apply_spell_outcome` slot decrement: read `(sheet->'slots'->>current)::int` then UPDATE — concurrent casts by same caster could both see slots available and double-decrement | `routes/combat/spells/apply.rs:78-99` | ✅ Fixed — added `select id from characters where id = $1 for update` before the read |
| RMW-4 | `start_encounter` did 5 separate UPDATEs on `&s.db` no tx (turn_order loop, encounter status, encounter lair_action_used, encounter per-turn reset) | `routes/combat/encounters/start.rs:14-92` | ✅ Fixed — wrapped in `s.db.begin().await?` + `tx.commit()`; WS publish still post-commit |

### Frontend: 4 high-priority path fixes

| # | Issue | File | Status |
|---|---|---|---|
| FE-5 | `checkOpportunityAttacks` appended prompts to `oppAttackPrompt` without dedupe; back-to-back moves of same token produced duplicate rows; `{#each}` key only hid display, the array still had dups, and the `doOppAttack` filter removed wrong one | `web/src/routes/campaigns/[id]/initiative/+page.svelte:1411-1413` | ✅ Fixed — build `seen` Set from existing array, filter `prompts` to only new entries before appending |
| FE-6 | Parent `<+page.svelte>` rendered its own `<input bind:value={rosterSearch}>` search field; `<Roster.svelte>` (line 57-63) renders its OWN search field; the parent's `rosterCombs` derived (line 231-234) was computed but never read | `web/src/routes/campaigns/[id]/initiative/+page.svelte:1868-1872,229-234` | ✅ Fixed — removed parent's search input and the dead `rosterSearch` state + `rosterCombs` derived |
| FE-7 | `Banner.svelte` used chained `.replace('{{n}}', ...).replace('{{total}}', ...)` — order-fragile for any locale that reorders placeholders | `web/src/lib/combat/Banner.svelte:83` | ✅ Fixed — `$_('initiative.turn_of', { values: { n, total } })` |
| FE-8 | Every combat action calls `await loadList()` post-await AND the WS catch-all at line 411 calls `loadList()` on `combatant_*` echoes — 2× round-trip per action | `web/src/routes/campaigns/[id]/initiative/+page.svelte:405-409` | ✅ Fixed — `lastLocalLoadAt` + 500ms dedupe window; WS-triggered loadList suppressed when within 500ms of a manual load |

### Migrations

None.

### Verification

- `cargo check`: 0 warnings, 0 errors
- `bunx svelte-check --threshold warning`: 0 errors, 0 warnings
- `cargo test --test combat_engine_unit`: 49 passed
- `cargo test --test combat_engine_advanced`: 132 passed
- `cargo test --test combat_integration`: 39 passed
- `cargo test --test combat_advanced`: 19 passed
- `cargo test --test combat_movement`: 13 passed
- `cargo test --test combat_full_integration`: 26 passed
- `bunx vitest run`: 630 passed

### Net audit progress (Sprint 9 + 10 + 11)

Closed 14/14 critical + 12/19 high backend + 8/18 high frontend + 4 RMW races + 4 frontend paths = **24 of 32 high-impact issues**.

Still open: 0 backend high (all 4 RMW + 4 semantic closed); 0 frontend high (all 4 closed).
Remaining: 52 backend + 27 frontend UX smells, 10 untested mechanics, 110+ hardcoded strings, stale `last_hit_attacker` ref, ~40 stale line refs.

---

## Fix Sprint 12 — 2026-06-19 (Validation derives + PHB mechanics + type drift)

### Input validation (6 bodies)

| # | Body | Validations added |
|---|---|---|
| V-1 | `AttackBody` | `#[derive(Validate)]`; `target_id` (Uuid), `attack_expression`/`damage_expression` ≤ 64 chars, `damage_type` 1-32 chars, `damage_die` ≤ 16, `ability` ≤ 8, `cover` ≤ 16, `label` ≤ 80, `weapon_id` ≤ 64, `extra_damage_expression` ≤ 64, `extra_damage_type` ≤ 32 |
| V-2 | `CastSpellBody` | `spell_slug` 1-64, `target_ids` 0-50, `upcast_level` 0-20, `damage_expression` ≤ 64, `save_dc` 0-30, `save_ability` ≤ 8 |
| V-3 | `DamageBody` | `amount` -1000..10000, `damage_type` 1-32, `label` ≤ 80 |
| V-4 | `HealBody` | `amount` -1000..10000, `label` ≤ 80 |
| V-5 | `SkillCheckBody` | `skill` 1-32, `dc` 0-50, `label` ≤ 80 |
| V-6 | `SaveBody` | `ability` 1-8, `dc` 0-50, `label` ≤ 80 |

Each handler now `body.validate().map_err(|e| AppError::BadRequest(...))?` at entry.

### PHB mechanics (2 fixes, 4 new test assertions)

| # | Issue | File | Status |
|---|---|---|---|
| PHB-1 | `resolve_attack` did not auto-crit on paralyzed/unconscious within 5ft (PHB p.292) | `combat_engine/resolvers/attack.rs:188-205` | ✅ Fixed — check `target_stats.paralyzed \|\| unconscious` + `within_5ft`; force `critical = true` regardless of natural roll |
| PHB-2 | `petrified` condition resistance list was missing psychic/radiant/necrotic/force (PHB p.183: "resistance to all damage") | `combat_engine/stats/compute.rs:28-32` | ✅ Fixed — added 4 damage types; test `condition_petrified_full_effects` now asserts all 4 are present |

### Type drift (2 fixes)

| # | Issue | File | Status |
|---|---|---|---|
| Drift-2 | `web/src/lib/types.ts:307` had `last_hit_attacker?: string` referring to a column dropped in migration `20260617000001` | `web/src/lib/types.ts` | ✅ Fixed — removed field, added comment pointing to `pending_hits` JSONB queue as replacement |
| Drift-3 | `castResult` frontend type was missing `hp_after`, `temp_hp_after`, `effects_applied`, `save_total`, `instant_death`, `overlay_created`, `concentration_required`, `spell_level`, `caster_id`, `slot_level_consumed` — backend's `CastSpellResult` returns all of them, frontend typed only subset (would silently break any new display code) | `web/src/lib/api/resources.ts:243-244` + `web/src/routes/campaigns/[id]/initiative/+page.svelte:124` | ✅ Fixed — both types now match the backend's `CastSpellResult` + `CastSpellTargetResult` exactly |

### Notes (out of scope, future sprints)

- **Long-range ranged disadvantage** (PHB p.196): would need to add `range_ft` field to `AttackBody`; deferred
- **Frightened LOS check** (PHB p.290): requires tracking the source of the frightened condition; deferred
- **Finesse player choice** (PHB p.96): would need a request field; auto-pick max(str, dex) is the safe default
- **Dueling off-hand check** (PHB p.91): requires inspecting character's equipment for off-hand; deferred
- **Death save counter reset on stabilize** (PHB p.197): current behavior is conservative; full PHB requires source tracking

### Verification

- `cargo check`: 0 warnings, 0 errors
- `bunx svelte-check --threshold warning`: 0 errors, 0 warnings
- `cargo test --test combat_engine_unit`: 49 passed
- `cargo test --test combat_engine_advanced`: 132 passed
- `cargo test --test combat_integration`: 39 passed
- `cargo test --test combat_advanced`: 19 passed
- `cargo test --test combat_movement`: 13 passed
- `cargo test --test combat_full_integration`: 26 passed
- `bunx vitest run`: 630 passed

### Net audit progress (Sprint 9 + 10 + 11 + 12)

Closed 14/14 critical + 12/19 high backend + 8/18 high frontend + 4 RMW races + 4 frontend paths + 6 validation derives + 2 PHB mechanics + 2 type drifts = **30 of 32 high-impact issues + 10 smell-class issues**.

Still open: 46 backend smells (down from 52), 27 frontend UX smells, 10 untested mechanics, 110+ hardcoded strings, ~40 stale line refs.

---

## Fix Sprint 13 — 2026-06-19 (i18n cleanup round 1: most-visible hardcoded strings)

### i18n keys added (en + it)

```
initiative.label_difficulty    "Difficulty" / "Difficoltà"
initiative.label_flank          "Flank" / "Fiancheggiamento"
initiative.label_combat_log     "Combat Log" / "Registro di Combattimento"
initiative.label_monster_xp     "Monster XP" / "XP Mostri"
initiative.label_xp             "XP" / "XP"
initiative.label_diff_entries    "entries" / "voci"
initiative.label_hide_history    "Hide history" / "Nascondi cronologia"
initiative.label_show_history   "Show history" / "Mostra cronologia"
initiative.label_no_rolls        "No rolls yet" / "Nessun tiro ancora"
initiative.label_hp_short        "HP" / "PF"
initiative.label_round_prefix    "R" / "R"
initiative.label_init_prefix     "Init" / "Iniziativa"
initiative.err_select_target     "Select a target" / "Seleziona un bersaglio"
initiative.err_select_grappler   "Select your grappler" / "Seleziona chi ti sta trattenendo"
initiative.err_enter_trigger     "Enter trigger condition" / "Inserisci la condizione di attivazione"
initiative.err_enter_spell_slug  "Enter spell slug" / "Inserisci lo slug dell'incantesimo"
initiative.err_select_parsed_target "Select a target for parsed attacks" / "Seleziona un bersaglio per gli attacchi analizzati"
initiative.err_add_target        "Add at least one target" / "Aggiungi almeno un bersaglio"
initiative.err_enter_damage_expr "Enter damage expression" / "Inserisci l'espressione del danno"
initiative.err_enter_damage_amount "Enter damage amount" / "Inserisci il danno"
initiative.err_enter_healing_amount "Enter healing amount" / "Inserisci la cura"
common.unknown                   "Unknown" / "Sconosciuto"
```

### Strings replaced (11 sites)

| File | Before | After |
|---|---|---|
| `web/src/lib/combat/Banner.svelte:96,99,103,115,117` | "Difficulty", "Flank", "Combat Log", "Monster XP", "XP", "entries" | i18n keys |
| `web/src/lib/combat/DiceRoller.svelte:108,120` | "Hide"/"Show" History, "No rolls yet" | i18n keys |
| `web/src/lib/combat/CombatLog.svelte:38,51,64,34` | "Combat Log — ...", "R{n}", "{n} HP", "Unknown" | i18n keys |
| `web/src/lib/combat/MyRolls.svelte:31` | "init +{n}" | i18n key |
| `web/src/routes/campaigns/[id]/initiative/+page.svelte:1029,1056,1070,1133,1143,1161,1171,1191,1212,1227,1247` | 11× `error = '...'` | i18n keys |
| `web/src/routes/campaigns/[id]/initiative/+page.svelte:2261,2392` | `<img alt="">` | `<img alt={display_name}>` (a11y fix) |

### Migrations

None.

### Verification

- `cargo check`: 0 warnings, 0 errors
- `bunx svelte-check --threshold warning`: 0 errors, 0 warnings
- `bunx vitest run`: 630 passed

### Net audit progress (Sprint 9 + 10 + 11 + 12 + 13)

Closed 14/14 critical + 12/19 high backend + 8/18 high frontend + 4 RMW + 4 frontend paths + 6 validation + 2 PHB + 2 type drift + 11 i18n + 1 a11y = **30 high-impact + 22 smell-class closed**.

Still open: 46 backend smells, 16 frontend UX smells (down from 27), 10 untested mechanics, ~100 hardcoded strings (down from 110+), ~40 stale line refs.

---

## Fix Sprint 14 — 2026-06-19 (i18n cleanup round 2: form components)

### i18n keys added (60+ × 2 locales)

```
initiative.ph_select_target, ph_select_grappler, ph_select_ally,
initiative.ph_select_overlay, ph_select_spell, ph_manual,
initiative.ph_save_dc, ph_upcast_level, ph_search_spells
initiative.label_adv, label_dis, label_half_on_save, label_dc,
initiative.label_concentration, label_cast_as_ritual
initiative.btn_cast_spell, btn_help_ally, btn_apply_overlay_damage,
initiative.btn_use_reaction, btn_roll_save, btn_roll_skill_check,
initiative.btn_roll_multiattack, btn_skip, btn_dodge, btn_dash,
initiative.btn_disengage, btn_hide, btn_attack, btn_add
initiative.msg_escaped, msg_escape_failed, msg_grapple_success,
initiative.msg_grapple_failed, msg_target_grappled, msg_target_prone,
initiative.msg_target_pushed, msg_passed, msg_failed, msg_hit,
initiative.msg_total_dmg, msg_knock_prone_or_push, msg_dmg,
initiative.msg_saved, msg_failed_saving, msg_vs_dc, msg_rolled,
initiative.msg_versus_ac, msg_apply_surprise, msg_auto_surprise,
initiative.msg_surprised, msg_alert, msg_vs_pp, msg_nat_surprise
initiative.label_is_casting
initiative.react_shield, react_counterspell, react_opportunity,
initiative.react_custom, react_hit_received
initiative.ready_manual, ready_target_range, ready_target_attacks,
initiative.ready_target_casts, ready_attack, ready_cast_spell,
initiative.ready_dash, ready_disengage, ready_dodge, ready_help
initiative.action_atk, action_dmg, action_type
initiative.skill_acrobatics, skill_animal_handling, skill_arcana,
initiative.skill_athletics, skill_deception, skill_history,
initiative.skill_insight, skill_intimidation, skill_investigation,
initiative.skill_medicine, skill_nature, skill_perception,
initiative.skill_performance, skill_persuasion, skill_religion,
initiative.skill_sleight_of_hand, skill_stealth, skill_survival
```

### Forms updated (11 components)

| File | Replaced |
|---|---|
| `AttackForm.svelte` | Select target…/Manual…/Adv/Dis |
| `CastForm.svelte` | — Select a spell —/Concentration/Cast as Ritual/Cast Spell |
| `EscapeForm.svelte` | Select grappler…/Escaped!/Failed! |
| `GrappleForm.svelte` | Select target…/Success!/Failed!/Target grappled! |
| `HelpForm.svelte` | Select ally…/Help Ally |
| `MultiattackForm.svelte` | Select target for parsed…/Atk/Dmg/Type/+ Add/Roll Multiattack |
| `OverlayDmgForm.svelte` | Select overlay…/DC/½ on save/Apply Overlay Damage/dmg/saved/failed |
| `ReactForm.svelte` | Shield (+5 AC)/Counterspell/Opportunity Attack/Custom/Hit received: roll/is casting/Use Reaction |
| `ReadyForm.svelte` | Manual only/Target enters range/Target attacks/Target casts a spell + 6 READY_ACTIONS (Attack/Cast Spell/Dash/Disengage/Dodge/Help) |
| `SaveForm.svelte` | STR/DEX/CON/INT/WIS/CHA/DC/Adv/Dis/Roll Save/Passed!/Failed! |
| `ShoveForm.svelte` | Select target…/Knock prone (uncheck = push 5ft)/Target knocked prone!/Target pushed 5ft! |
| `SkillForm.svelte` | 18 skill names (Acrobatics, Athletics, …) + 6 ability codes (STR/DEX/CON/INT/WIS/CHA) + DC/Adv/Dis/Roll Skill Check |
| `SurpriseForm.svelte` | Apply Surprise Round/Auto Surprise (Stealth vs PP)/vs PP:/SURPRISED/alert |

### Migrations

None.

### Verification

- `cargo check`: 0 warnings, 0 errors
- `bunx svelte-check --threshold warning`: 0 errors, 0 warnings
- `bunx vitest run`: 630 passed

### Net audit progress (Sprint 9 + 10 + 11 + 12 + 13 + 14)

Closed 14/14 critical + 12/19 high backend + 8/18 high frontend + 4 RMW + 4 frontend paths + 6 validation + 2 PHB + 2 type drift + 11 i18n + 1 a11y + 60+ form i18n = **30 high-impact + ~30 smell-class closed** (was 22, +~8 form/site i18n hits).

Still open: 46 backend smells, 8 frontend UX smells (was 16, −8 form labels), 10 untested mechanics, ~50 hardcoded strings (was ~100, −50 form strings), ~40 stale line refs.

---

## Fix Sprint 15 — 2026-06-19 (i18n round 3 + 1 missing fighting style)

### i18n round 3 (16 more strings)

| File | What |
|---|---|
| `web/src/routes/campaigns/[id]/initiative/+page.svelte:1225,1279` | 2× hardcoded `error = '...'` → i18n |
| `web/src/routes/campaigns/[id]/initiative/+page.svelte:2315-2354` | ctx menu 14 buttons (Attack, Damage, Dodge, Disengage, Dash, Hide, Cast Spell, Grapple, Shove, Help, Stand Up, Death Save, Heal, Remove from Map) — emoji kept, text via `$_()` |

3 new i18n keys (`label_cast_spell`, `label_death_save`, `label_heal`) added en + it.

### Fighting style: Defense (PHB p.91) — implemented + tested

- `ComputedStats.defense_style: bool` field added
- `compute_stats` now matches `"defense"` style name and adds `+1` to `stats.ac`
- New test: `compute_stats_defense_style_adds_ac` in `combat_engine_unit.rs` (49 → 50 tests)

### Migrations

None.

### Verification

- `cargo check`: 0 warnings, 0 errors
- `bunx svelte-check --threshold warning`: 0 errors, 0 warnings
- `cargo test --test combat_engine_unit`: **50 passed** (was 49, +1 Defense)
- `cargo test --test combat_engine_advanced`: 132 passed
- `cargo test --test combat_integration`: 39 passed
- `cargo test --test combat_advanced`: 19 passed
- `cargo test --test combat_movement`: 13 passed
- `cargo test --test combat_full_integration`: 26 passed (279 total combat tests)
- `bunx vitest run`: 630 passed

### Net audit progress (Sprint 9 + 10 + 11 + 12 + 13 + 14 + 15)

Closed 14/14 critical + 12/19 high backend + 8/18 high frontend + 4 RMW + 4 frontend paths + 6 validation + 3 PHB (auto-crit, petrified, Defense) + 2 type drift + 11+1 i18n + 1 a11y + 14 ctx menu + 16 misc form strings = **30 high-impact + ~32 smell-class closed** (was 22, +~10).

Still open: 46 backend smells, ~5 frontend UX smells, 9 untested mechanics (was 10, −1 Defense), ~30 hardcoded strings (was ~50, −16), ~40 stale line refs.

---

## Fix Sprint 16 — 2026-06-19 (N+1 cleanup + 1 untested mechanic)

### Backend N+1 fix (bulk_add_combatants)

| Before | After |
|---|---|
| 2 sequential queries per row (NPC stats fetch + dup check) | 2 batched queries (fetch all NPC stats in 1 query; fetch existing dups in 1 query using `id = any($N)`) |
| For 10 combatants: 20+ round-trips | 2 round-trips + 1 insert per row (10 inserts = 12 total) |

Implementation:
- `bulk.rs` now collects distinct NPC ids + character/npc ids upfront
- Batched fetches via `id = any($N)` for both stats and dup check
- Reserves already-seen ids in `HashSet` so duplicate rows in the same payload are also caught
- Validation pass + insert pass split (errors collected in pass 1; inserts in pass 2)

### Untested mechanic closed: condition immunity by creature type (5 tests)

| Test | Asserts |
|---|---|
| `undead_immune_to_poison_exhaustion_frightened_charmed` | PHB: undead immune to all 4 |
| `construct_immune_to_paralyzed_and_petrified` | PHB: construct immune to both |
| `plant_immune_to_blinded_and_deafened` | PHB: plant immune to both |
| `humanoid_not_immune_to_any` | humanoid has no creature-type immunities |
| `non_type_specific_conditions_unaffected_by_type` | restrained/prone/stunned are NOT in any type's table |

### Migrations

None.

### Verification

- `cargo check`: 0 warnings, 0 errors
- `bunx svelte-check --threshold warning`: 0 errors, 0 warnings
- `cargo test --lib`: **28 passed** (was 23, +5 creature-type immunity tests)
- `cargo test --test combat_engine_unit`: 50 passed
- `cargo test --test combat_engine_advanced`: 132 passed
- `cargo test --test combat_integration`: 39 passed
- `cargo test --test combat_advanced`: 19 passed
- `cargo test --test combat_movement`: 13 passed
- `cargo test --test combat_full_integration`: 26 passed (279 total combat tests)
- `bunx vitest run`: 630 passed

### Net audit progress (Sprint 9-16)

Closed 14/14 critical + 12/19 high backend + 8/18 high frontend + 4 RMW + 4 frontend paths + 6 validation + 3 PHB + 2 type drift + ~32 smell-class + 1 untested mechanic (creature-type immunity) = **30 high-impact + ~33 closed**.

Still open: 45 backend smells (was 46, −1 N+1 fix), 8 untested mechanics (was 9, −1 creature-type immunity), ~30 hardcoded strings, ~40 stale line refs.

---

## Fix Sprint 7 — 2026-06-16 (M15 + M21b partial)

### Past-tense WS event rename + more i18n

| # | Issue | File | Status |
|---|---|---|---|
| M15 | 41 past-tense WS event names violate §5.3 (present-tense) | `backend/src/routes/combat/*.rs` + `effects.rs` + `web/src/routes/campaigns/[id]/initiative/+page.svelte` + `character/+page.svelte` | ✅ Fixed — 36 combatant events, 5 encounter events, 5 other events renamed to present-tense (e.g., `combatant_attacked` → `combatant_attacks`, `encounter_started` → `encounter_starts`, `effects_changed` → `effects_change`). Frontend `combatant_*` prefix listener + explicit `===` checks updated. |
| M21b | ~30 more hardcoded English strings (action chips, death save, damage/attack labels) | `+page.svelte`, `en.json` + `it.json` | ✅ Partial — ~100 i18n keys added (death save, action labels, common UI); 12+ most-visible strings extracted (`opp_attack`, `ds_*`, `label_attack`, `label_damage`, `label_surprised_combatants`, etc.) |

### Migrations

None (rename only — no schema change).

### Verification

- `cargo test`: 479 passed / 0 failed
- `bunx svelte-check`: 0 errors, 0 warnings
- Backend emit/listen: 46 event names renamed (present-tense)
- Frontend prefix listeners: `combatant_*` automatically catches all renamed events
- i18n additions: ~100 keys × 2 locales = ~200 entries (action labels, death save, common UI)

### Notes

- `reaction_window`, `lair_action`, `next_turn`, `surprise_auto`, `message`, `whisper`, `dice_roll`, `character_updated`, etc. were left as-is (already noun-phrase or present-tense, not past-tense verbs).
- `combatant_save` → `combatant_saves` (was a verb in past tense; could be misread as noun "save" in some contexts but the audit classified it as past tense).


---

## Fix Sprint 6 — 2026-06-16 (L1b + M21b)

### More actions.rs splits + NpcStatBlock i18n

| # | Issue | File | Status |
|---|---|---|---|
| L1b | actions.rs 2,038 lines (over 500-line cap) | `actions.rs` → `actions/combat.rs` + `actions/economy.rs` | ✅ Fixed — extracted combat.rs (952 lines: attack, deal_damage, heal, death_save, skill_check, roll_save, computed_stats) and economy.rs (950 lines: dodge, disengage, help_action, opportunity_attack, delay_turn, two_weapon_fight, dash, hide, contested_hide, search_action, use_object). actions.rs now 14 lines (re-export shim only) |
| M21b | NpcStatBlock had ~80 hardcoded English strings | `NpcStatBlock.svelte`, `en.json` + `it.json` | ✅ Partial — 49 strings extracted (6 ability labels, 7 stat labels, 12 section labels, 5 placeholders, 5 "+ Add" buttons, 4 sense labels, 10 placeholder/category labels); ability scores, section headers, stat block labels all use `$_('npcs.*')` |

### Verification

- `cargo test`: 479 passed / 0 failed
- `bunx svelte-check`: 0 errors, 0 warnings
- actions.rs: 2,038 → 14 lines (-99%)
- New file sizes: combat.rs 952, economy.rs 950, reactions.rs 334, sync.rs 88 (total 2,338 in 4 submodules)
- i18n additions: 49 keys × 2 locales = 98 entries in `npcs.*` namespace

### Migrations

None (refactor only).


---

## Fix Sprint 5 — 2026-06-16 (M19b + M21 partial + L1 partial)

### EffectPanel confirms + i18n extraction + actions.rs split (no new tests)

| # | Issue | File | Status |
|---|---|---|---|
| M19b | EffectPanel addEffect/applySpell/removeEffect had no `confirm()` | `EffectPanel.svelte`, `en.json` + `it.json` | ✅ Fixed — 3 new i18n keys × 2 locales; `confirm()` before mutation |
| M21 | Hardcoded damage types, ability scores, cover levels, trigger events | `+page.svelte:1880-1894`, `en.json` + `it.json` | ✅ Partial — 24 strings extracted (12 damage types, 6 abilities, 3 cover, 3 trigger_event); ~180 remain |
| L1 | actions.rs 2,401 lines (4.8× 500-line cap) | `actions.rs` → `actions/sync.rs` + `actions/reactions.rs` | ✅ Partial — extracted 2 submodules (88 + 334 lines); actions.rs now 2,038; needs 2-3 more splits |

### Migrations

None.

### Verification

- `cargo test`: 479 passed / 0 failed
- `bunx svelte-check`: 0 errors, 0 warnings
- actions.rs: 2,401 → 2,038 lines (-363 = -15%)
- actions/sync.rs: 88 lines (sync_combatant_hp_to_sheet, sync_combatant_hp_to_sheet_tx, refresh_combatant)
- actions/reactions.rs: 334 lines (react, auto_trigger_ready_actions_for_event, ready_action + their structs)

### i18n additions (24 keys × 2 locales)

`initiative.damage_type_*` (12), `initiative.ability_*` (6), `initiative.cover_*` (3), `initiative.trigger_event_*` (3), `initiative.effect_*_confirm` (3 EN + IT)


---

## Fix Sprint 4 — 2026-06-16 (H5b + H8 + M19 partial)

### Counterspell ability check + frontend button guards + missing confirms

| # | Issue | File | Status |
|---|---|---|---|
| H5b | Counterspell: low-slot counter auto-failed (no ability check roll) | `actions.rs:1083-1087, 1190-1280` | ✅ Fixed — `ability_check_total: Option<i32>` in `ReactBody`; client rolls d20+mod+prof, passes total; backend validates vs `DC = 10 + target_spell_level` |
| H8 | ~20+ combat action buttons fire-and-forget HTTP/WS (double-click = double-action) | `+page.svelte:31-50, 505-525, 1591-1608, 1661-1685` | ✅ Fixed — `actionInFlight: Set<string>` + `guarded(key, fn)` helper; 5+ critical buttons guarded (start/end/next/prev/useAction×3/legendary) |
| M19 | Missing `confirm()` on destructive ops: end encounter, placeAllTokens, clearMap, removeToken | `+page.svelte:505-525, 1390-1420`, `en.json` + `it.json` | ✅ Fixed — added 4 confirms in EN + IT; end_encounter_confirm, place_all_tokens_confirm, clear_map_confirm, remove_token_confirm |

### Tests (3 new, 476 → 479)

- `counterspell_ability_check_success` (H5b) — low slot + ability check meeting DC → 200
- `counterspell_ability_check_failure` (H5b) — low slot + low check → 400
- `counterspell_low_slot_requires_ability_check` (H5b) — low slot without check → 400

### Previously High / Medium — Now Fixed

- **H5b** Counterspell ability check (H5 split)
- **H8** Double-click double-action risk on combat buttons
- **M19a** 4 of 6+ missing confirms (end/placeAll/clearMap/removeToken)

### API change

`POST /api/v1/combatants/{id}/react` body adds (optional, backward compat):
- `ability_check_total: i32` — for low-slot Counterspell; backend validates vs `10 + target_spell_level`

### Migration

None (no schema change in Sprint 4).

### Verification
- `cargo test`: 479 passed / 0 failed
- `bunx svelte-check`: 0 errors, 0 warnings


---

## Fix Sprint 6 — 2026-06-22 (Combat audit C1-C4 atomicity)

### CRITICAL atomicity: events published after commit, single tx per handler

| # | Issue | File | Status |
|---|---|---|---|
| C1 | `set_initiative` per-row autocommit + `turn_order = coalesce(turn_order, 0)` collides all updated combatants on slot 0; WS published per-row before next UPDATE | `routes/combat/encounters/initiative.rs:31-54` | ✅ Fixed — single tx, batch UPDATE initiatives + ROW_NUMBER re-sort (pattern from `start.rs:50-62`), single `combatant_updates` batch event after commit |
| C2 | `shove` action-consume + conditions/token writes in autocommit — partial state on failure | `routes/combat/special/shove.rs:92-128` | ✅ Fixed — wrapped action consume + conditions update + token update in single tx; WS published after commit |
| C3 | `overlay_damage` per-target HP UPDATE in loop on `&s.db` autocommit — partial state on failure | `routes/combat/tactical/hazards.rs:151-156` | ✅ Fixed — wrapped all target HP UPDATEs in single tx; single `overlay_damages` event after commit |
| C4 | `tick_effects` `ws::publish` inside open tx — events for state that may roll back | `routes/combat/tick.rs:137,164,192,275,308,332` | ✅ Fixed — refactored to return `Vec<String>` events; 3 callers (`next_turn`/`prev_turn`/`goto_turn`) publish after commit |

### Refactor pattern: WS-after-commit

`tick_effects` now collects events to a `Vec<String>` and returns them. All 3 callers (`encounters/turns.rs:84-93,146-155,204-213`) commit, then publish the collected events. Eliminates the desync window where clients see state events for state that rolls back.

### Tests (3 new)

- `initiative_turn_order_unique_after_batch_set` — set initiative on 3 combatants; verify no two share `turn_order`
- `shove_action_and_conditions_atomic_on_roll_back` — simulate failure; verify action NOT consumed
- `hazard_damage_all_targets_in_single_tx` — 3 targets in AoE; verify all-or-nothing

### New PHB Violations Discovered (2026-06-22 audit, NOT yet fixed)

These were uncovered by the 2026-06-22 deep-dive. Severity + fix in `COMBAT_AUDIT.md`:

| # | PHB Rule | Status | File |
|---|----------|--------|------|
| TWF-1 | TWF main-hand must be `light` (PHB p.195) | ✅ **Closed 2026-06-22** (HIGH-6) | `actions/economy/twf.rs:80-108` |
| ATK-1 | Within-5ft uses 5% threshold; PHB 5ft = 20% of map per `move_combatant.rs:89` | ✅ **Closed 2026-06-22** (HIGH-2) | `combat_engine/resolvers/attack.rs:56, 219` (`d_pct < 20.0`) |
| ATK-2 | Auto-cover `cover="full"` (≥3 blockers) → 0 AC bonus (dead branch); should reject | ✅ **Closed 2026-06-22** (HIGH-3) | `combat_engine/resolvers/attack.rs:24-27` (returns Err for `cover=full`) |
| RNG-1 | Spell range formula broken: `dist_ft = g_size * dist_pct`. 150ft Fireball targets <0.75ft. Same bug in attack/opportunity/twf | ✅ **Closed 2026-06-22** (HIGH-4) | `spells/cast.rs:325` (`dist_pct × 0.25`); attack/opp/twf already correct |
| HP-1 | `apply_hp_damage` does not clamp HP to 0 — 0-HP target takes dmg → `hp_current = -X` | ✅ **Closed 2026-06-22** (HIGH-5) | `combat_engine/resolvers/damage_type.rs:62` (`saturating_sub.max(0)`) |
| INIT-1 | Mid-encounter `set_initiative` re-uses `turn_order=0` collision pattern | ✅ **Closed 2026-06-22** (HIGH-7+10) | `encounters/initiative.rs:32-99` (tx + batch unnest + ROW_NUMBER) |
| DEL-1 | `delete_combatant` does not recompute `turn_order` — gaps break `next_turn` | ✅ **Closed 2026-06-22** (HIGH-8) | `combatants/delete.rs:30-45` (ROW_NUMBER renumber in same tx) |
| CON-1 | `stunned`/`unconscious` not in auto-fail STR/DEX save list | ✅ **Closed 2026-06-22** (MED-2) | `combat_engine/resolvers/save.rs:42-44` covers paralyzed+petrified+stunned+unconscious |
| CON-2 | `stunned` does not trigger attacks-against-adv (only paralyzed/unconscious/restrained) | ✅ **Closed 2026-06-22** (MED-3) | `combat_engine/stats/compute.rs:32-40` adds `attack_advantage_against` |
| CON-3 | `blinded` does not grant attacks-against-adv (PHB: attacker dis AND target adv) | ✅ **Closed 2026-06-22** (MED-1) | `combat_engine/stats/compute.rs:19-24` sets both `attack_disadvantage` and `attack_advantage_against` |
| EVA-1 | Evasion halves on DEX save **success** only; PHB: also halves on **failure** | ✅ **Closed 2026-06-22** (MED-4) | `spells/cast.rs:392-407` + `tactical/hazards.rs:150-160` (FAIL→½, SUCCESS→0) |
| DTH-1 | Damage at 0 HP does not add death save failure (PHB p.197) | ✅ **Closed 2026-06-22** (MED-7) | `actions/combat/damage.rs:103-127` + `attack_apply.rs:121-152`; melee crit within 5ft → 2 failures |
| HAZ-1 | Hazard radius (feet) compared against percent coords; uses `r` as % directly | ✅ **Closed 2026-06-22** (MED-9) | `tactical/hazards.rs:66` + `tick.rs:247` (`radius_pct = radius_ft * 4.0`) |
| WS-1 | `combatant_attacks/damages/heals/death_saves` broadcast `hp_after` to all members; `is_visible` mask missing | ✅ **Closed 2026-06-22** (MED-12) | `class_feature.rs:514-527` drops `hp_after` from `combatant_uses_class_feature` WS payload; 10/10 handlers now clean |
| CON-4 | Poisoned → ability-check dis not implemented (only attack dis) | ✅ **Closed 2026-06-22** (LOW-13) | `combat_engine/resolvers/skill_check.rs:57` |
| CON-5 | Restrained `save_disadvantage_for("dex")` ignores ability param, sets global dis | ✅ **Closed 2026-06-22** (LOW-14) | `combat_engine/types.rs:244,294-298` + `resolvers/save.rs:18-21` per-ability HashSet |
| CON-6 | Frightened attacker dis without LOS check on source (PHB p.290) | ⚠️ **Partial 2026-06-22** (LOW-15) | `combat_engine/resolvers/attack.rs:91-95` gates `frightened` dis on `!attacker_stats.blinded` (blindness breaks LOS). Full source-of-fear tracking refactor still required for the not-blinded-but-source-not-visible case. |
| SPELL-1 | `upcast_level` no validation `>= spell_level` (upcast 0 / 5th-level cantrip) | ✅ **Closed 2026-06-22** (MED-6) | `cast.rs:124-129` forces `slot_level = 0` for `spell_level == 0`; leveled spells clamp `raw_upcast.max(spell_level).min(9)`. Cantrips no longer burn slots. |
| OA-1 | `opportunity_attack` validates reach + `modifiers.disengaged` but NOT leaving reach | ✅ **Closed 2026-06-22** (LOW-3+18) | `opportunity.rs:103-110` uses strict `dist_ft > attacker_reach_ft` (no +5.0 buffer), matching L16 frontend rule |
| NA-1 | `token_x/token_y` PATCH accepts NaN/+inf/-inf; `move_combatant` clamps 0..100, PATCH does not | ✅ **Closed 2026-06-22** (MED-8) | `combatants/update.rs:81-86` (`!is_finite()→50.0, else clamp(0,100)`) |

### Previously Critical — Now Fixed (this sprint)

- **C1** `set_initiative` per-row autocommit + `turn_order=0` collision — fixed via `encounters/initiative.rs:32-99` (tx + batch unnest + ROW_NUMBER)
- **C2** `shove` partial state on failure (action consumed, no effect) — fixed via `special/shove.rs:92-132` (tx wraps all writes, commit before publish)
- **C3** `overlay_damage` partial state on per-target failure — fixed via `tactical/hazards.rs:113-183` (single tx, single commit, single publish)
- **C4** `tick_effects` events for state that may roll back — fixed via `tick.rs:36-355` (returns `Vec<String>`, callers commit then publish)

### Previously HIGH — Now Fixed (2026-06-22)

All 12 HIGH bugs from `COMBAT_AUDIT.md` closed in code AND have regression tests in `backend/tests/combat_coverage_jun2026.rs`:

- **H1** Multiattack target reorder index swap → `multiattack.rs:118-193`
- **H2** Within-5ft threshold (5% → 20%) → `attack.rs:56, 219`
- **H3** Total cover dead branch → `attack.rs:24-27`
- **H4** Spell range formula → `cast.rs:325`
- **H5** HP clamp at 0 → `damage_type.rs:62`
- **H6** TWF main-hand `light` check → `twf.rs:80-108`
- **H7** Mid-encounter turn_order collisions → `initiative.rs:32-99`
- **H8** Delete turn_order gaps → `delete.rs:30-45`
- **H9** conditions.rs events inside tx → `conditions.rs:165-259` (pending_events Vec)
- **H10** set_initiative race (merged with H7)
- **H11** delay TOCTOU → `delay.rs:43-50` (SELECT FOR UPDATE)
- **H12** bulk_add no tx → `bulk.rs:123-251` (tx + savepoints)

### Verification

- `cargo test`: **579 passed, 0 failed, 1 ignored** (was 437 pre-Sprint 6 → 556 pre-2026-06-22 → 573 after HIGH-no-test regressions → 579 after HIGH-already-fixed regressions)
- `bunx svelte-check`: 0 errors / 0 warnings
- `bunx vitest run`: 630 passed (20 files)



