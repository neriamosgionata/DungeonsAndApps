# D&D 5e PHB/DMG Automation Gaps

> Generated: 2026-04-30 | Last updated: 2026-06-01 (Tier 1 combat features)
> Scope: Combat engine + character sheet + rest mechanics vs PHB/DMG

---

## Combat Automation (24 items)

| # | Feature | Status | Detail |
|---|---------|--------|--------|
| 1 | Auto AC from equipped gear | ❌ | AC is flat manual number. No armor + shield + DEX cap calculation. Magic armor +1/+2/+3 must be entered manually or as effects. |
| 2 | Attack calculation | ⚠️ | Prof bonus + ability mod (STR melee, DEX ranged, max STR/DEX finesse) auto. Fighting Styles now auto (Archery +2, Dueling +2, GWF reroll 1–2, TWF). Power Attack (−5/+10) via `power_attack: true`. **Missing:** Bless, Bardic Inspiration, magic weapon +1/+2/+3. |
| 3 | Damage calculation | ⚠️ | Crit doubles dice. Resistances/immunities work. Extra damage (`extra_damage_expression`) handles Sneak Attack, Smite, Rage. **Missing:** auto ability mod on damage without expression, versatile/two-handed auto-selection. |
| 4 | Save calculation | ✅ | Ability mod + proficiency + effect bonuses. |
| 5 | Skill check | ⚠️ | Proficiency + expertise work. Reliable Talent (Rogue 11+): floor-10 enforced in `resolve_skill_check`. **Missing:** Jack of All Trades. |
| 6 | Action economy | ✅ | Action, bonus action, reaction, movement, legendary, lair all tracked. |
| 7 | Opportunity attacks / reactions / ready / delay | ✅ | All implemented with proper economy checks. |
| 8 | Conditions auto-applied | ⚠️ | Blinded, Paralyzed, Restrained, Frightened, Poisoned, Grappled, Invisible, Surprised handled. Prone: attacker dis on ALL attacks (incl. ranged); target prone → melee adv / ranged dis. Timed conditions `name:N` tick down at turn start. Condition immunity enforced by creature type. Grapple auto-releases on incapacitation. **Missing:** Flanking does not auto-apply advantage. Cover is manual parameter. |
| 9 | Death saves | ✅ | Nat 20 → stabilize + 1 HP. Nat 1 → 2 failures. Tracked correctly. |
| 10 | Concentration checks | ✅ | Auto-roll CON save, DC = max(10, dmg/2). Auto-break on fail. |
| 11 | Multiattack | ⚠️ | Batch manual attacks exist. Does NOT auto-read creature stat block for attack count. |
| 12 | Spell attacks / saves | ✅ | `spell_attack_bonus` and `spell_save_dc` auto-computed from prof + casting mod. |
| 13 | Temporary HP | ✅ | Absorbs damage first. Only replaces if new > current (PHB rule enforced in `update_combatant`). |
| 14 | Resistance/immunity/vulnerability | ✅ | Half/zero/double damage. Supports "nonmagical" variants. |
| 15 | Invisible attackers | ✅ | Attacker advantage, target causes disadvantage. |
| 16 | Prone attackers | ✅ | Disadvantage on ALL attack rolls when attacker prone. |
| 17 | Range increments | ❌ | No long-range disadvantage automation. |
| 18 | Ammunition tracking | ⚠️ | Arrows/bolts/bullets auto-decrement. **Missing:** thrown weapons (daggers, javelins). |
| 19 | Two-weapon fighting | ✅ | Bonus-action off-hand attack via `/two-weapon-fight`. Ability mod stripped unless TWF fighting style. `twf_style` auto-detected from `sheet_raw.fighting_styles`. |
| 20 | Dodge / Disengage | ✅ | Dodge = attackers disadvantaged. Disengage = no opportunity attacks. |
| 21 | Help action | ⚠️ | Gives advantage on next attack. **Missing:** advantage on next skill check. |
| 22 | Hide action | ⚠️ | Applies "Hidden" effect. **Missing:** contested Stealth vs Perception auto-roll. |
| 23 | Surprise round | ⚠️ | `surprised` condition blocks full turn (action+BA+movement set to max at turn start, condition removed). **Missing:** auto stealth vs passive perception check to determine surprise. |
| 24 | Darkvision / dim light | ⚠️ | Overlay zones (magical_darkness, low_visibility) cause disadvantage if attacker lacks darkvision. **Missing:** dim-light/darkness beyond overlay zones. |

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
| 9 | Tool proficiencies | ❌ | Free-form text only. No structured list, no auto bonuses. |
| 10 | All 6 saves listed | ✅ | |
| 11 | Save bonus auto-calc | ✅ | Ability mod + prof if proficient. |
| 12 | Conditional save bonuses | ⚠️ | Backend checks effect modifiers. Frontend shows static total only. |
| 13 | Initiative from DEX | ⚠️ | Backend auto from DEX. Frontend defaults to DEX but user can override — can diverge. |
| 14 | AC from armor + shield + DEX | ❌ | Flat manual number. No equipment-aware calc. |
| 15 | Unarmored defense | ⚠️ | Backend parses `"10+dex+con"` from effects. **Not auto-applied** by selecting barbarian/monk class. |
| 16 | Mage armor | ⚠️ | Backend parses `"13+dex"` from effects. No mage armor toggle. |
| 17 | Max HP from hit dice | ❌ | Fully manual entry. |
| 18 | Current HP / temp HP | ✅ | Tracked and synced to combatants. |
| 19 | Hit dice pool | ✅ | `hit_dice.current/max/die`. **Limitation:** single die type only — no multiclass pooling (e.g. 3d10 + 2d8). |
| 20 | Spellcasting ability per class | ❌ | Single global `casting.ability` dropdown. No per-class auto-detection. |
| 21 | Spell save DC auto-calc | ⚠️ | Backend auto (`8 + prof + mod`). Frontend manual field — not auto-filled. |
| 22 | Spell attack bonus auto-calc | ⚠️ | Backend auto (`prof + mod`). Frontend manual field — not auto-filled. |
| 23 | Spell slots tracking | ✅ | `sheet.slots` for levels 1–9. |
| 24 | Auto-seed spell slots | ✅ | `computeBaselineSlots()` uses PHB multiclass rules + warlock pact magic. |
| 25 | Prepared vs known spells | ✅ | `prepared` boolean on each spell. Enforced in `cast_spell` for Wizard/Cleric/Druid/Paladin/Artificer (non-masters). Known-spell classes skip. |
| 26 | Ritual casting | ✅ | `cast_as_ritual: true` + `spell.ritual = true` → slot not consumed. UI shows "Cast as Ritual" checkbox for ritual spells. |
| 27 | Concentration tracking | ✅ | `sheet.concentration` with spell name + timestamp. Backend checks on damage. |
| 28 | Class resources (ki, rage, etc.) | ✅ | Auto-seed from `templatesForClass`: Ki, Rage, Channel Divinity, Superiority Dice, Sorcery Points, Wild Shape, Bardic Inspiration, Lay on Hands, Second Wind, Action Surge, Indomitable, Mystic Arcanum, Infusions. |
| 29 | Short-rest resource regain | ✅ | Frontend resets `reset === 'short'` or `'long'`. Warlock pact slots refilled. Backend only handles hit dice/HP. |
| 30 | Long-rest resource regain | ✅ | Frontend resets all `reset !== 'none'`. Backend resets HP, hit dice, exhaustion, death saves, spell slots. |
| 31 | Darkvision range | ✅ | `sheet.senses.darkvision`. Backend also from effects. |
| 32 | Racial resistances | ⚠️ | Backend supports via effects. Frontend has no racial trait database. |
| 33 | Racial ability bonuses | ❌ | Race is text field only. No mechanical effects. |
| 34 | Feat selector | ✅ | Full UI with prerequisites and config. |
| 35 | Feat mechanical effects | ⚠️ | `applyFeatEffects` handles: ability +1, init/speed/PP bonus, save/armor prof, resource creation (Lucky → Luck Points). **Many major feats empty:** Sharpshooter, GWM, Crossbow Expert, Sentinel, Polearm Master, War Caster — listed for reference, mechanics NOT enforced. |
| 36 | Equipment/inventory section | ✅ | `sheet.equipment` array with name, qty, weight, equipped flag, coin purse. |
| 37 | Equipped armor/weapons/shields | ⚠️ | Weapons have `equipped` toggle. **No structured armor type** — armor is generic equipment row with no AC automation. |
| 38 | Magic item bonuses applied | ⚠️ | `sheet.attunement` stores `bonuses` object. **NOT mechanically applied** — neither frontend display nor backend combat engine reads them. Reference only. |
| 39 | Attunement limit (max 3) | ✅ | Frontend enforces. Warns at limit. |
| 40 | Carrying capacity | ✅ | `STR × 15` computed. Total weight summed. |
| 41 | Alignment | ❌ | Not in data model or UI. |
| 42 | Background | ⚠️ | Free-form text areas (backstory, personality, ideals, bonds, flaws). No structured picker with mechanical effects. |
| 43 | Inspiration | ✅ | Binary toggle. |
| 44 | Passive Perception | ✅ | `10 + perception mod + bonus`. Backend computes for all skills. |
| 45 | Speed (race/armor/monk) | ⚠️ | `sheet.speed` is manual number. Backend applies effect modifiers. No auto race/armor/monk calculation. |
| 46 | Languages | ✅ | Free-form text field. |
| 47 | Encumbrance penalties | ⚠️ | Warning when weight > STR×15. **No speed reduction** or other mechanical penalties. |

---

## Rest Mechanics (3 items)

| # | Feature | Status | Detail |
|---|---------|--------|--------|
| 1 | Short rest | ⚠️ | Backend rolls hit dice. Frontend resets `reset === 'short'` resources. **Backend does NOT auto-reset class resources** — relies on frontend patch. |
| 2 | Long rest | ⚠️ | Backend: full heal, hit dice to half max (rounded up), spell slots restored, exhaustion −1, death saves reset. Frontend resets all `reset !== 'none'`. **Backend does NOT iterate `sheet.resources`/`sheet.features`** — relies on frontend. |
| 3 | Short vs long rest resource tracking | ⚠️ | Resources have `reset` field (`'short'|'long'|'none'`). Frontend respects it. Backend does NOT read it. |

---

## Summary by Severity

### 🔴 Critical Gaps (break core loop)

| Gap | Impact |
|-----|--------|
| Auto AC from equipped gear | Fighter must manually compute AC every time armor/shield changes |
| Auto max HP from class hit dice | Player must manually track HP across 20 levels |
| Auto damage from weapon stats | Attack endpoint requires manual `damage_expression`; no auto ability mod, versatile, two-handed |
| Spellcasting ability per class | Multiclass caster uses one global ability — wrong for wizard/cleric combos |
| Racial traits | Race is text — no darkvision, resistances, bonuses auto-applied |
| Alignment | Not tracked |
| Tool proficiencies | Not structured |

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

### 🟡 High Gaps (expected in modern VTT)

| Gap | Impact |
|-----|--------|
| Long-range disadvantage | Ranged attacks at long range not auto-disadvantaged |
| Jack of All Trades | Bard feature not implemented |
| Thrown weapon tracking | Daggers/javelins not counted |
| Max HP auto-calc | Manual entry error-prone |
| Many feats empty effects | Sharpshooter/GWM power_attack bool works; Sentinel/Polearm/Crossbow Expert still reference-only |
| Backend ignores frontend overrides | Ability/save overrides on sheet don't affect combat rolls |
| Encumbrance penalties | Warning but no speed reduction |
| NPC multiattack parsing | "2 claws + 1 bite" not parsed into batch |
| Counterspell full automation | Slot selection + spell cancellation still manual after gating |

### 🟢 Medium Gaps (nice-to-have)

| Gap | Impact |
|-----|--------|
| Hide contested roll | No auto Stealth vs Perception |
| Surprise auto-check | No auto stealth for surprise |
| Background structured picker | Free-form only |
| Multiclass hit dice pooling | Single die type only |
| Flying speed 0 → fall damage | Paralyzed/stunned fliers not grounded |
| Mounted combat | No rider/mount relationship |

---

## Class Implementation Status (as of 2026-05-04)

| Class | Resources | Spell Slots | Mechanical Features | Subclass Mechanics |
|-------|-----------|-------------|--------------------|--------------------|
| **Barbarian** | ✅ Rages (correct max by level) | — | ✅ Rage (BPS resist + dmg bonus + adv), ✅ Fast Movement (5+), ✅ Unarmored Defense armor types, ✅ Reckless Attack (adv on attack, enemies have adv vs you) | Champion Crit: ❌ (Fighter only). Berserker Frenzy: ❌ |
| **Bard** | ✅ Bardic Inspiration (manual max) | ✅ Full caster | ✅ Die scaling display (d6→d12), ✅ Evasion 7+... wait Bard has no evasion. Jack of All Trades: ❌ (displayed, not mechanical) | ❌ All subclasses reference only |
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

The app is **"manual entry with computed display"** not **"equipment-driven auto-calculation"**.

- Backend combat engine (`combat_engine.rs`) is solid for resolving attacks/saves/damage **when given expressions**.
- It does NOT deeply integrate with character sheet equipment/class features to auto-generate those expressions.
- The frontend character sheet computes display values (skill mods, save mods, PP) but many combat-relevant stats (AC, HP max, spell DC/attack, initiative) are manual fields that can diverge from computed values.

**To close the gap:**
1. Normalize equipment to structured items (armor, weapon, shield types with mechanical properties)
2. Auto-calculate AC, max HP, speed from equipped gear + class + race
3. Auto-generate `attack_expression` and `damage_expression` from equipped weapon + ability mod + prof + fighting style + magic bonuses
4. Add per-class spellcasting ability (not global single)
5. Add racial trait database with auto-application
6. Fill empty feat effect handlers (Sharpshooter, GWM, etc.)
7. Sync frontend overrides to backend combat engine

---

*End of audit. Use for feature planning priority.*
