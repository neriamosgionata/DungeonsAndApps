# Combat System Audit

**Date**: 2026-06-22
**Scope**: `backend/src/routes/combat/` (62 files, ~9,400 LOC) + `backend/src/combat_engine/` (19 files) + `web/src/lib/combat/` + `web/src/routes/campaigns/[id]/initiative/+page.svelte` + `web/src/routes/campaigns/[id]/map/+page.svelte`
**Auditor**: 3 parallel deep-dive passes (RBAC/security · atomicity/races · D&D mechanics)
**Tests**: 437 backend / 630 frontend passing — audit found 5 HIGH bugs that tests do NOT cover

---

## Executive Summary

| Severity | Count | Status |
|----------|-------|--------|
| CRITICAL | 4     | 0 Fixed |
| HIGH     | 12    | 0 Fixed |
| MEDIUM   | 13    | 0 Fixed |
| LOW      | 17    | Documented |
| INFO     | 5     | Documented |

**Verdict**: MEDIUM-HIGH risk. Combat is **correct on RBAC and SQL-injection** (62 routes audited, all gates present, all queries parameterized). **5 HIGH bugs cause visible incorrect play** (wrong damage on wrong target, dead cover branch, 2 unit-conversion bugs, HP-clamp missing). 12 HIGH-level atomicity/race issues allow partial state on failures or WS events for rolled-back tx. PHB violations noticed by rules-literate players.

---

## CRITICAL (4) — partial state / event desync

| ID | Location | Bug | Fix |
|----|----------|-----|-----|
| C1 | `encounters/initiative.rs:31-54` | No tx. Per-row UPDATE on `&s.db` autocommit. `turn_order = coalesce(turn_order, 0)` collides all updated combatants on slot 0. Multiple combatants end up at `turn_order=0`. WS published per-row before next UPDATE. | Open tx. Batch UPDATE via ROW_NUMBER subquery (pattern from `start.rs:50-62`). Publish ONCE after commit. |
| C2 | `special/shove.rs:92-124` | No tx. `action_used` UPDATE (line 93) + conditions/token UPDATEs (line 105-124) in autocommit. If condition update fails, action is consumed but no effect applied. | Wrap in `s.db.begin()`. Commit after all writes. |
| C3 | `tactical/hazards.rs:151-156` | No tx. Per-target HP UPDATE in loop on `&s.db`. Partial state on failure (some targets damaged, others not). | Open tx. Batch UPDATE all targets. Commit. Publish ONCE after. |
| C4 | `tick.rs:137,164,192,275,308,332` | `ws::publish` called INSIDE open tx passed from `turns.rs:84-93`. `tick_effects` runs within `next_turn/prev_turn/goto_turn` tx. `effects_change`, `overlays_expire`, `combatant_is_surprised`, `combatant_takes_hazard_damage`, `combatant_regenerates`, `combatant_conditions_tick` events broadcast BEFORE caller commits. If tx rolls back, clients see events for state that will revert. | Collect WS events in `tick_effects` (return `Vec<Event>`). Let caller publish after commit. |

---

## HIGH (12) — data corruption / wrong behavior

| ID | Location | Bug | Fix |
|----|----------|-----|-----|
| H1 | `special/multiattack.rs:56-105,184-219` | Multiattack target reorder: when `needs_auto=true` + `try_parse_npc_multiattack` succeeds, `targets` vec is reordered. `results` built by iterating reordered `targets`; FINAL loop iterates ORIGINAL `body.targets` by index — index mismatch → wrong damage to wrong target, wrong `combat_event` rows, sheet HP synced to wrong HP. **Data corruption**. | Build `HashMap<target_id, &Result>` after resolve. Iterate `body.targets` to look up by `t.target_id`. Or zip with `target_id` instead of index. |
| H2 | `combat_engine/resolvers/attack.rs:42-58,198-213` | Within-5ft threshold uses `d_pct < 5.0` (5% of map). Per `move_combatant.rs:89` hardcode 1 cell = 20% of map, 5ft = 20% — auto-crit (paralyzed/unconscious) and prone-advantage fire only at <1.25ft, basically never. | Change threshold to `d_pct < 20.0` or factor via `map_grid_size/5.0` properly. |
| H3 | `routes/combat/actions/combat/attack.rs:216-220` + `combat_engine/resolvers/attack.rs:22-26` | Auto-cover writes `cover = "full"` for ≥3 blockers, but `resolve_attack` only maps `"half"` → +2, `"three_quarters"` → +5; `"full"` → 0. **Dead branch**: total cover gives 0 AC bonus instead of blocking attack. PHB p.150: "can't be targeted directly". | In `resolve_attack`: `Some("full") => return AppError::BadRequest("target has total cover")`. Also fix `positioning.rs:181` inconsistent `cover_bonus: 999` (display-only). |
| H4 | `routes/combat/spells/cast.rs:307-322` (and `actions/reactions.rs:286-298`) | Spell range formula broken: `pct_per_5ft = 5.0/g_size`, `dist_ft = sqrt(dx²+dy²) * 5.0` = `g_size * dist_pct`. With g_size=50, dist_ft = 50×dist_pct. A 150ft Fireball can only target things within 3% of caster ≈ 0.75ft on 5-cell map. Effectively useless. Same bug in attack/opportunity/twf. | Standardize: `dist_ft = dist_pct * 5.0 / (100.0/5.0) = dist_pct * 0.25` (1 cell = 20% = 5ft). Or use `map_grid_size` consistently. |
| H5 | `combat_engine/resolvers/damage_type.rs:51-61` + `damage.rs:17` | `apply_hp_damage` does NOT clamp HP to 0. 0-HP target taking damage → `hp_current = -X` in DB. PHB: HP cannot go below 0. | In `apply_hp_damage` return `(0, ...)` when `hp + remaining < 0`. Or clamp at SQL level: `GREATEST(0, hp_current - $N)`. |
| H6 | `actions/economy/twf.rs:18` | TWF off-hand checks `light`; main-hand also required (PHB p.195: "a light melee weapon in one hand and a different light melee weapon in the other hand"). Main-hand check missing. | Fetch main-hand weapon and check `light` property. |
| H7 | `encounters/initiative.rs:31-43` (separate from C1) | Mid-encounter combatant addition via `set_initiative` does `turn_order = coalesce(turn_order, 0)`. New combatants slot at position 0, colliding with first combatant. | Re-sort full turn_order like `start.rs:50-62` does (ROW_NUMBER over initiative DESC, dex DESC). |
| H8 | `combatants/delete.rs:25-28` | No `turn_order` recompute after delete. Gaps in `turn_order` break `next_turn` sequence (`turns.rs:74-77` selects by `turn_order`, finds no combatant for some slot). Long-term initiative corruption. | After delete, `UPDATE turn_order = ROW_NUMBER() over (initiative DESC, dex DESC) for remaining combatants` in same tx. |
| H9 | `tactical/conditions.rs:217-226` | `ws::publish` called inside open tx (tx opened line 164, commit line 232). `combatant_loses_condition` events for grappled combatants fire before commit. Same race as C4. | Collect events, publish after `tx.commit()` at line 232. |
| H10 | `encounters/initiative.rs:31-43` | Per-combatant loop in autocommit. Two concurrent `set_initiative` calls could both UPDATE same combatant; last-write-wins. | Open tx, batch all UPDATEs via ROW_NUMBER. |
| H11 | `delay.rs:43-70` | TOCTOU: `UPDATE combatant delayed_turn + action_used` (line 43) and `UPDATE turn_order of ALL encounter combatants` (line 58-70) on `&s.db` autocommit. Two concurrent `delay_turn` calls for different combatants both succeed; both then run encounter-wide UPDATE shifting `turn_order`. Final state depends on commit order. | Add `SELECT ... FOR UPDATE` on encounter row, OR serialize via encounter row lock. |
| H12 | `combatants/bulk.rs:119-227` | No tx wrapping per-insert loop. `existing_char_ids/npc_ids` HashSet in-memory only. Concurrent `bulk_add` calls bypass dup check (rely on unique index `20260617000002` for safety). | Wrap loop in tx. Or rely fully on unique constraint + ON CONFLICT. |

---

## MEDIUM (13) — PHB violations / race windows

| ID | Location | Bug | Fix |
|----|----------|-----|-----|
| M1 | `combat_engine/stats/compute.rs:19` | `blinded` only sets `attack_disadvantage` (attacker dis). PHB: attacker dis AND attacks against blinded target have adv. | Also set `target's attack_advantage_against`. |
| M2 | `combat_engine/stats/compute.rs:21-26` | `stunned`/`unconscious` not in `save.rs:34-35` auto-fail STR/DEX list. PHB: auto-fail STR/DEX saves. | Extend auto-fail check to `paralyzed \|\| petrified \|\| stunned \|\| unconscious`. |
| M3 | `combat_engine/stats/compute.rs:26` | `stunned` does NOT trigger attacks-against-adv in `attack.rs:62-64` (only paralyzed/unconscious/restrained). | Add `target_stats.stunned` to adv list. |
| M4 | `routes/combat/spells/cast.rs:343-376` + `tactical/hazards.rs:117-146` | Evasion only halves/zeroes damage on **successful** DEX save. PHB: also halves on **failed** DEX save. | Add `else if save_passed == Some(false) && evasion && save_ability_str == "dex" { damage_applied = eff_dmg / 2; }`. |
| M5 | `routes/combat/spells/cast.rs:120,281,369-376` | `detect_damage_type` defaults to `"force"` when no modifier key matches. AoE spells without explicit damage modifier get `force` damage. | Use `spells.damage_type` column as fallback. |
| M6 | `routes/combat/spells/cast.rs:120` | `slot_level = upcast_level.unwrap_or(spell_level)` — no validation `upcast_level >= spell_level`. 2nd-level spell with `upcast_level=0` skips slot consumption. Cantrip with `upcast_level=5` consumes 5th-level slot. | Enforce `upcast_level >= spell_level`, fall back to `spell_level` on `<`. |
| M7 | `routes/combat/actions/combat/damage.rs` + `resolvers/damage.rs` | Damage at 0 HP doesn't add death save failure. PHB p.197: "Any time you take damage while you have 0 hit points, you suffer one death saving throw failure." | In `apply_attack_outcome`/`apply_damage_outcome`, when `target.hp_current == 0 && result.hp_after <= 0`, increment `death_saves.failures` (1 normal, 2 if melee crit within 5ft). |
| M8 | `routes/combat/combatants/update.rs:113-122` | `token_x: Option<f32>`, `token_y: Option<f32>` not clamped/finite-checked. `move_combatant.rs:36-37` clamps to 0..100, but PATCH path lets master write NaN/+inf/-inf. f32 NaN propagates through distance `sqrt` → permanent NaN distance → blocks all positioning. | `.filter(\|v\| v.is_finite()).clamp(0.0, 100.0)` before bind. |
| M9 | `tactical/hazards.rs:73-101` + `tick.rs:202-289` | Hazard zone radius (feet) compared against percent coords. With radius 20ft, any combatant within 20 percent of center (≈80ft-equivalent on 5-cell map) is in zone. | Convert `radius_ft → percent` via formula from H4 fix (`radius_pct = radius_ft * 4`). |
| M10 | `tick.rs:181-200` | Surprised auto-consume TOCTOU: SELECT conditions → has_condition check → UPDATE action_used. Not atomic. | `UPDATE combatants SET action_used=true, bonus_action_used=true, movement_used_ft=9999 WHERE id=$1 AND 'surprised' = ANY(conditions) RETURNING id`; check `rows_affected`. |
| M11 | `encounters/turns.rs:38-49` | TOCTOU between `SELECT encounter` (line 20) and `begin tx` (line 26). Encounter row could change status/turn_index. Status check at line 22 uses stale data. | Re-fetch inside tx. Or `SELECT FOR UPDATE` on encounters row. |
| M12 | `actions/combat/attack_apply.rs:213-230` + `damage.rs:132-144` + `heal.rs:142-153` + `death_save.rs:95-110` | **WS event HP leak**: `combatant_attacks/damages/heals/death_saves` broadcast `hp_after`, `temp_hp_after`, `damage`, `extra_damage` to ALL campaign members. `list_combatants` masks HP for hidden combatants (`is_visible` filter), but WS payload does NOT. Malicious/out-of-spec client reads WS stream → extracts HP of hidden enemies. | Drop `hp_after`/`temp_hp_after` from broadcast payload. Or send redacted copy to non-owners. Frontend already ignores these fields (re-fetches via masked list). |
| M13 | `actions/economy/contested.rs:69-78` | Contested-hide observer query filters on `ref_type` but NOT `is_visible`. Hidden NPC combatant (`is_visible=false`) can be chosen as observer, `passive_perception` exposed in response. | Add `and c.is_visible = true` to observer query. |

---

## LOW (17) — defense-in-depth / edge cases

| ID | Location | Issue |
|----|----------|-------|
| L1 | `combatants/types.rs:30-31` | `hp_max` / `temp_hp` lack `#[validate(range)]`. Master can set i32::MIN..MAX. DB CHECK blocks negatives; defense-in-depth only. |
| L2 | `spells/cast.rs:155-176` | Caster level `level_total` cast to i16 for `action_spell_level` — overflow possible for high custom-level chars. Frontend guards; no backend `try_into()`. |
| L3 | `actions/economy/opportunity.rs:38-46,48-112` | `opportunity_attack` validates reach + `modifiers.disengaged` but does NOT verify target is actually leaving reach. Client can fire OA on any in-reach target without movement event. Game-logic, not security — reaction still consumed. |
| L4 | `actions/economy/twf.rs:192-204` | `combatant_two_weapon_fights` publishes `hp_after`, `temp_hp_after` (M12 class). |
| L5 | `actions/economy/opportunity.rs:204-215` | Publishes `damage`, `instant_death` (M12 class). |
| L6 | `spells/apply.rs:236-252` | `combatant_casts_spell` per-target publishes `damage`, `hp_after`, `save_passed`, `concentration_breaks` (M12 class). |
| L7 | `tick.rs:275-285` | `combatant_takes_hazard_damage` publishes `hp_after`, `damage`, `damage_type` (M12 class). |
| L8 | `tick.rs:308-317` | `combatant_regenerates` publishes `hp_after` (M12 class). |
| L9 | `special/class_feature.rs:362-374` | Smite `slot_level` validated 1..=5 in code; DB doesn't restrict `slot_level` to 1-5. JSONB `slots` map arbitrary. |
| L10 | `encounters/initiative.rs:32-43` | `set_initiative` doesn't pre-validate combatant belongs to encounter; uses WHERE guard. Returns `NotFound` instead of `BadRequest` — cosmetic. |
| L11 | `encounters/start.rs:99-103` | Turn-start reset hard-codes first combatant only. If GM deletes mid-encounter and indices shift, reset may miss. |
| L12 | `special/shove.rs:111-128` | Shove mutates `token_x/token_y` of target without tx wrap. Concurrent shove+move could produce final value that's neither's intended. |
| L13 | `combat_engine/stats/compute.rs:25` | `poisoned` sets `attack_disadvantage` only. PHB: also ability-check dis. Add separate flag for skill-check dis. |
| L14 | `combat_engine/stats/compute.rs:22,289-291` | `restrained` calls `save_disadvantage_for("dex")` but body ignores `_ability`, sets global `save_disadvantage=true`. PHB: DEX-only dis. Make ability-specific. |
| L15 | `combat_engine/resolvers/attack.rs:78-80` | `frightened` attacker → blanket `dis = true` without LOS check on source. PHB p.290: "while the source of its fear is within line of sight." |
| L16 | `web/src/routes/campaigns/[id]/initiative/+page.svelte:1484-1517` | `checkOpportunityAttacks` only checks `oldDist <= reach`; doesn't verify `newDist > reach`. Small moves WITHIN reach still trigger prompt. |
| L17 | `combat_engine/resolvers/damage_type.rs:63-101` | Concentration check runs whenever `target.active_effects.any(\|e\| e.concentration)` — even at 0 damage. 0 damage → DC=10, may break concentration at random. Gate on `damage > 0`. |

---

## INFO (5) — documented quirks

| ID | Location | Note |
|----|----------|------|
| I1 | `encounters/read.rs:16` | Encounter fetched BEFORE `require_member`. Returns only public metadata (name/status/map_image/round). No PII. Existence-leak only (UUID unguessable). |
| I2 | `encounters/turns.rs:74-83` | Per-turn reset for next combatant, no reorder needed. If combatant was deleted, `new_idx` may point to empty `turn_order` slot — turn ticks wastefully but doesn't break. |
| I3 | `combat_engine/stats/compute.rs:210-212` | `evasion` requires single class entry ≥7 in Rogue OR Monk. Multiclass Rogue 6 / Monk 1 = total 7 = should grant Evasion per PHB multiclass. |
| I4 | `routes/combat/special/class_feature.rs:74-78` | `action_surge` unconditional `UPDATE action_used=false` without guard. Idempotent, no actual bug. |
| I5 | `tick.rs:37` | No background tick loop; effect ticking tied to explicit turn transition via HTTP. Master serializes via requests. OK. |

---

## OK Sections (verified clean)

- **RBAC**: all 62 routes call `require_master` / `require_member` / `require_action_auth` BEFORE any data mutation. Per-route audit in `SECURITY_AUDIT.md` HIGH-12.
- **SQL injection**: 100+ `sqlx::query*` calls all parameterized. `format!` (60+ uses) only builds dice expressions with i32 from server-computed stats, or user-facing strings. Never reaches SQL.
- **SQLx reborrow**: 100% of `fetch_optional/fetch_one/execute` on tx use `&mut *tx` reborrow. No moves detected. AGENTS.md §5.1 landmine avoided.
- **Action economy atomicity**: all 5 branches of `consume_action_or_bonus` use `UPDATE ... WHERE action_used=false RETURNING id` + `is_none()` check. All handlers in `actions/economy/`, `combatants/action.rs`, `special/{class_feature,escape,grapple,shove,multiattack,legendary}.rs`, `spells/apply.rs`, `actions/reactions.rs` atomic.
- **IDOR**: every combatant-scoped handler resolves `combatant_id → encounter → campaign → require_member` chain before any data write.
- **Shield reaction** (`actions/reactions.rs:51-112`): reads `pending_hits` JSONB queue, errors if empty. Hits only appended after `result.hit` check. Cannot shield a non-hit.
- **Counterspell** (`actions/reactions.rs:113-178`): reads `spell_being_cast` scoped to `encounter_id`. Auto-success at slot_level ≥ target spell level. Server-validated.
- **Uncanny Dodge** (`special/class_feature.rs:307-359`): reads `pending_hits`, falls back to legacy `last_hit_damage`. Reaction atomic. Half-damage applied to queue pop.
- **Token move** (`combatants/move_combatant.rs:114-140`): `SELECT FOR UPDATE` + tx serializes concurrent moves. Pessimistic lock correct.
- **Async shared state**: no `static mut`, no `RwLock<HashMap>` for encounter state. Combat state lives entirely in Postgres. DB is sole source of truth.
- **BA+action spell restriction** (`spells/apply.rs:40-52`): correct.
- **Known/prepared casters** (`spells/cast.rs:154-178`): correct per migration `20260616000002`.
- **Cantrip scaling** (`spells/cast.rs:230-253`): correct.
- **Spell components** (`spells/cast.rs:131-152`): V/S validated. M deferred (audit gap #33).
- **Spell save DC / attack bonus**: 8+prof+casting_mod, prof+casting_mod respectively.
- **Ritual casting** (`spells/cast.rs:117-119`): correct.
- **Temp HP** (`combatants/update.rs:85`): `case when $7 > temp_hp then $7 else temp_hp` — highest-wins. Correct.
- **Massive damage** (`attack.rs:364-366` + `damage.rs:39-40`): `target.hp_current > 0 && remaining_after_zero >= target.hp_max`. Correct.
- **R/V cancellation** (`combat_engine/resolvers/damage_type.rs:6-44`): correct.
- **Lay on Hands** (`class_feature.rs:200-306`): reads `sheet.resources` fuzzy name `like '%lay on hands%'`, validates same encounter, locked via `SELECT FOR UPDATE`, decrements pool, heals `min(pool, missing)`.
- **Conditions creature-type immunity** (`tactical/conditions.rs:73-82`): correct.
- **Timed conditions** (`tick.rs:14-34`): `name:N` tick down at turn start, removed at 1.
- **Incapacitating conditions break concentration** (`tactical/conditions.rs:153-157` + `spells/apply.rs:108-111`): correct.
- **Death saves** (`resolvers/death_save.rs:59-86`): nat 20 = +1 HP + reset; nat 1 = +2 failures; 3 successes = stable + reset. Correct (Sprint 12).
- **Heal 0 → >0** (`actions/combat/heal.rs:85-106`): resets death saves. Correct.
- **Multiattack parser** (`parse_multiattack.rs:24-152`): parses "2 claws + 1 bite" / "makes two attacks: one with its bite…" / fallback first-action. Correct.
- **Body limit**: `DefaultBodyLimit::max(512 * 1024)` on entire combat router. `bulk_add_combatants` 1-100 cap explicit. `set_initiative` 1-50 cap explicit.
- **Token revocation**: `extract.rs:27-37` checks `token_version` on every HTTP request; `ws.rs:250-252` checks on every WS upgrade. All combat handlers via `AuthUser` extractor.
- **WS connect rate limit**: 60/min/user, bounded map (`ws.rs:174-210`).

---

## Test Coverage Gaps (HIGH bugs without tests)

| Bug | Test missing |
|-----|-------------|
| H1 (multiattack index swap) | No test for `try_parse_npc_multiattack` path with 2+ parsed attacks against reordered targets |
| H2 (within-5ft threshold) | No test for paralyzed/unconscious auto-crit distance boundary |
| H3 (cover="full" dead branch) | No test for `cover="full"` (3+ blockers) attack resolution |
| H4 (spell range formula) | No test for `parse_spell_range_ft` × token distance cross-check |
| H5 (HP clamp) | No test for damage at 0 HP — verified HP goes negative |

---

## Recommended Fix Order

1. **C1, C2, C3, C4, H9** — atomicity / event-desync (5 files, low risk)
2. **H1** — multiattack data corruption (1 file, regression test critical)
3. **H2, H3, H4, H5, H6** — wrong-behavior bugs (5 files, regression tests critical)
4. **H7, H8, H11, H12** — turn-order / dedup (4 files)
5. **M12, M13** — visibility leaks (2 files)
6. **M1-M11** — PHB violations (batch by area: stats/, spells/, damage/)
7. **L1-L17** — defense-in-depth, edge cases
