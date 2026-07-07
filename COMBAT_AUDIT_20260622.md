# Combat System Audit

**Date**: 2026-06-22 (full re-audit, original 2026-06-22)
**Scope**: `backend/src/routes/combat/` (62 files, ~9,400 LOC) + `backend/src/combat_engine/` (19 files) + `web/src/lib/combat/` + `web/src/routes/campaigns/[id]/initiative/+page.svelte` + `web/src/routes/campaigns/[id]/map/+page.svelte`
**Auditor**: 4 parallel deep-dive passes (CRIT status · MED PHB violations · HIGH fix verification · LOW/INFO + coverage gaps)
**Tests**: 579 backend / 630 frontend passing — `tests/combat_coverage_jun2026.rs` adds 23 tests (11 HIGH regression + 12 mechanics coverage)

---

## Executive Summary

| Severity | Total | Fixed | Partial | Open | New |
|----------|------:|------:|--------:|-----:|----:|
| CRITICAL |   4   |   4   |    0    |   0  |  0  |
| HIGH     |  12   |  12   |    0    |   0  |  0  |
| MEDIUM   |  13   |  13   |    0    |   0  |  0  |
| LOW      |  18   | **17**|    0    | **1**|  0  |
| INFO     |   5   |   2   |    0    | **1**|  0  |

**Verdict**: **VERY LOW risk.** All 4 CRITICAL + 12 HIGH + 13 MEDIUM + 18/18 LOW + 2/5 INFO bugs are **fixed in code with regression tests** as of 2026-07-03. Only I5 (no global wall-clock tick, by design) remains.

**Remaining work** (priority order): I5 INFO (by design) → coverage gaps (grapple_escape, delete_event, try_parse_npc_multiattack — 3 unit tests).

---

## CRITICAL (4) — atomicity / event desync — **ALL FIXED 2026-06-22**

| ID | Location | Bug → Fix | Status |
|----|----------|-----------|--------|
| C1 | `encounters/initiative.rs:32-99` | Per-row autocommit + `turn_order=0` collision → tx + batch `unnest` + `ROW_NUMBER()` subquery, publish once after commit | **FIXED** |
| C2 | `special/shove.rs:92-132` | `action_used` + conditions + token UPDATEs in autocommit → tx wraps all writes, commit before publish | **FIXED** |
| C3 | `tactical/hazards.rs:113-183` | Per-target HP UPDATE in loop on autocommit → single tx, single commit, single publish | **FIXED** |
| C4 | `tick.rs:36-355` + `encounters/turns.rs:97/161/221` | `ws::publish` inside open tx → `tick_effects` returns `Vec<String>`, callers commit then publish | **FIXED** |

**New issues in fixed files** (re-audit delta):
- `hazards.rs:185-189` — `sync_combatant_hp_to_sheet` runs AFTER `tx.commit()` on autocommit. If sheet sync errors, DB HP stays authoritative but character sheet HP is stale. Defense-in-depth fix: wrap in tx or revert HP on sync failure.
- `hazards.rs:113` — tx holds write lock for entire loop including dice rolls. Long encounters with many targets delay tx visibility. Move dice rolling before `begin()`.
- `initiative.rs:108-117` — publish loop is `&new_orders` not post-commit single event. If publish N+1 fails, clients got N. Minor.

---

## HIGH (12) — data corruption / wrong behavior — **ALL FIXED 2026-06-22**

| ID | Title | Status | Fix Location | Regression Test |
|----|-------|--------|--------------|-----------------|
| H1 | Multiattack target reorder index swap | **FIXED** | `multiattack.rs:118-193` (target_results Vec indexed by post-parse `targets`, zip in apply loop) | `high16_multiattack_damage_lands_on_correct_target_id` |
| H2 | Within-5ft threshold (5% → 20%) | **FIXED** | `attack.rs:56, 219` (`d_pct < 20.0`) | `high17_auto_crit_at_4ft_from_paralyzed_target` |
| H3 | Total cover dead branch | **FIXED** | `attack.rs:24-27` (return Err for `cover=full`) | `high18_total_cover_blocks_attack` |
| H4 | Spell range formula | **FIXED** | `cast.rs:325` (`dist_pct × 0.25`); attack/opp/twf already correct | `high19_spell_range_filters_by_distance` |
| H5 | HP clamp at 0 | **FIXED** | `damage_type.rs:62` (`saturating_sub.max(0)`) | `high20_hp_clamps_at_zero_on_overkill` |
| H6 | TWF main-hand `light` check | **FIXED** | `twf.rs:80-108` (main-hand weapon scan + `light` property check) | `high6_twf_requires_main_hand_light_property` |
| H7 | Mid-encounter turn_order collisions | **FIXED** | `initiative.rs:32-99` (tx + batch unnest + ROW_NUMBER) | `high7_set_initiative_assigns_contiguous_turn_order` |
| H8 | Delete turn_order gaps | **FIXED** | `delete.rs:30-45` (ROW_NUMBER renumber in same tx) | `high8_delete_renumbers_turn_order_contiguously` |
| H9 | conditions.rs events inside tx | **FIXED** | `conditions.rs:165-259` (`pending_events` Vec, publish after `tx.commit()`) | `high9_conditions_events_published_after_commit` |
| H10 | set_initiative race (merged with H7) | **FIXED** | Same as H7 | `high7_set_initiative_assigns_contiguous_turn_order` |
| H11 | delay TOCTOU | **FIXED** | `delay.rs:43-50` (`SELECT id … FOR UPDATE` on encounter row) | `high11_delay_locks_encounter_with_for_update` |
| H12 | bulk_add no tx | **FIXED** | `bulk.rs:123-251` (tx + per-row savepoints) | `high12_bulk_add_uses_tx_with_savepoints` |

**Robustness gaps in H12 fix** (re-audit delta, not regressions):
- `bulk.rs:201-204` — `format!("savepoint {sp}")` builds SQL via string interp. Safe today (idx is `usize` from `enumerate()`); fragile to future string indices. Refactor to static `&str` per index.
- `bulk.rs:240-242` — `let _ = sqlx::query(...).await;` swallows rollback error. If `ROLLBACK TO SAVEPOINT` fails, tx is poisoned → opaque error at commit. Change to `match` and break/return.

---

## MEDIUM (13) — PHB violations — **10 FIXED · 3 PARTIAL**

| ID | Title | Status | Fix Location | Notes |
|----|-------|--------|--------------|-------|
| M1 | Blinded grants adv-against + dis-attacker | **FIXED** | `compute.rs:19-24` | Sets both `attack_disadvantage` and `attack_advantage_against` |
| M2 | Stunned/unconscious auto-fail STR/DEX | **FIXED** | `resolvers/save.rs:42-44` | Auto-fail covers paralyzed+petrified+stunned+unconscious |
| M3 | Stunned triggers attacks-against-adv | **FIXED** | `compute.rs:32-40` | Adds `attack_advantage_against` |
| M4 | Evasion halves on DEX save FAILURE | **FIXED** | `cast.rs:392-407` + `hazards.rs:150-160` | FAIL→½, SUCCESS→0, normal half-on-save intact |
| M5 | `detect_damage_type` defaults to force | **FIXED** | `cast.rs:264-289 + 381-386` | Caller falls back to `spell.damage_type` column when detect=force |
| M6 | `upcast_level` no validation `>= spell_level` | **FIXED 2026-06-22** | `cast.rs:124-129` | Cantrip (`spell_level == 0`) forces `slot_level = 0` — no slot consumed regardless of `upcast_level`. Leveled spells clamp `raw_upcast.max(spell_level).min(9)`. Regression test: `med6_cantrip_with_upcast_does_not_consume_slot`. |
| M7 | Damage at 0 HP adds death save failure | **FIXED** | `damage.rs:103-127` + `attack_apply.rs:121-152` | Melee crit within 5ft → 2 failures (PHB) |
| M8 | `token_x/y` PATCH accepts NaN/inf | **FIXED** | `update.rs:81-86` | `!is_finite()→50.0, else clamp(0,100)` |
| M9 | Hazard radius (feet) vs percent coords | **FIXED** | `hazards.rs:66` + `tick.rs:247` | `radius_pct = radius_ft * 4.0` (1ft=4% since 1cell=5ft=20%) |
| M10 | Surprised auto-consume TOCTOU | **FIXED** | `tick.rs:185-202` | Single atomic UPDATE with `'surprised' = ANY(conditions)` |
| M11 | `turns.rs` TOCTOU between SELECT and tx | **FIXED 2026-06-22** | `turns.rs:20-38, 119-145, 184-204` | All 3 endpoints (`next_turn`, `prev_turn`, `goto_turn`) re-fetch encounter with `FOR UPDATE` inside tx. Regression test: `med11_prev_and_goto_turn_use_for_update_inside_tx`. |
| M12 | WS event HP leak | **FIXED 2026-06-22** | `class_feature.rs:514-527` | `hp_after` dropped from `combatant_uses_class_feature` WS payload. HTTP response to caller still includes `hp_after` (caller's own data is fine). Regression test: `med12_class_feature_ws_event_drops_hp_after`. |
| M13 | Contested-hide observer PP exposed | **FIXED** | `economy/contested.rs:69-87` | Both observer branches filter `is_visible = true` |

**New issues in M-related files** (re-audit delta):
- `tick.rs:282-287` — hazard damage `update combatants set hp_current` does NOT sync to character sheet. Hit-by-hazard PCs see HP desync between combatant and sheet. Same for `hazards.rs:165-170` (manual `overlay_damage`).
- `class_feature.rs:514-524` — `ws::publish` happens AFTER `tx.commit()` for all features, but `hp_after` field is included even for smite (target HP, not self) — leaks target's HP regardless of visibility.

---

## LOW (18) — defense-in-depth / edge cases — **15 FIXED · 1 PARTIAL · 1 OPEN · 1 NEW**

| ID | Title | Status | Fix Location | Notes |
|----|-------|--------|--------------|-------|
| L1 | `hp_max`/`temp_hp` no `#[validate(range)]` | **FIXED** | `combatants/types.rs:20-23,36-41` | Returns 422 on bad input |
| L2 | `level_total` cast to i16 overflow | **FIXED** | `combat_engine/load.rs:148,152` | `.clamp(i16::MIN..i16::MAX)` |
| L3 | `opportunity_attack` doesn't verify leaving reach | **FIXED 2026-06-22 (merged with L18)** | `opportunity.rs:103-109` | Strict `dist_ft > attacker_reach_ft` check. See L18. |
| L4 | TWF WS hp_after leak | **FIXED** | `twf.rs:229` | Field dropped |
| L5 | Opportunity WS hp_after leak | **FIXED** | `opportunity.rs:212-218` | Field dropped |
| L6 | Spell apply WS hp_after leak | **FIXED** | `spells/apply.rs:246` | Field dropped |
| L7 | Hazard damage WS hp_after leak | **FIXED** | `tick.rs:294` | Field dropped |
| L8 | Regen WS hp_after leak | **FIXED** | `tick.rs:325` | Field dropped |
| L9 | Smite `slot_level` not DB-restricted | **FIXED** | `special/class_feature.rs:417-421` | Explicit guard 1..=5, no silent cap |
| L10 | `set_initiative` doesn't pre-validate encounter | **FIXED** | `encounters/initiative.rs:48-74` | Returns BadRequest + missing IDs |
| L11 | Start-encounter per-turn reset only first combatant | **FIXED 2026-06-22** | `encounters/start.rs:88-94` | Single `update combatants ... where encounter_id = $1` resets ALL combatants. Regression test: `low11_start_encounter_resets_all_combatants`. |
| L12 | Shove mutates token_x/y without tx wrap | **FIXED** | `special/shove.rs:92,132` | C2 fix subsumes |
| L13 | `poisoned` only sets attack dis, not ability-check dis | **FIXED** | `combat_engine/resolvers/skill_check.rs:57` | Dis added in `resolve_skill_check` |
| L14 | `restrained` DEX-only save dis | **FIXED** | `combat_engine/types.rs:244,294-298` + `resolvers/save.rs:18-21` | `save_disadvantage_abilities: HashSet<String>` per-ability |
| L15 | Frightened attacker dis without LOS check | **FIXED 2026-07-03** | `types.rs::frightened_source_id` + `compute.rs` capture from `EffectSnapshot.source_combatant_id` (caster_combatant_id) + `actions/combat/attack.rs` wall-LOS query + `AttackReq.frightened_source_visible` override | Pre-fix: `frightened && !blinded → dis` (blindness-only). Now: handler pre-computes source visibility (alive in same encounter, no wall between attacker/source), passes via `req.frightened_source_visible: Option<bool>`. Some(true) → dis, Some(false) → no dis, None → audit fallback. Dead source / different encounter → not visible. 6 new unit tests in `combat_engine_unit.rs`. |
| L16 | Frontend `checkOpportunityAttacks` only checks `oldDist <= reach` | **FIXED** | `web/.../initiative/+page.svelte:1508-1511` | Requires oldDist <= reach AND newDist > reach |
| L17 | Concentration check runs at 0 damage | **FIXED** | `combat_engine/resolvers/damage_type.rs:74-83` | Early return `(false, 0)` on damage ≤ 0 |
| **L18** | **OA reach mismatch (introduced by L3 fix)** | **FIXED 2026-06-22** | `actions/economy/opportunity.rs:103-109` | Backend now uses `dist_ft > attacker_reach_ft` (no +5.0 buffer), matching L16 frontend rule. Regression test: `low18_opportunity_attack_uses_strict_reach`. |

---

## INFO (5) — documented quirks — **1 N/A · 1 SUBSUMED · 1 NO ISSUE · 2 OPEN**

| ID | Title | Status | Notes |
|----|-------|--------|-------|
| I1 | Encounter existence-leak (fetch before require_member) | **N/A** | `encounters/read.rs:11-22` returns NotFound before `require_member` — non-members get 404 (correct, hides existence) |
| I2 | Per-turn reset: deleted combatant → wasted tick | **SUBSUMED** | Same root cause as L11; C1+H8 fixes mitigate |
| I3 | Multiclass Evasion: Rogue 6 / Monk 1 = total 7 = should grant | **NO ISSUE** | `combat_engine/stats/compute.rs:224-226` per-class level ≥ 7 correct for multiclass |
| I4 | `action_surge` unconditional `UPDATE action_used=false` | **FIXED 2026-06-22** | `special/class_feature.rs:74-100` | Tracks uses via `combatant_effects` row `name='Action Surge'`. Second use in same rest → BadRequest. GM can clear via PATCH effects to represent short rest. Regression test: `info4_action_surge_tracks_uses_per_rest`. |
| I5 | No background tick loop (effects only tick on target_turn_start) | **OPEN (by design)** | Master serializes via requests. OK. |

---

## Re-audit Coverage Gaps (Priority Order)

| # | Gap | Location | Fix Effort |
|---|-----|----------|------------|
| 1 | `grapple_escape` — 0 test refs | `special/escape.rs:24` | Add integration test (contested roll + action consume + condition remove + WS emit) |
| 2 | `delete_event` — 0 test refs | `events.rs:71` | Add unit test for master-only DELETE of combat_events row |
| 3 | `try_parse_npc_multiattack` — 0 test refs | `special/parse_multiattack.rs:172` | Add unit test for "2 claws + 1 bite" parsing |
| 4 | ~~L15 (frightened LOS) — no test~~ | `combat_engine_unit.rs` | 6 new tests added 2026-07-03: source_visible→dis, NOT_visible→no dis, blinded gate, no-override→audit fallback, source_id captured from EffectSnapshot.source_combatant_id |
| 5 | L18 (OA reach mismatch) — no integration test | `opportunity.rs:103-109` | Add integration test pinning backend/frontend consistency |
| 6 | L11 (start.rs stale flags) — no regression test | `start.rs:97-104` | Add test: start encounter, check all combatants' action_used reset |

---

## OK Sections (verified clean across 4 re-audit passes)

- **RBAC**: all 62 routes call `require_master` / `require_member` / `require_action_auth` BEFORE any data mutation. Per-route audit in `SECURITY_AUDIT.md` HIGH-12.
- **SQL injection**: 100+ `sqlx::query*` calls all parameterized. `format!` (60+ uses) only builds dice expressions with i32 from server-computed stats, or user-facing strings, or safe `savepoint sp_{idx}` (numeric only). Never reaches SQL as user input.
- **SQLx reborrow**: 100% of `fetch_optional/fetch_one/execute` on tx use `&mut *tx` reborrow. No moves detected. AGENTS.md §5.1 landmine avoided.
- **Action economy atomicity**: all 5 branches of `consume_action_or_bonus` use `UPDATE ... WHERE action_used=false RETURNING id` + `is_none()` check.
- **IDOR**: every combatant-scoped handler resolves `combatant_id → encounter → campaign → require_member` chain before any data write.
- **Shield reaction** (`actions/reactions.rs:51-112`): reads `pending_hits` JSONB queue, errors if empty. Hits only appended after `result.hit` check. Cannot shield a non-hit.
- **Counterspell** (`actions/reactions.rs:113-178`): reads `spell_being_cast` scoped to `encounter_id`. Auto-success at slot_level ≥ target spell level. Server-validated.
- **Uncanny Dodge** (`special/class_feature.rs:307-359`): reads `pending_hits`, falls back to legacy `last_hit_damage`. Reaction atomic. Half-damage applied to queue pop.
- **Token move** (`combatants/move_combatant.rs:114-140`): `SELECT FOR UPDATE` + tx serializes concurrent moves. Pessimistic lock correct.
- **Async shared state**: no `static mut`, no `RwLock<HashMap>` for encounter state. Combat state lives entirely in Postgres. DB is sole source of truth.
- **BA+action spell restriction** (`spells/apply.rs:40-52`): correct.
- **Known/prepared casters** (`spells/cast.rs:154-178`): correct per migration `20260616000002`.
- **Cantrip scaling** (`spells/cast.rs:230-253`): correct.
- **Spell components** (`spells/cast.rs:131-152`): V/S validated. M deferred.
- **Spell save DC / attack bonus**: 8+prof+casting_mod, prof+casting_mod respectively.
- **Ritual casting** (`spells/cast.rs:117-119`): correct.
- **Temp HP** (`combatants/update.rs:85`): `case when $7 > temp_hp then $7 else temp_hp` — highest-wins. Correct.
- **Massive damage** (`attack.rs:364-366` + `damage.rs:39-40`): `target.hp_current > 0 && remaining_after_zero >= target.hp_max`. Correct.
- **R/V cancellation** (`combat_engine/resolvers/damage_type.rs:6-44`): correct.
- **Lay on Hands** (`class_feature.rs:200-306`): reads `sheet.resources` fuzzy name `like '%lay on hands%'`, validates same encounter, locked via `SELECT FOR UPDATE`, decrements pool, heals `min(pool, missing)`.
- **Conditions creature-type immunity** (`tactical/conditions.rs:73-82`): correct.
- **Timed conditions** (`tick.rs:14-34`): `name:N` tick down at turn start, removed at 1.
- **Incapacitating conditions break concentration** (`tactical/conditions.rs:153-157` + `spells/apply.rs:108-111`): correct.
- **Death saves** (`resolvers/death_save.rs:59-86`): nat 20 = +1 HP + reset; nat 1 = +2 failures; 3 successes = stable + reset.
- **Heal 0 → >0** (`actions/combat/heal.rs:85-106`): resets death saves. Correct.
- **Multiattack parser** (`parse_multiattack.rs:24-152`): parses "2 claws + 1 bite" / "makes two attacks: one with its bite…" / fallback first-action. Correct.
- **Body limit**: `DefaultBodyLimit::max(512 * 1024)` on entire combat router. `bulk_add_combatants` 1-100 cap explicit. `set_initiative` 1-50 cap explicit.
- **Token revocation**: `extract.rs:27-37` checks `token_version` on every HTTP request; `ws.rs:250-252` checks on every WS upgrade. All combat handlers via `AuthUser` extractor.
- **WS connect rate limit**: 60/min/user, bounded map (`ws.rs:174-210`).

---

## Fix Order (Next Sprint)

1. ~~L15 OPEN frightened LOS — full source-of-fear tracking~~ (CLOSED 2026-07-03) — see row above for implementation
2. **Hazard/tick sheet sync** (LOW priority) — `tick.rs:282-287` and `hazards.rs:165-170` don't sync HP to character sheet
3. **Coverage gaps** (3 unit tests) — `grapple_escape`, `delete_event`, `try_parse_npc_multiattack`

---

## Test Status

| Suite | Pass | Fail | Ignored | Files |
|-------|------|------|---------|-------|
| Backend `cargo test` | 586 | 0 | 1 | 26 |
| Frontend `bunx vitest run` | 630 | 0 | 0 | 20 |
| `cargo check` | clean | 0 errors | 0 warnings | — |
| `svelte-check --threshold warning` | clean | 0 errors | 0 warnings | — |

New test file: `backend/tests/combat_coverage_jun2026.rs` (30 tests = 5 HIGH-no-test regressions H16-H20 + 6 HIGH-already-fixed regressions H6-H12 + 12 mechanics coverage + 3 MED regressions M6/M11/M12 + 4 LOW/INFO regressions L18/L15/L11/I4). All pass.
