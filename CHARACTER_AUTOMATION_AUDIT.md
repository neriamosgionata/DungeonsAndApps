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
| 6 | Exhaustion 6 = death not enforced: `exhaustion_dead` set (stats/compute.rs:159), zero readers. Combatant keeps turns (turns.rs), takes hazard damage, regens, any heal revives. Comment claims loader/turn-start skip — false. | stats/compute.rs:152-160 | ❌ open |
| 7 | Hazard turn-start damage ignores saves entirely: `tick.rs:288 let _ = (save_ability, save_dc, half_on_save)` — full damage always, no half-on-save, no evasion. | routes/combat/tick.rs:288 | ❌ open |
| 8 | Massive-damage threshold uses raw `hp_max`, ignores exhaustion-4 halved max → instant death 2× too late. | attack.rs:482, damage.rs:39-40, cast.rs:560, polearm.rs:144, two_weapon_fight.rs:155 | ❌ open |
| 9 | Long rest revives the dead: sets `'alive', true` + death_saves 0/0 unconditionally (characters.rs:1034-1035); alive-guard in `update()` (404-412) bypassed. | characters.rs:1033-1035 | ❌ open |
| 10 | AC divergence: backend `compute_ac_from_sheet` (ac.rs:5-41) ignores `sheet.ac_bonus` + Dual Wielder +1; frontend `computedAC` has both (+page.svelte:874-881). | stats/ac.rs | ❌ open |
| 11 | Resource max frozen at seed level: `if (existing.has(tpl.name)) continue` (+page.svelte:285) → Ki stuck at 2, Superiority Dice stuck at 4, never rescale on level-up. Superiority Dice seeded for ALL fighters 3+ (resources.ts:88), not just Battle Master. | +page.svelte:278-289, resources.ts | ❌ open |
| 12 | Bardic Inspiration max = 1 forever: template passes hardcoded 0 chaMod (resources.ts:70); CHA rescale only in toAdd-empty branch (+page.svelte:387-393); rescale reads raw `abilities.cha` (384), ignores racial/override. | resources.ts:70, +page.svelte:383-393 | ❌ open |

## 🟡 Medium (10)

| # | Bug | Location | Status |
|---|-----|----------|--------|
| 13 | Long rest legacy (non-pools) HD restored to FULL max; response claims half. | characters.rs:983-991,1046 | ✅ Fixed |
| 14 | Long rest per-pool HD: `cur + ceil(mx/2)` per pool ≠ PHB half-of-total (pools 3+3 → 4 restored, should be 3). | characters.rs:964-976 | ❌ open |
| 15 | Manual slot levels deleted by class $effect (any key not in baseline). | +page.svelte:314-320 | ❌ open |
| 16 | Race change leaves orphaned seeded data; revert A→B→A no-ops. | +page.svelte:450-506 | ❌ open |
| 17 | `pendingPatch` dropped on re-entrancy → class+race simultaneous patch lost. | +page.svelte:430 | ❌ open |
| 18 | Removed class never revokes saves (346-349); pools zeroed but kept (419-425). | +page.svelte | ❌ open |
| 19 | `crit_range` silently overwritten for Champion. | +page.svelte:369 | ❌ open |
| 20 | Initiative semantics diverge: frontend `sheet.initiative` = total (replaces dex); backend adds `dex + initiative` (compute.rs:181-182). Latent (server never rolls init). | compute.rs:181-182 | ❌ open |
| 21 | Aura of Protection self-only, no radius. | compute.rs:188-194,220 | ❌ open |
| 22 | short_rest CON mod ignores racial + overrides → wrong heal/die. | characters.rs:747-753 | ✅ Fixed |

## ⚪ Low (4)

- Manual AC edit silently ignored once armor type set (+page.svelte:3020 vs 852)
- No-armor AC branch drops shield bonus (computeAC:852)
- `medium_armor_max_dex_override` applied to light/heavy too (frontend 864; backend medium-only)
- Natural armor (lizardfolk) blocks Monk unarmored movement (+page.svelte:927)

---

## Missing features (still open)

- Sneak Attack auto — **partially implemented** (adv/ally-adjacent + once/turn via `sneak_attack_used_this_turn` + scaling dice; manual toggle, no auto-trigger)
- Divine Smite — partially implemented (`smite_slot_level` consumes slot, +1d8 vs undead/fiend)
- Metamagic (0/8), Stunning Strike, Ki abilities (Flurry/Patient/Step), Wild Shape stat blocks, Eldritch Invocations, Battle Master maneuvers (0/16), Turn/Destroy Undead, Countercharm, Song of Rest, Magical Secrets, Deflect Missiles, Rage persistence (15 turns / end-if-no-damage), Brutal Critical, Shield Master, GWM crit/kill BA attack, spell components (M), ritual +10min, falling damage, mounted combat, dim light beyond overlay zones, racial resistance database frontend.

## Fixed 2026-08-04

- CRIT 1: UD refunds half (new_hp = hp_cur + dmg − halve, capped at effective max); pending_hits stores total incl. sneak/smite
- CRIT 2: `abilities.rs` racial table synced to frontend (human +1 all; goblin dex+2/con+1; lightfoot dex+2/cha+1; fairy dex+2/cha+1; air/earth/fire/water genasi mains)
- CRIT 3: short rest decrements hit dice pools server-side (dice spent across pools in order); frontend computes total from pools; legacy flat field kept in sync
- CRIT 4: `computedMaxHP` returns raw total (no −reduction); reduction applied exactly once at display + combatant sync
- CRIT 5: long rest clears `hp_max_reduction` + restores combatant `hp_max`; short rest caps heal at effective max
- MED 13: legacy long-rest HD restores half (min 1), response consistent
- MED 22: short_rest CON mod honors racial + `abilities_override`
