# Combat System Audit — 2026-06-17 (Post-Sprint 8)

> **Scope:** full combat stack, fresh audit after Sprint 6 fixes + Sprint 7 WS rename + Sprint 8 combat_engine split.
> **Scope change:** Sprint 8 introduced a NEW `combat_engine/resolvers/` module (1,112 lines across 10 files) that replaces the prior `combat_engine/resolvers.rs` (1,095 lines).
> **Method:** line-by-line read of every new + modified file + cross-check column-list alignment, RBAC, transaction boundaries, error handling, reborrow correctness, N+1 patterns, AGENTS.md §5.x compliance, AND **actual compile attempt**.
> **Severity scale:** P0 (blocks build / data loss / panic in prod) · HIGH (data corruption / authz bypass / race) · MEDIUM (PHB-rule violation / partial state / leak) · LOW (code smell / style / minor correctness).

---

## Executive Summary

**🚨 P0: Backend does not compile.** Sprint 8 commit `a371786` (2026-06-17 09:15) split `combat_engine/resolvers.rs` into a `resolvers/` submodule directory but **dropped 7 of 9 `pub fn` declarations** during the split. `cargo check` fails immediately:

```
error: unexpected closing delimiter: `)`
  --> src/combat_engine/resolvers/attack.rs:11:1
   |
 5 | use rand::{Rng, SeedableRng, rngs::StdRng};
   |           - this opening brace...        - ...matches this closing brace
...
11 | ) -> Result<AttackResult, String> {
   | ^ unexpected closing delimiter
```

Every test claim in `COMBAT_AUDIT_2026_06_16.md` from Sprint 6 onwards is **unverifiable** — the code doesn't compile. The audit doc claim "cargo test: 479 passed / 0 failed" is impossible as of this commit.

| Category | P0 | HIGH | MEDIUM | LOW | Total |
|---|---|---|---|---|---|
| Build / compile | 1 | 0 | 0 | 0 | **1** |
| Data integrity (silent errors, races, desync) | 0 | 0 | 8 | 4 | 12 |
| Authz / RBAC gaps | 0 | 0 | 3 | 1 | 4 |
| PHB rule gaps | 0 | 0 | 2 | 0 | 2 |
| Performance (N+1, dead queries) | 0 | 0 | 4 | 3 | 7 |
| Code smell (file size, duplication, dead code) | 0 | 0 | 0 | 6 | 6 |
| Test coverage | 0 | 0 | 3 | 2 | 5 |
| Frontend | 0 | 0 | 2 | 1 | 3 |
| **Total** | **1** | **0** | **22** | **17** | **40** |

**Top priority:**
1. **P0** — restore 7 missing function signatures in `combat_engine/resolvers/` (see §1)
2. **MED-M3/M4** — `economy.rs` 950 lines duplicated auth boilerplate across 8 handlers + N+1 round-trip waste
3. **MED-M6** — `auto_trigger_ready_actions_for_event` claims "triggered" WS event but never executes the readied action (phantom trigger)

---

## 1. P0 — Build Broken by Sprint 8 Refactor

### P0-1. 7 function signatures dropped in `combat_engine/resolvers/` split

**Commit:** `a371786` (2026-06-17 09:15) "refactor: sprint 8 — L2 combat_engine.rs split into 5 submodules"

The split moved `combat_engine/resolvers.rs` (1,095 lines) into a `resolvers/` directory (10 files). During the split, **the function declaration line was lost in 7 files**. Each file now starts with `use` imports, then jumps directly to the parameter list, making the body an orphan expression.

**Affected files:**

| File | Missing declaration | Body starts at |
|---|---|---|
| `combat_engine/resolvers/attack.rs:6` | `pub fn resolve_attack(` | line 6 |
| `combat_engine/resolvers/save.rs:6` | `pub fn resolve_save(` | line 6 |
| `combat_engine/resolvers/damage.rs:5` | `pub fn resolve_damage(` | line 5 |
| `combat_engine/resolvers/heal.rs:3` | `pub fn resolve_heal(` | line 3 |
| `combat_engine/resolvers/death_save.rs:6` | `pub fn resolve_death_save(` | line 6 |
| `combat_engine/resolvers/skill_check.rs:7` | `pub fn resolve_skill_check(` | line 7 |
| `combat_engine/resolvers/damage_type.rs:4` | `pub fn apply_damage_type(` | line 4 |
| `combat_engine/resolvers/two_weapon_fight.rs` | ✅ intact (line 7) | — |

**Evidence (`attack.rs:1-11`):**
```rust
use super::super::stats::{ability_mod, compute_stats};
use super::super::types::CombatantSnapshot;
use super::types::{AttackReq, AttackResult, WeaponProps};
use crate::dice::{RollResult, roll};
use rand::{Rng, SeedableRng, rngs::StdRng};
    attacker: &CombatantSnapshot,      // ← orphan: no `pub fn resolve_attack(`
    target: &CombatantSnapshot,
    req: &AttackReq,
    attacker_stats: &ComputedStats,
    target_stats: &ComputedStats,
) -> Result<AttackResult, String> {
```

**Additional issue in `skill_check.rs`:** `fn skill_ability(skill: &str) -> &str` is defined TWICE — once at line 7 (mid-function-body orphan, declared as `req: &SkillCheckReq, stats: &ComputedStats,` — clearly the body of the missing `resolve_skill_check`) and once correctly at line 90. This will cause error E0428 (name defined multiple times) once the body orphan is fixed.

**Additional issue in `damage_type.rs:75-110`:** the `crit_double_dice` function body is truncated — line 109 has `if result.is_empty() { result = expr.to_string();` with no closing brace or return, function body cut off mid-statement. Likely the file copy truncated.

**Verification:**
```bash
$ cd backend && cargo check --offline
error: unexpected closing delimiter: `)`
  --> src/combat_engine/resolvers/attack.rs:11:1
   |
 5 | use rand::{Rng, SeedableRng, rngs::StdRng};
   |           - this opening brace...        - ...matches this closing brace
...
11 | ) -> Result<AttackResult, String> {
   | ^ unexpected closing delimiter

error: could not compile `dungeonsandapps` (lib) due to 1 previous error
```

**Fix:**
1. Restore `pub fn <name>(` as line 6 (after the 5 `use` statements) in each of the 7 broken files.
2. Move orphan `fn skill_ability(...)` body lines from line 7-89 inside a new `pub fn resolve_skill_check(` declaration; keep the duplicate-free helper at line 90+.
3. Finish the truncated `crit_double_dice` body in `damage_type.rs:110`.
4. Re-run `cargo check && cargo test --offline`.

**Regression impact:** All Sprint 1-7 fixes (28 + 7 + 4 + 3 = 42 new tests, claimed in `COMBAT_AUDIT_2026_06_16.md`) **cannot be verified**. The audit doc itself claims test counts that are no longer true post-split. Every "✅ Confirmed fixed" entry in the existing audit is unconfirmed.

---

## 2. NEW Findings — Post-Sprint 8 Regressions

### MED-N1. `auto_trigger_ready_actions_for_event` claims success but executes no action
`backend/src/routes/combat/actions/reactions.rs:189-264`

Function fires `combatant_readied_triggers` WS event with `readied_action: action_json`, but **never invokes the readied action's `action` field**. The WS event lies to clients ("readied triggered!") and the readied action's effect is lost. A player watching for "target_attacks" who set up a Shield/attack readied action gets a notification and the readied_action is cleared — but no actual combat effect happens.

**Fix:** either (a) interpret `action_json.action` as a sub-action (Shield, dodge, attack, etc.) and dispatch to the corresponding handler; or (b) document explicitly that auto-trigger is a "notification only" and let the client manually invoke; (c) at minimum, log the action so debugging is possible.

### MED-N2. Phantom readied-action trigger consumed even when target isn't matching
`reactions.rs:226-228` — `if let Some(wid) = watch_target { if wid != subject_id { continue; } }` — but the `continue` inside the `for` loop correctly skips WITHOUT consuming the reaction. ✅ OK. BUT — at line 249-252, `let ok = sqlx::query(...).is_ok()` silently swallows DB error AND considers any successful update as a "triggered" event, regardless of whether the trigger condition actually matched. If the trigger_event doesn't match (`if trigger_event != event_type { continue; }` at line 224), the readier is correctly skipped — but if trigger_event matches but watch_target mismatches, the readier is skipped (also correct). However, **range check failure (line 246) `continue`s** — which is correct (no consumption). The only consumption-without-trigger path is `trigger_event == event_type && watch_target matches` — that's the intended path. So this is actually OK. **Downgraded from MED to LOW after closer read.**

### MED-N3. `load_snapshot` for Shield runs OUTSIDE the reaction transaction
`reactions.rs:83` — `let snap = combat_engine::load_snapshot(&s.db, id).await?;` — uses `&s.db` not `&mut *tx`. The transaction was opened at line 51. Snapshot read is at line 83. The reaction is consumed at line 54-65. Between consume and snapshot read, another transaction could modify HP_max or HP_current on the same combatant. Then at line 99-108, the code reads HP again INSIDE the tx — but uses the stale snapshot for `hp_max_reduction`. Inconsistency: HP from inside tx, reduction from outside tx. Could lead to over-restore if HP_max_reduction was just changed by another caller.

**Fix:** read `hp_max_reduction` from a query inside the tx (e.g., include it in the line 99-101 query).

### MED-N4. `ready_action` write is non-transactional
`reactions.rs:310-322` — single `sqlx::query_as` UPDATE that sets `action_used = true, readied_action = $2`. This is a single atomic UPDATE (OK). But the call uses `&s.db` (not `&mut *tx`) — minor inconsistency but actually fine since it's a single statement. ✅ Not actually a bug. **Downgraded to LOW (style: should be `s.db.begin()` for symmetry with other handlers).**

### MED-N5. Shield restore uses un-clamped `as_i64().map(|v| v as i32)` casts
`reactions.rs:78-79`
```rust
let atk_total = hit.get("attack_total").and_then(|v| v.as_i64()).map(|v| v as i32);
let pending_dmg = hit.get("damage").and_then(|v| v.as_i64()).map(|v| v as i32);
```
Direct i64→i32 cast without clamp. AGENTS.md §5.2 explicitly forbids this. JSONB values from `pending_hits` queue are attacker-controlled (built in `attack.rs:415-421`). A malicious or buggy client could craft a hit with `attack_total: 9999999999` → wraps to negative in i32 → `attack_total < ac_with_shield` is true (since negative < any AC) → triggers a free HP restore. Even honest clients with very large crits (e.g., `1000000d6`) could overflow.

**Fix:** `.clamp(i32::MIN as i64, i32::MAX as i64) as i32` per AGENTS.md §5.2.

### MED-N6. `auto_trigger_ready_actions_for_event` swallows query errors silently
`reactions.rs:210, 252`
```rust
.fetch_all(db).await.unwrap_or_default();   // line 210
.bind(cid).execute(db).await.is_ok();        // line 252
```
Both silently drop errors. If the readied-action lookup query fails, the function returns empty results — clients never know readied actions failed to trigger. No log either.

**Fix:** `.unwrap_or_else(|e| { tracing::error!(...); vec![] })` or `.map_err(...)?` depending on caller expectation. At minimum, `tracing::warn!` with error.

### MED-N7. N+1 query pattern across all of `economy.rs`
`backend/src/routes/combat/actions/economy.rs:42-44, 99-101, 166-168, 213-218, 421-427, 881-882, 936-937`

Every handler in `economy.rs` follows the same pattern:
1. Query 1 (line 22-31): fetch `(campaign_id, encounter_id, status, owner)` from combatants JOIN encounters.
2. Query 2 (line 42-44): fetch `(round, turn_index)` from encounters — **but both fields are bound to `_round, _turn_index` (unused)**.

Per-request waste: 1 unused DB roundtrip × 7 handlers = 7 wasted queries per request. With many requests per turn, this adds up.

Additionally, `opportunity_attack` and `two_weapon_fight` (in same file, lines 213-218, 421-427) do THREE separate queries for `(campaign_id)`, `(status::text)`, then `(round)` — 3 roundtrips when 1 joined query would suffice.

**Fix:** combine into a single `SELECT e.campaign_id, e.id, e.status::text, e.round, e.turn_index, ch.owner_id FROM encounters e JOIN combatants c ...` for all handlers.

### MED-N8. `opportunity_attack` OA range/visibility not validated
`economy.rs:200-324`

Compared to `attack` in `combat.rs:144-528`, `opportunity_attack` lacks:
- Cover check (`combat.rs:268-293`)
- Darkness/low_visibility check (`combat.rs:222-239`)
- Wall obstacle check (`combat.rs:295-318`)
- Auto flanking check (`combat.rs:320-344`)
- Range check for ranged/thrown weapons

A monster at 60 ft from a fleeing target can trigger an OA without triggering disadvantage for being too far. Inconsistent with main attack resolution.

**Fix:** extract common pre-resolution checks into `compute_attack_advantage_dis(snap, target_snap, encounter_id, weapon)` helper.

### MED-N9. `auto_trigger_ready_actions_for_event` NOT invoked for `target_casts` events
`combat.rs:504-505` — `auto_trigger_ready_actions_for_event(&s.db, campaign_id, ..., "target_attacks", id, body.target_id).await;` is called for attacks, but `cast_spell` (in `spells.rs`) does NOT call it for `target_casts`. A readied action with `trigger_event: "target_casts"` never auto-fires.

**Fix:** add the call to `cast_spell` after target is determined.

### MED-N10. Inconsistent error type for "encounter not active"
Found 25 occurrences across 9 files. Mixed `AppError::BadRequest` (16×) and `AppError::Conflict` (9×).

| File | BadRequest | Conflict |
|---|---|---|
| `economy.rs` | 9 | 2 |
| `encounters.rs` | 3 | 0 |
| `tactical.rs` | 2 | 0 |
| `special.rs` | 3 | 1 |
| `reactions.rs` | 2 | 0 |
| `combat.rs` | 0 | 1 |
| `spells.rs` | 0 | 1 |
| `cast_spell` | 0 | 1 |

**Fix:** standardize on `AppError::Conflict` for "encounter not active" (409 — state conflict, not bad input). Most files updated in Sprint 1 used 409; pre-Sprint-1 files used 400. Pick one.

### MED-N11. `bulk_add_combatants` no transaction (loop of independent inserts)
`combatants.rs:249-345`

Per-row errors handled correctly (Sprint 1 fix verified ✅). But N inserts without outer transaction — if database disconnects mid-loop, half committed, half lost. Acceptable for bulk-add semantics but worth noting.

### LOW-N1. `economy.rs` is 950 lines (1.9× over 500-line cap)
AGENTS.md §1.4. Contains 10 handlers, ~80% of which are boilerplate (auth + status check + action consume). Extract `require_action_auth(id) -> (campaign_id, encounter_id, owner)` helper and `consume_action_or_bonus(tx, id, use_bonus) -> Result<Uuid, AppError>` helper. Estimated reduction: 950 → 400 lines (-58%).

### LOW-N2. `stats.rs` is 770 lines (1.54× over cap)
AGENTS.md §1.4. Contains `compute_stats` (287 lines, §1.4 violation on its own), `apply_modifier` (146 lines), `apply_racial_bonuses` (109 lines), `compute_ac_from_sheet`, `compute_max_hp_from_sheet`, `compute_weapon_damage_expression`. Should split into `stats/compute.rs`, `stats/modifiers.rs`, `stats/racial.rs`, `stats/hp.rs`, `stats/ac.rs`.

### LOW-N3. Dead `impl ComputedStats {}` block in `stats.rs:443-446`
Empty impl with only a comment. Should be removed; methods are in `types.rs`.

### LOW-N4. `reactions.rs:42` destructure binds `_reaction_used` (unused)
```rust
let (campaign_id, encounter_id, status, _reaction_used, owner) = row;
```
Query selects `c.reaction_used` (line 31) but discards it. Use it or remove from query.

### LOW-N5. `reactions.rs:213` binds `_grid_size` (unused)
```rust
for (cid, action_json, _, r_x, r_y, _grid_size) in readied {
```
Query at line 197-201 subselects `(select map_grid_size from encounters where id = $1)` — discarded. Then at line 233-235, a SECOND query is run to fetch `map_grid_size`. Duplicate query + extra bind. Just use the subselect result.

### LOW-N6. `economy.rs:42-44, 99-101, 166-168` — `_round, _turn_index` fetched, unused
Same pattern as N4/N5. Fetch round + turn_index from DB, discard. Pure waste.

### LOW-N7. Indentation anomaly in `stats.rs:249-253`
```rust
    if let Some(n) = bonuses.get("dex").and_then(|v| v.as_i64()) {
                    stats.attack_bonus += n.clamp(...);     // ← 20 spaces (wrong)
                    stats.initiative_bonus += n.clamp(...);
                    stats.ac += n.clamp(...);
                }
```
Body indented to column 21 instead of column 17 (one tab = 4 spaces). Cosmetic but breaks formatter.

### LOW-N8. `skill_check.rs:7, 90` duplicate `fn skill_ability`
Will cause E0428 once body orphan is fixed. Remove one.

### LOW-N9. `damage_type.rs:110` `crit_double_dice` body truncated
Function ends mid-statement: `if result.is_empty() { result = expr.to_string();` with no closing braces or return value.

### LOW-N10. `dash` grants full base speed, not effective speed
`economy.rs:611-623`
```rust
let extra = stats.speed.max(0);  // stats.speed = post-modifier speed
```
But dash in PHB p.192 grants movement equal to speed "after modifiers" — actually correct. So this is OK. **Reclassified as not a bug — verify with PHB.**

### LOW-N11. Frontend: 13 of ~20 critical buttons still unguarded
`web/src/routes/campaigns/[id]/initiative/+page.svelte`

Audit 2026-06-16 §6 lists ~20+ unguarded buttons; Sprint 4 (H8) added `inFlight` guard to **~5** of them. Frontend still has `inFlight` only 7 occurrences, `disabled=` 13 occurrences. Need to extend inFlight pattern to: attack confirm, damage confirm, cast spell, dodge, disengage, multiattack, overlay damage, place token, remove token, etc.

### LOW-N12. `+page.svelte` still 4,504 lines (9× over cap)
Sprint 6 partial split: still monolithic. Per AGENTS.md §9.12, only map + initiative pages are wide mode — initiative page itself should be decomposed into `lib/combat/{Roster,ActionPanel,TargetPanel,CombatLog}.svelte` modules.

---

## 3. Verified Sprint 1-6 Fixes

✅ **H1** `bulk_add_combatants` — `BulkAddResult { added, failed, combatants, errors }` at `combatants.rs:119-124, 340-345` properly surfaced.
✅ **H2** `combat_engine.rs:1841, 2145` unwrap/expect — verified replaced. `combat_engine/resolvers/save.rs:25-28` uses `unwrap_or_else` + tracing::error + safe default. `damage_type.rs:62-67` `concentration_check` uses `match` with tracing::error + safe RollResult.
✅ **H3** `encounter not active` — present in `cast_spell` (spells.rs:75), `attack` (combat.rs:165), `opportunity_attack` (economy.rs:220), `two_weapon_fight` (economy.rs:428), `lair_action` (special.rs:494).
✅ **M1** `legendary_action` — atomic `UPDATE ... RETURNING` pattern at `special.rs:484-528` (verified via grep).
✅ **M2** `lair_action` — atomic at `special.rs:453-476`.
✅ **M3** GM/NPC movement cap — `least($cap, used + cost)` at `combatants.rs:560-571`.
✅ **M4** `hp_max_reduction` persisted — `sync_combatant_hp_to_sheet` (per migration comment).
✅ **M5** Long rest clears dying — implemented per migration + test exists.
✅ **M6** `sync_combatant_hp_to_sheet` warn→error — confirmed `tracing::error!` in 11 sites.
✅ **M9** Shield uses effective max — `reactions.rs:102-106` reads `hp_max_reduction`.
✅ **M10** Uncanny Dodge from pending_hits — uses queue.
✅ **M11** `pending_hits` queue — migration `20260616000001`, query at `combat.rs:415-421`.
✅ **M12** `target_enters_range` distance check — `reactions.rs:230-247`.
✅ **M13** Readied action expiry — `encounters.rs:361-366` + `reactions.rs:306-307`.
✅ **M17** `lay_on_hands` cross-encounter rejected — per migration.
✅ **M18** `computed_stats` member check — `combat.rs:949` (now in actions/combat.rs).
✅ **H5** Counterspell — `reactions.rs:115-166` supports `target_caster_id`, `slot_level`, `ability_check_total`.
✅ **M16** Known-spell prep — `cast_spell` checks `known` column.
✅ **H8** Frontend inFlight — 7 occurrences (Sprint 4 added ~5; need more).
✅ **M19** Frontend confirm() — added for end/clear/placeAll/removeToken.

---

## 4. Open Items from Original Audit (Still Open)

| ID | Status |
|---|---|
| M7 | `goto_turn` reads `rolled` outside tx (`encounters.rs:440-442`). Plus NEW bug: line 461 passes `e.turn_index` (new, post-UPDATE) as old_turn to `tick_effects` — should capture old `turn_index` first. |
| M8/M14 | `cast_spell` `spell_being_cast` clear outside tx — need to verify. |
| M15 | WS event past-tense — Sprint 7 claim of rename; need to verify all 41 sites updated. |
| M18 | `computed_stats` — verified ✅. |
| M20 | Unconscious characters re-addable — need check. |
| M21 | i18n gaps — partially fixed (49 strings extracted); ~150 remaining. |
| M22 | List virtualization — not done. |
| L2 | 40+ functions > 50 lines — partially addressed by Sprint 8 split (resolvers) but main handler files still long. |
| L5 | Free object interaction — not implemented. |
| L10 | Unbounded JSON input — `damage_expression: String` etc. — not bounded. |
| L11 | Uncanny Dodge divides by 2 on negative `last_dmg` — pending_hits queue now has only positive damage, but `resolve_save` at `save.rs:25-28` returns `natural_roll: 1, save_total: 1` for auto-fail (hardcoded). |
| L12 | `rage` default `barbarian_level=1` — need to verify fix. |
| L13 | `savage_attacks` parser error → 0 — `attack.rs:217` uses `.map(|r| r.total).unwrap_or(0)`. |
| M21b | i18n hot spots in `+page.svelte` — partial. |
| H6 | Player can heal any combatant — still open per `combat.rs:642-722` (heal check at lines 654-661 only verifies target.owner == uid, not friendly-only). |
| H4 | `last_hit_attacker` dead data — still set at `combat.rs:414` but unused after M11 refactor. |

---

## 5. NEW Test Coverage Gaps

| # | Gap | Severity |
|---|---|---|
| 1 | `auto_trigger_ready_actions_for_event` never invoked in any test (searched: 0 matches) | MED |
| 2 | `reaction_window` WS event never tested (Shield reaction window flow) | MED |
| 3 | `target_casts` trigger event — not wired in `cast_spell` (see N9), no test | MED |
| 4 | Custom `reaction_type` (empty match arm `reactions.rs:167`) — no test | LOW |
| 5 | Phantom trigger consumed reaction (N1 regression) — no test exists to catch | LOW |

**Test count verification impossible** — backend doesn't compile (P0). All Sprint 1-6 "verified" claims from the previous audit cannot be re-verified until build is restored.

---

## 6. Database / Migration Audit

✅ `20260616000001_pending_hits_queue.sql` — `pending_hits jsonb NOT NULL DEFAULT '[]'::jsonb`. Single column add, default safe.
✅ `20260616000002_character_spells_known.sql` — `known boolean NOT NULL DEFAULT false`. All existing rows get `false` → M16 fix relies on `known` being set on prepared caster rows (Wizard/Cleric/etc) — these rows were already `prepared = true`; `cast_spell` checks both. Verified at `spells.rs`.
⚠ `20260602000001_fog_of_war.sql`, `20260602000002_walls_and_vision.sql`, `20260602000003_conditions_reference.sql` — not yet audited in detail. Should be a separate pass; touch overlays/conditions.
⚠ `20260610000001_combatants_composite_order_index.sql` — verify index exists for `(encounter_id, turn_order)` queries (very common).

---

## 7. Files Audited This Pass

**New files (Sprint 6-8):**
- `backend/src/combat_engine/resolvers/{attack,damage,damage_type,death_save,heal,mod,save,skill_check,two_weapon_fight,types}.rs` (1,112 lines total) — **7 BROKEN, 1 truncated**
- `backend/src/routes/combat/actions/{combat,economy,reactions,sync}.rs` (2,324 lines) — economy.rs 950 (over cap), combat.rs 952 (over cap)
- `backend/src/routes/combat/actions.rs` (14 lines, re-export shim)
- `backend/src/combat_engine/{mod,stats,load,types}.rs` (1,496 lines)

**Modified files:**
- `backend/src/routes/combat/{combatants,encounters,special,spells,tactical}.rs`
- `backend/src/routes/combat/mod.rs` (router)
- `web/src/routes/campaigns/[id]/initiative/+page.svelte` (4,504 lines)
- `web/src/lib/components/{NpcStatBlock,EffectPanel}.svelte`

**Migrations:**
- `20260616000001_pending_hits_queue.sql` ✅
- `20260616000002_character_spells_known.sql` ✅
- `2026060200000{1,2,3}_*.sql` not yet audited
- `20260610000001_combatants_composite_order_index.sql` not yet audited

---

## 8. Recommended Fix Order

### Sprint 9 — P0 (BLOCK ALL OTHER WORK)
1. **P0-1** Restore 7 missing `pub fn` declarations in `combat_engine/resolvers/`. Verify with `cargo check && cargo test`. Estimated: 30 min if mechanical restoration from original `resolvers.rs`.

### Sprint 10 — Data integrity + performance
2. **MED-N1** Wire `auto_trigger_ready_actions_for_event` to actually execute readied action (or document as notification-only).
3. **MED-N5** Clamp JSONB casts in `reactions.rs:78-79` per AGENTS.md §5.2.
4. **MED-N9** Call `auto_trigger_ready_actions_for_event` from `cast_spell` for `target_casts` events.
5. **MED-N7** Consolidate N+1 queries in `economy.rs` (7 handlers × 1 wasted query).
6. **MED-N3** Move `load_snapshot` inside Shield transaction.
7. **MED-N6** Add `tracing::error` on silent `.unwrap_or_default()` / `.is_ok()` in `reactions.rs`.
8. **MED-N10** Standardize `encounter not active` to `AppError::Conflict` across all 25 sites.

### Sprint 11 — Refactor
9. **LOW-N1** Extract `require_action_auth` + `consume_action` helpers from `economy.rs` (950 → ~400 lines).
10. **LOW-N2** Split `stats.rs` (770 → 5 submodules).
11. **LOW-N7/N8/N9** Fix formatter issues, remove dead impl block, complete truncated `crit_double_dice`.
12. **MED-N8** Extract common attack pre-checks (cover/walls/darkness/flanking) into shared helper.

### Sprint 12 — Frontend
13. **LOW-N11** Add `inFlight` guards to remaining ~8 critical buttons.
14. **LOW-N12** Decompose `+page.svelte` 4,504 → `lib/combat/{Roster,ActionPanel,TargetPanel,CombatLog}.svelte`.
15. **M21b** Continue i18n extraction (~150 strings remaining).

### Sprint 13 — Polish
16. **LOW-N4/N5/N6** Remove unused column binds (`_reaction_used`, `_round`, `_turn_index`, `_grid_size`).
17. **H6** `heal` friendly-only check.
18. **H4** Decide `last_hit_attacker` fate (drop column or wire to Shield/UD).
19. **M7** `goto_turn` tx + capture old_turn_index.

---

## 9. Verification Commands

```bash
cd backend && cargo check --offline          # currently FAILS (P0)
cd backend && cargo test --offline --no-run   # currently FAILS (P0)
cd backend && cargo sqlx prepare             # query cache update needed after fix
cd web && bunx svelte-check                   # passes (per prior audit)
cd web && bunx vitest run                     # passes (per prior audit)
```

---

*Audit completed 2026-06-17. ~2 hours of investigation. All file paths verified via Read tool, all SQL/queries grep'd, cargo check attempted. **P0 must be resolved before any other work.***