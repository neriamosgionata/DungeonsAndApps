# Character Sheet Automation Audit

**Date**: 2026-08-04
**Scope**: character sheet automation — `web/src/routes/campaigns/[id]/character/+page.svelte` (5,344 LOC), `web/src/lib/dnd/*`, `backend/src/routes/characters.rs` (1,245 LOC), `backend/src/combat_engine/stats/*`, rest mechanics, combat↔sheet sync.
**Method**: code read + cross-check frontend formulas vs backend `compute_stats`; two parallel sub-agent deep dives (frontend $effects, backend HP/rest mechanics); all findings verified against source.
**Supersedes**: `COMBAT_AUDIT.md`, `COMBAT_AUDIT_20260622.md` (deleted — combat findings folded in below).

---

## 🔴 Critical (5)

| # | Bug | Location | Status |
|---|-----|----------|--------|
| 1 | **Uncanny Dodge deals 1.5× damage**: attack applies full damage (`actions/combat/attack_apply.rs:144-149`), then UD *subtracts* half again (`special/class_feature.rs:392-399`, `new_hp = hp_cur - halve`). Should refund half like Shield (`reactions.rs:99-108`). Pending hit also stores only `damage_applied + extra_damage_applied` (attack_apply.rs:137) — excludes Sneak/Smite (`resolvers/attack.rs:478` total includes them). | class_feature.rs:350-403, attack_apply.rs:137 | ✅ Fixed |
| 2 | **Racial ability tables diverge frontend/backend** → combat mods ≠ sheet mods: `human` +1 all missing backend; `goblin` backend str+2/dex+1 (should dex+2/con+1); `lightfoot halfling` dex double-counted (+3), cha+1 missing; `fairy` dex+3, cha+1 missing; genasi ×4 (air/earth/fire/water) missing primary +2, wrong secondary. | stats/abilities.rs:74-172 vs +page.svelte:999-1055 | ✅ Fixed |
| 3 | **Short rest hit dice dead for multiclass (pooled) sheets**: frontend reads legacy `hit_dice.current` (=0 once pools exist) → prompt "max 0" blocks spend (`+page.svelte:1161`); backend writes back flat `current` only (`characters.rs:849-850`), pools never decremented. | characters.rs:699-919, +page.svelte:1159-1170 | ✅ Fixed |
| 4 | **`hp_max_reduction` double-counted**: `computedMaxHP` returns `total - reduction` (+page.svelte:910-911), $effect writes `hp.max = computedHp` (438), UI subtracts again (2582), combatant sync subtracts a 3rd time (characters.rs:522). After any level-up effective max = total − 3×reduction. | +page.svelte:884-912,438,2582; characters.rs:516-522 | ✅ Fixed |
| 5 | **`hp_max_reduction` never cleared**: long rest omits it (characters.rs:1033-1038). PHB: ends at long rest. Short rest also caps heal at raw max (characters.rs:786) → over-heals past effective max. | characters.rs long_rest | ✅ Fixed |

## 🟠 High (7)

| # | Bug | Location | Status |
|---|-----|----------|--------|
| 6 | Exhaustion 6 = death not enforced: `exhaustion_dead` set (stats/compute.rs:159), zero readers. Combatant keeps turns (turns.rs), takes hazard damage, regens, any heal revives. Comment claims loader/turn-start skip — false. | stats/compute.rs:152-160 | ✅ Fixed |
| 7 | Hazard turn-start damage ignores saves entirely: `tick.rs:288 let _ = (save_ability, save_dc, half_on_save)` — full damage always, no half-on-save, no evasion. | routes/combat/tick.rs:288 | ✅ Fixed |
| 8 | Massive-damage threshold uses raw `hp_max`, ignores exhaustion-4 halved max → instant death 2× too late. | attack.rs:482, damage.rs:39-40, cast.rs:560, polearm.rs:144, two_weapon_fight.rs:155 | ✅ Fixed |
| 9 | Long rest revives the dead: sets `'alive', true` + death_saves 0/0 unconditionally (characters.rs:1034-1035); alive-guard in `update()` (404-412) bypassed. | characters.rs:1033-1035 | ✅ Fixed |
| 10 | AC divergence: backend `compute_ac_from_sheet` (ac.rs:5-41) ignores `sheet.ac_bonus` + Dual Wielder +1; frontend `computedAC` has both (+page.svelte:874-881). | stats/ac.rs | ✅ Fixed |
| 11 | Resource max frozen at seed level: `if (existing.has(tpl.name)) continue` (+page.svelte:285) → Ki stuck at 2, Superiority Dice stuck at 4, never rescale on level-up. Superiority Dice seeded for ALL fighters 3+ (resources.ts:88), not just Battle Master. | +page.svelte:278-289, resources.ts | ✅ max rescale (subclass gating still open) |
| 12 | Bardic Inspiration max = 1 forever: template passes hardcoded 0 chaMod (resources.ts:70); CHA rescale only in toAdd-empty branch (+page.svelte:387-393); rescale reads raw `abilities.cha` (384), ignores racial/override. | resources.ts:70, +page.svelte:383-393 | ✅ Fixed |

## 🟡 Medium (10)

| # | Bug | Location | Status |
|---|-----|----------|--------|
| 13 | Long rest legacy (non-pools) HD restored to FULL max; response claims half. | characters.rs:983-991,1046 | ✅ Fixed |
| 14 | Long rest per-pool HD: `cur + ceil(mx/2)` per pool ≠ PHB half-of-total (pools 3+3 → 4 restored, should be 3). | characters.rs:964-976 | ✅ Fixed |
| 15 | Manual slot levels deleted by class $effect (any key not in baseline). | +page.svelte:314-320 | ✅ Fixed |
| 16 | Race change leaves orphaned seeded data; revert A→B→A no-ops. | +page.svelte:450-506 | ✅ Fixed |
| 17 | `pendingPatch` dropped on re-entrancy → class+race simultaneous patch lost. | +page.svelte:430 | ✅ Fixed |
| 18 | Removed class never revokes saves (346-349); pools zeroed but kept (419-425). | +page.svelte | ✅ Fixed |
| 19 | `crit_range` silently overwritten for Champion. | +page.svelte:369 | ✅ Fixed |
| 20 | Initiative semantics diverge: frontend `sheet.initiative` = total (replaces dex); backend adds `dex + initiative` (compute.rs:181-182). Latent (server never rolls init). | compute.rs:181-182 | ✅ Fixed |
| 21 | Aura of Protection self-only, no radius. | compute.rs:188-194,220 | ⏸ deferred (needs encounter-wide context) |
| 22 | short_rest CON mod ignores racial + overrides → wrong heal/die. | characters.rs:747-753 | ✅ Fixed |

## ⚪ Low (4)

- Manual AC edit silently ignored once armor type set — **fixed**: `ac_manual` override; armor/shield handlers reset it (frontend + backend parity)
- No-armor AC branch drops shield bonus — **fixed**: shield +2 in no-armor path (frontend + backend)
- `medium_armor_max_dex_override` applied to light/heavy too — **fixed**: medium-only (backend was already)
- Natural armor (lizardfolk) blocks Monk unarmored movement — **fixed**: PHB "not wearing armor" = not light/medium/heavy

---

## Missing features (still open)

- Sneak Attack auto — **partially implemented** (adv/ally-adjacent + once/turn via `sneak_attack_used_this_turn` + scaling dice; manual toggle, no auto-trigger)
- Divine Smite — partially implemented (`smite_slot_level` consumes slot, +1d8 vs undead/fiend)
- Metamagic (0/8), Stunning Strike, Ki abilities (Flurry/Patient/Step), Wild Shape stat blocks, Eldritch Invocations, Battle Master maneuvers (0/16), Turn/Destroy Undead, Countercharm, Song of Rest, Magical Secrets, Deflect Missiles, Rage persistence (15 turns / end-if-no-damage), Brutal Critical, Shield Master, GWM crit/kill BA attack, spell components (M), ritual +10min, falling damage, mounted combat, dim light beyond overlay zones, racial resistance database frontend.

## Fixed 2026-08-04 (round 2 — HIGH)

- CRIT 1: UD refunds half (new_hp = hp_cur + dmg − halve, capped at effective max); pending_hits stores total incl. sneak/smite
- CRIT 2: `abilities.rs` racial table synced to frontend (human +1 all; goblin dex+2/con+1; lightfoot dex+2/cha+1; fairy dex+2/cha+1; air/earth/fire/water genasi mains)
- CRIT 3: short rest decrements hit dice pools server-side (dice spent across pools in order); frontend computes total from pools; legacy flat field kept in sync
- CRIT 4: `computedMaxHP` returns raw total (no −reduction); reduction applied exactly once at display + combatant sync
- CRIT 5: long rest clears `hp_max_reduction` + restores combatant `hp_max`; short rest caps heal at effective max
- MED 13: legacy long-rest HD restores half (min 1), response consistent
- MED 22: short_rest CON mod honors racial + `abilities_override`

### Round 2 — HIGH fixes (2026-08-04)

| # | Fix |
|---|-----|
| H6 | Exhaustion 6 = death enforced: turn-start skips dead combatants (`turns.rs` next/prev/goto — no economy reset, stale `action_used` blocks actions); `tick.rs` skips hazards/regen/conditions; heal, Lay on Hands, attack, damage, spell-cast, polearm BA, TWF reject dead targets |
| H7 | Hazard zone turn-start damage now rolls the save: `save_mods` + d20 vs DC, `half_on_save`, Evasion (DEX), resistances/immunities via `apply_damage_type` (`tick.rs`) |
| H8 | Massive-damage threshold uses halved max at all 5 sites (attack, damage, polearm, TWF resolvers + spell cast) |
| H9 | Long rest rejects dead characters (alive=false + 3 fails → 400); unconscious (≤2 fails) still benefits |
| H10 | Backend `compute_ac_from_sheet` honors `sheet.ac_bonus` + Dual Wielder +1 (two equipped melee weapons) — matches frontend |
| H11 | Class resource maxes rescale upward on level-up via `expectedMax` map (Ki, Superiority Dice, etc.) |
| H12 | Bardic Inspiration max = CHA mod (min 1) via `abilityModForChar` (racial + override aware), applied in all branches |

Tests: +2 unit (`resolve_attack_massive_damage_uses_halved_max_for_exhausted`, `compute_stats_ac_includes_sheet_bonus_and_dual_wielder`), +1 DB (long-rest dead reject), +2 DB (attack/heal rejected on exhaustion-6 target). H7/H6 turn-skip logic verified by code read (DB/RNG-bound, no automated test).

### Round 3 — regression + consistency fixes (2026-08-04)

| # | Issue | Fix |
|---|-------|-----|
| R1 | **REGRESSION (mine)**: multiclass short rest 500 — hit_dice placeholder `$6` collided with hardcoded `$4`/`$5` (resources/features); bind order broken | hit_dice always `$3`; slots `$6` |
| R2 | UD refund over-restored when temp HP absorbed part of hit (refunded half of raw damage to HP) | pending_hits stores `hp_before`/`hp_after`; refund capped at actual HP lost |
| R3 | UD + Shield capped at `hp_max − reduction` but the combatant `hp_max` column is ALREADY effective (reduction applied at sheet→combatant sync) → double-subtract | cap at column value; Shield restores actual HP lost (temp-aware) |
| R4 | `create.rs` stored RAW hp_max for character-linked combatants — broke the effective-column invariant (hpRatio/healDelta/Shield/UD caps) | apply reduction at create |
| R5 | initiative page `effectiveMx = mx − reduction` + `hpRatio` double-subtracted (column already effective) | use column directly |
| R6 | `drinkPotion` capped at raw max → healed past effective max | cap at effective max |
| R7 | hazard saves: `save_ability` not lowercased + exhaustion 1–3 save disadvantage not applied | lowercase + `2d20kl1` |
| R8 | heal + Lay on Hands could revive death-saves-dead (alive=false + 3 fails) — inconsistent with H9 long-rest rule | reject like long rest |
| R9 | 2 DB-gated tests broken: `uncanny_dodge_halves_real_pending_hit` asserted stale semantics; dead-rest test hit `character_limit` 1 | rewrote UD test to refund semantics; split unconscious-rest into own test |

Tests: +1 DB (`long_rest_allowed_for_unconscious_character`). UD/Shield refund semantics now temp-aware.

### Round 4 — Medium fixes (2026-08-04)

| # | Fix |
|---|-----|
| M14 | Long rest regains half of TOTAL max hit dice, distributed across pools in order (was ceil(mx/2) per pool → 3+3 pools restored 4, now 3) |
| M15 | Class $effect no longer deletes slot levels outside the baseline (manual rows preserved) |
| M16 | Race seeding persisted via `_race_seed` marker: race change removes prior race's auto-seeded fields (only when still matching seed — user edits survive), re-applies new race's; A→B→A revert works; old-race spells cleaned + new-race spells re-seeded |
| M17 | Auto-seed patches merge in a queue (pendingAutoPatches) instead of dropping on re-entrancy; race + class seeds fold in order |
| M18 | Removed class revokes auto-granted saves (`_auto_saves`) + auto-seeded resources (`_auto_resources`) + drops its hit-die pool (was zeroed but kept) |
| M19 | `crit_range` only auto-set when never touched (no silent overwrite of manual values); subclass added to class sig so Champion/Draconic react to subclass changes |
| M20 | Backend `initiative_bonus` uses `sheet.initiative` as full override (replaces DEX), matching frontend |
| M21 | Aura of Protection — deferred: needs encounter-wide ally+position context |

Tests: +1 unit (`initiative_bonus_uses_override_as_total`), +1 DB (`long_rest_restores_half_of_total_hit_dice_across_pools`). Flaky massive-damage unit test guarded against nat-1 auto-miss.

### Round 5 — Low fixes (2026-08-04)

| # | Fix |
|---|-----|
| L1 | Manual AC edit now overrides armor computation via `ac_manual` marker (frontend `computeAC` + backend `ac.rs`); armor/shield handlers reset it |
| L2 | No-armor AC path gains shield +2 (was dropped) — frontend + backend |
| L3 | `medium_armor_max_dex_override` applies to medium armor only (frontend) |
| L4 | Monk Unarmored Movement allowed with natural/mage armor (PHB: "not wearing armor" = not light/medium/heavy) |

Tests: +1 unit (`compute_stats_ac_manual_override_and_shield_fallback`). All 5 CRIT + 2 MED + 7 HIGH + 7 MED + 4 LOW closed. Remaining: M21 aura radius (deferred, needs encounter-wide context); accepted trade-offs (resource max auto-bump overrides manual lower on level-up; first-pool-first HD spend).

---

## Round 6 — new sweep (2026-08-04, 3 parallel sub-agents + manual verification)

**Scope**: +page.svelte effects (7), resources.ts, subclasses.ts, classes.ts, characters.rs rest/sync/XP, stats/compute.rs + resolvers, FE↔BE parity.

### 🔴 Critical (2)

| # | Bug | Location | Evidence |
|---|-----|----------|----------|
| 6.1 | **New character starts at 1/1 HP** — `default_sheet()` seeds `hp 1/1`; `create()` sends only `{alignment}`; auto-seed effect keeps `current = min(1, computedHp)`. Fresh char shows 1/N until manual heal. PHB: full HP at creation. Also `default_sheet` ignores race/level entirely (a lvl-1 fighter via API gets HP 1). | characters.rs:215-228,230-237; +page.svelte:708,482 | read |
| 6.2 | **`_race_seed` array revocation broken** — `if (next[k] === v) delete next[k]` uses reference equality; after server round-trip `resistances`/`condition_immunities` arrays are fresh refs → never deleted. Dragonborn→Triton keeps `fire` AND gains `cold`. Scalars/spells/resources revoke fine (M16 partial). | +page.svelte:517-519,571-579,589-597 | read |

### 🟠 High (8)

| # | Bug | Location | Evidence |
|---|-----|----------|----------|
| 6.3 | **Champion crit progression dead (M19 regression)** — `critSet = crit_range !== undefined` treats the effect's OWN write as "manual": 19 written once → never upgrades to 18 at lvl 15. Needs persisted marker like `_race_seed`. | +page.svelte:386-388 | read |
| 6.4 | **Race trait spells stored at wrong levels** — `Math.max(1, ceil(level_required/2))` maps char-level gate to spell level: drow/tiefling `darkness` (req 5) → 3rd (real 2nd), `hellish-rebuke` (req 3) → 2nd (real 1st), firbolg `detect-magic`/`disguise-self`, triton `fog-cloud` (req 0) → cantrips (real 1st). Only `gust-of-wind`/`wall-of-water` land right. Wrong grouping + prepared counts + upcast UI. | +page.svelte:614,650; data 1209-1253 | read |
| 6.5 | **Subclass feature seeding triple-broken** — (a) name mismatch: autocomplete stores `'Berserker'`/`'Life'`/`'Draconic'`/`'Fiend'` (classes.ts:101,138,273), `getSubclassFeatures` keys `'Path of the Berserker'`/`'Life Domain'`/`'Draconic Bloodline'`/`'The Fiend'` (subclasses.ts:32,68,205,222) → **no features ever seed** for the most common choices; (b) no `f.level <= cls.level` gate — EK 3 gets `Survivor` (18), Diviner 2 gets `Greater Portent`; (c) class remove/rename keeps subclass-sourced features (`source:'subclass'` not stripped by `pruneClassData`, only `src===cls`/`cls — `). | +page.svelte:4056-4063,1889-1897; classes.ts vs subclasses.ts | read |
| 6.6 | **Thrown non-finesse weapons: FE DEX, BE STR** — `isRanged = props.includes('ranged') || (w.range && !w.range.includes('melee'))` → handaxe/javelin/spear (`20/60`) = dex. Engine: thrown → STR. Sync button writes dex total; engine ignores it. Same in initiative page. | +page.svelte:1122-1132; initiative 434-445; resolvers/attack.rs:160-173 | read |
| 6.7 | **Defense style: FE shows +0, BE +1** — `computedAC` has no defense term; `compute_stats` `stats.ac += 1` unconditionally (comment claims armor-gated — false; unarmored chars get it too). Combat AC ≠ sheet AC. | +page.svelte:1003-1013; compute.rs:393-395 | read |
| 6.8 | **Engine ignores user-stored combat values** (parity cluster): `weapon.attack_bonus` (magic +2 vanishes in combat; only `parse_multiattack.rs:124` reads it), `sheet.save_bonuses` (zero backend reads — grep), stored `casting.spell_attack/save_dc` override (engine recomputes `pb+mod`). FE treats these as authoritative; engine recomputes from scratch. | resolvers/attack.rs:154-179; compute.rs:202-227,189-190; spells/cast.rs:467 | grep + read |
| 6.9 | **Multiclass short rest rolls ALL dice as first pool's die** — `let first_die = p.first()…` then `{spent}{die}` → Paladin d10+Sorc d6, spend 2 → `2d10` (over-heal ≤+4). Decrement drains pools correctly; roll doesn't match draw. | characters.rs:728-733,790 | read |
| 6.10 | **Exhaustion levels misapplied** — `exhaustion >= 1 → save_disadvantage` (PHB: lvl 1 = ability-check dis, saves at lvl 3); NO ability-check disadvantage implemented anywhere (`resolve_skill_check` reads zero exhaustion). Saves dis a level early, checks never dis. | compute.rs:131-133; skill_check.rs | grep |

### 🟡 Medium (10)

| # | Bug | Location |
|---|-----|----------|
| 6.11 | Rolled (low) HP silently overwritten: `hpChanged = computedHp > currentMaxHp` bumps to average-formula on EVERY load/class touch — no "manual" marker. | +page.svelte:398-400,482 |
| 6.12 | `hp_max_reduction` change not in sync-change detection (`changed` compares raw hp_max only) → wraith-touch reduction edit never updates combatant `hp_max`. | characters.rs:516-524 |
| 6.13 | Short rest has NO dead guard (long rest has one, H9) — dead char rests to positive HP, stays dead. | characters.rs:699-706 vs 1009-1013 |
| 6.14 | Long rest never clears `sheet.hp.temp` (combatant `temp_hp=0` but sheet stale; frontend shows it, later PATCH re-pushes). | characters.rs:1099-1113 |
| 6.15 | `ac_base` combat effect REPLACES computed AC — mage-armor + shield loses +2, Defense style, Dual Wielder, attunement. | compute.rs:108-119 |
| 6.16 | XP level-up: single-class works via `classLevelSync` chain, but multiclass CANNOT level (sum effect reverts `level_total`); no server-side recompute (`award_xp` writes xp+level only; `hp.rs` dead code, `compute_max_hp_from_sheet` zero callers); no level-up summary/ASI/feat/spell grants. | characters.rs:1286-1297, hp.rs; +page.svelte:687-699 |
| 6.17 | Resource max never clamps DOWN on level-down (Barbarian 5→3 keeps 4 Rages); slot levels do clamp. | +page.svelte:417-421 vs 336-340 |
| 6.18 | Auto-granted saves re-added after manual removal (`savesToGrant` refills on level-up; no "user removed" marker). | +page.svelte:371,474-479 |
| 6.19 | `resources.ts` table errors: Superiority Dice 6@15 (PHB 7@15), missing 6@10/8@18, seeded for ALL fighters not just Battle Master; Artificer Infusions 2-6 (PHB 2,4,6,8,10); Cleansing Touch @6 (PHB 14); War Priest every Cleric (War Domain only, Wis-mod uses); Bardic Inspiration template `bardicInspiration(0,L)` → max 1 + `reset:'short'` at all levels (long until Font of Inspiration, Bard 5); Paladin Channel Divinity 2@6 (PHB 2@9). | resources.ts:39-44,63-64,70,78,95-98 |
| 6.20 | Short rest heal can REDUCE HP: `hp_after = (hp_current + roll_total).min(effective_max)` — negative CON mod + low rolls → negative heal. No `.max()` floor. | characters.rs:803-810 |

### ⚪ Low (8)

| # | Bug | Location |
|---|-----|----------|
| 6.21 | Long-rest pools branch writes `{"pools":…}` only — legacy `hit_dice.current/max` stale; `$3` bound but no placeholder in that branch (silently dropped). | characters.rs:1023-1063 |
| 6.22 | JoAT: FE passive perception includes `floor(pb/2)` for bard; BE `passive_scores` raw; JoAT not on initiative (RAW: DEX check). | +page.svelte:934-937; compute.rs:239-266,183-188 |
| 6.23 | Casting-ability tie-break: FE first-max (`>`), BE last-max (`max_by_key`) → Wizard 5/Cleric 5 different DC. Class w/ missing level: FE votes 1, BE skips. | +page.svelte:1101-1119; abilities.rs:38-58 |
| 6.24 | Natural armor `max_dex` defaults 0 (homebrew "15+DEX" without max_dex → DEX excluded entirely). | ac.rs:19-23 |
| 6.25 | Loot-tab encumbrance uses base STR; `computedSpeed` uses racial-adjusted → 2 results for same char (Orc 10 vs 12). | +page.svelte:3934 vs 1071 |
| 6.26 | `abilities_override` unclamped FE (score 40 → +15) vs BE clamp 1..30 (+10). | +page.svelte:787-791; abilities.rs:11-12 |
| 6.27 | Composite race strings: `"High Elf (Sun Elf)"` → FE dex+2/int+1, BE int+1 only (exact base match fails); 3 copies of racial table (lib + 2 inline) drift risk. | racialBonuses.ts:52-58 vs abilities.rs:67-183 |
| 6.28 | `award_xp` stores xp uncapped (reads clamp 355k); `sync_combatant_hp_to_sheet_tx` dead code double-penalizes reduction if wired; `load.rs:51` level_total from sheet JSONB not DB column; short-rest prompt `parseInt` NaN on junk; `level_total` empty input → 0; warlock pact display max-not-sum; Draconic Resilience missing from `computedMaxHP`; petrified lacks STR/DEX autofail; racial spell lists lack `__N` variants (High Elf (Sun Elf) etc. — race matching is `includes` substring). | various |

### Missing features (still open, verified absent)

XP→level wizard + level-up grant summary; spells-known automation + known caps (Bard 10+/Warlock 10+ quirks); ASI/feat auto-grants @4/8/12/16/19; starting equipment/gold/proficiencies at creation; multiclass proficiency grants (PHB p.164); subclass coverage in subclasses.ts (10 classes only, no Artificer/Blood Hunter; 1 subclass option for most); hit-dice spend UI (prompt only); per-class spell-slot attribution for shared slots; M21 aura radius still deferred.

**Verified intact from earlier rounds**: long-rest half-of-total HD, hp_max_reduction caps (short rest + combatant sync), exhaustion-6 death blocks, alive guards on heal/long-rest, warlock pact short-rest refill, racial tables (44 races, standard names), AC paths, M20 initiative override, proficiency scaling, crit range engine read.
