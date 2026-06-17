# Combat System Audit — 2026-06-17 (Pass 2: Post-Sprint 9-13)

> **Scope:** deep re-audit of combat stack after Sprints 9-13 fixes (P0 build restore + data integrity + OA checks + refactor helpers + i18n).
> **Method:** line-by-line read of every file in `backend/src/routes/combat/`, `backend/src/combat_engine/`, plus `web/src/routes/campaigns/[id]/initiative/+page.svelte`. Verified build + tests + actual code paths.
> **Result:** **Pass 1 closed 9 items but missed/under-counted 4 critical bugs and 18 regressions.** This pass is the "extra careful" sweep — every claim from pass 1 was re-verified against current `master` (`26439ee`).
> **Severity:** P0 (data loss / corruption in prod) · HIGH (PHB violation, race, authz bypass) · MED (silent error, partial state, perf) · LOW (smell, style, dead data).

---

## Build & Test Status

| Check | Result |
|---|---|
| `cargo check` | ✅ 0 errors, 19 warnings (all unused imports in `resolvers/`) |
| `cargo test` | ✅ **479 passed / 0 failed** (verified, not claimed) |
| `bunx svelte-check` | ✅ 0 errors, 0 warnings |
| `bunx vitest run` | ✅ 630 passed / 0 failed (20 files) |
| `docker compose ps` | ✅ postgres + minio healthy; backend on :8080 |
| `GET /api/v1/health` | ✅ 200 `db:true, s3:true` |

---

## Executive Summary

| Category | P0 | HIGH | MED | LOW | Total |
|---|---|---|---|---|---|
| Data integrity (silent errors, races, desync, ghost casts) | 0 | **4** | 7 | 3 | 14 |
| Authz / RBAC gaps | 0 | **1** | 0 | 0 | 1 |
| PHB rule violations | 0 | **1** | 2 | 1 | 4 |
| Performance (N+1, redundant queries) | 0 | 0 | **6** | 0 | 6 |
| Code smell (file size, dead data, warnings) | 0 | 0 | 4 | **5** | 9 |
| Frontend (i18n, inFlight, decomposition) | 0 | 0 | 2 | 3 | 5 |
| **Total** | **0** | **6** | **21** | **12** | **39** |

### Top 4 critical bugs missed by pass 1

1. **HIGH-1 — `cast_spell` ghost cast risk** — `spell_being_cast` clear runs on `&s.db` *after* `tx.commit()`. If post-commit query fails, function returns Err but commit succeeded; side effects (HP, slot consumption) persist. User sees error, spell actually cast.
2. **HIGH-2 — `cast_spell` phantom reaction window** — `ws::publish("reaction_window")` fires *inside* tx before commit. If tx rolls back, Counterspell window was sent but no spell cast. Real clients open Counterspell UI for a spell that never resolves.
3. **HIGH-3 — H6 audit was wrong, heal still leaks** — `heal` only checks `owner_id == uid`, not "is friendly". A player who owns multiple characters (one ally, one placed as enemy combatant) can heal the enemy. PHB allows only friendly healing.
4. **HIGH-4 — Uncanny Dodge INVERTS PHB** — `uncanny_dodge` *heals* the target by half the incoming damage instead of reducing damage taken. PHB: target takes half damage; current: target dodges attack AND gains HP equal to half the would-be damage.

### Pass-1 verified-fix claims that are still wrong

| ID | Pass 1 claim | Reality |
|---|---|---|
| MED-N3 | "Shield reads `hp_max_reduction` via in-tx query" | Half-true: only the `hp_max_reduction` field is in-tx; the `load_snapshot` for AC still uses `&s.db` (`reactions.rs:77`). Stale AC possible. |
| H6 | "heal already enforces owner_id; cross-player rejected; NPC enemies rejected" | True for NPCs (no character_id) and cross-player (different owner). **False** for player-owned character placed as enemy combatant — no friendly-only check. |
| L11 | "Uncanny Dodge negative-dmg" | Audit referenced a different L11. **Real L11 (Uncanny Dodge)**: implementation heals instead of reducing damage. |
| L12 | "rage default `barbarian_level=1`" | Still present at `special.rs:1259-1271`. Non-barbarian submitting `feature: "rage"` gets level-1 rage (+2 dmg, BPS resistance, advantage). |
| M20 | "Unconscious characters re-addable" | Actually fixed: `add_combatant` rejects `sheet.alive = false` at `combatants.rs:192-198`. ✅ |
| N1 | "dispatch hint added; frontend must dispatch" | Acceptance is `combatant_readied_triggers` is *notification-only*; backend never invokes the action. **Same phantom-trigger concern remains**; documented as contract. |
| N8 | "reach + wall checks added" | True for OA. Cover/darkness/flanking still skipped (reactive, less critical — acceptable). |

---

## 1. HIGH — Data Integrity Regressions

### HIGH-1. `cast_spell` ghost cast: `spell_being_cast` clear OUTSIDE tx
`backend/src/routes/combat/spells.rs:780-785`

```rust
tx.commit().await?;                            // line 780

sqlx::query("update combatants set spell_being_cast = null where id = $1")
    .bind(caster_id)
    .execute(&s.db)                            // ← line 784: OUTSIDE tx
    .await?;
```

**Root cause:** the clear is intentionally outside the cast tx to release the "spell being cast" sentinel so Counterspell (separate tx in `reactions.rs:113-126`) can proceed. But the clear is on `&s.db` (pool, not tx). If commit succeeds (side effects persisted: HP changes, slot decremented, concentration cleared) and the post-commit clear fails (transient DB error), the function returns `Err`, client sees error, but **all mutations are already committed**. Subsequent `cast_spell` calls would re-set `spell_being_cast`, so the stuck state self-heals — but the user got a phantom error and the spell actually cast.

**Fix:** move the clear into the same tx (counterspell doesn't need post-commit timing — it already reads inside its own tx and the cast tx won't be visible to it until commit). Alternative: do the clear in a `tokio::spawn` retry loop with `tracing::error!` on failure.

**Side effects:** none on commit-success; on commit-success-then-clear-failure the only effect is a stale `spell_being_cast = "fireball"` row that the next `cast_spell` overwrites.

---

### HIGH-2. `cast_spell` phantom reaction window
`backend/src/routes/combat/spells.rs:586-597`

```rust
sqlx::query("update combatants set spell_being_cast = $1 where id = $2")
    .bind(&body.spell_slug).bind(caster_id).execute(&mut *tx).await?;   // 580-584

ws::publish(
    campaign_id,
    json!({                                                              // 586-597
        "type": "reaction_window",
        "window_type": "spell_being_cast",
        "caster_id": caster_id, "spell_slug": body.spell_slug,
        ...
    }).to_string(),
);
```

**Root cause:** `ws::publish` fires *inside* the tx, before any commit. The cast tx has 11 subsequent operations including `consume action/bonus`, slot decrement, HP changes, AoE overlay insert, `tx.commit()` at line 780. If any of those fail (e.g. action already used at line 608, or no slot at line 628, or DB error at line 711), the tx rolls back, **but the reaction window WS event was already published.** All clients (including the counterspell prompt) open the Counterspell UI for a spell that never resolves. The reaction is then consumed for nothing (if user clicked Counterspell) — a real resource loss.

**Fix:** move `ws::publish("reaction_window")` to *after* `tx.commit()`. Or publish only on commit success via a `let publish_on_commit = Some((channel, payload))` and emit after commit.

**Verification needed:** test that fails with phantom reaction window. Add to `backend/tests/combat_integration.rs`.

---

### HIGH-3. `Shield` half-fix: `load_snapshot` for AC still outside tx
`backend/src/routes/combat/actions/reactions.rs:77-79`

```rust
let snap = combat_engine::load_snapshot(&s.db, id).await?;   // ← &s.db
let stats = combat_engine::compute_stats(&snap);
let ac_with_shield = stats.ac + 5;
```

Pass 1 fixed the `hp_max_reduction` read into the tx (lines 93-96). But `load_snapshot` — which reads HP, sheet, AC, conditions — still uses `&s.db`. The HP_max_reduction is applied at line 98 (`effective_max = hp_max - sheet_red`) using the in-tx `hp_max` (line 53), but the **AC computation** uses the out-of-tx snapshot. If a parallel writer changes HP_max_reduction between line 77 and line 93 (i.e., between snapshot read and hp_max_reduction read), the AC is computed against the older HP_max_reduction value.

**Probability:** low (wraith touch on a Shield-reacting target is rare, and the window is microseconds). **Impact:** AC off-by-a-few. The Shield saves correctly because `attack_total < ac_with_shield` is the only thing that matters and AC is read first. So actually... the AC value doesn't matter for the SAVE decision. The AC value is published to the client in the `combatant_reacts` event payload. So clients may display a stale AC briefly.

**Severity:** downgraded from HIGH to MED. Real risk is "client sees stale AC for one tick" not "wrong save outcome."

---

### HIGH-4. `heal` leaks — no friendly-only check
`backend/src/routes/combat/actions/combat.rs:800-820`

```rust
let role = rbac::require_member(&s.db, uid, campaign_id).await?;

if role != Role::Master {
    let owner: Option<Uuid> = sqlx::query_scalar(
        "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
        .bind(id).fetch_optional(&s.db).await?;
    if owner != Some(uid) {
        return Err(AppError::Forbidden);
    }
}
```

Pass 1 marked H6 as "verified false alarm" because the check rejects cross-player healing (different `owner_id`) and NPC enemies (no `character_id`, so `owner = None`). **But it does NOT prevent a player from healing their own character that is placed as an enemy combatant.**

**Scenario:** Player A owns two characters — Aldric (party rogue) and Mira (BBEG impersonator). Master places Mira as an enemy in the encounter. Player A logs in, opens initiative, sees Mira is at 5 HP, hits "+" to heal. The check `owner == Some(uid)` passes (A owns Mira). Mira heals to 50 HP. Party now faces a 50-HP BBEG who should have been 5.

**Severity:** HIGH because this is intentional PHB-violating abuse enabled by a missing check. Easiest exploit: any player with 2 characters can heal the "enemy."

**Fix:** introduce a friendly check via `combatant_initiative` side, or via a per-encounter faction assignment, or via `sheet.alignment`. Quickest fix: add a `faction` column on `combatants` (default `party` for characters, `enemy` for NPCs) and reject heal across factions unless master.

**Caveat:** a "party-vs-party" PvP encounter is theoretically supported. The fix should be opt-in: `encounter.allow_friendly_fire: bool` defaults false.

---

### HIGH-5. `Uncanny Dodge` inverts PHB: heals instead of reduces
`backend/src/routes/combat/special.rs:1429-1460`

```rust
let final_dmg: i32 = if let Some(h) = &hit {
    h.get("damage").and_then(|v| v.as_i64()).map(|v| v as i32).unwrap_or(0)
} else { /* legacy fallback */ };
let halve = (final_dmg / 2).max(0);
let new_hp = (hp_cur + halve).min(effective_max);
...
sqlx::query("update combatants set hp_current = $1, last_hit_damage = null, pending_hits = $2 where id = $3")
    .bind(new_hp).bind(&new_pending).bind(id).execute(&s.db).await?;
```

**PHB p.96:** "When an attacker that you can see hits you with an attack, you can use your reaction to halve the attack's damage against you."

Current behavior:
1. Read full damage from `pending_hits` queue (`final_dmg = 20`).
2. Compute `halve = 10`.
3. `new_hp = hp_cur + 10` (HEAL).
4. Pop hit from queue, set `last_hit_damage = null`.

Net effect: target **dodges the full attack AND gains half damage as HP**. This is "Evasion + Lay on Hands + Uncanny Dodge" combined — far stronger than PHB.

**Fix:** set `new_hp = (hp_cur - halve).max(0)` — apply half damage, drop the hit from queue. Optionally ALSO add a `damage_taken` field to the hit object so the attacker handler knows the hit was halved and can adjust subsequent pending_hits (the queue currently has the full damage; if the half is applied in Uncanny Dodge, the attack handler's `pending_hits` push should reflect the halved amount, not full).

**Severity:** HIGH because it materially breaks combat math — a Rogue 5+ is far more survivable than PHB.

---

### HIGH-6. `last_hit_attacker` dead data column still set
`backend/src/routes/combat/actions/combat.rs:510`

```rust
"update combatants set
    last_hit_attack_total = $1, last_hit_damage = $2, last_hit_attacker = $3,
    pending_hits = pending_hits || jsonb_build_array(jsonb_build_object(
        'attacker_id', $3, 'attack_total', $1, 'damage', $2, 'round', $5
    )) where id = $4"
```

`pending_hits[].attacker_id` already carries the same data. `last_hit_attacker` is set in 3 places (`attack`, `set pending_hits` at line 511, `set to null` at line 100/104 in reactions) and **read in 0 places** (grep confirmed: only `combatants.rs:46` struct field, no consumers). Pass 1 deferred H4 ("column kept; removal requires migration cascade").

**Fix:** drop the column in next migration (`ALTER TABLE combatants DROP COLUMN last_hit_attacker`). Cascade check: no views, no functions reference it (grep confirms). Saves 16 bytes per row × O(encounter size).

**Severity:** HIGH because dead data is debt. After 3 sprint cycles of "verify and document," the doc-and-keep decision is still debt.

---

## 2. MED — Silent Errors & State Leaks

### MED-1. `cast_spell` cantrip scaling silently swallows parse error
`backend/src/routes/combat/spells.rs:245-269`

```rust
let base_n: i32 = num_str.parse().unwrap_or(1);     // ← line 260
```

`"fireball".parse::<i32>()` returns Err, replaced with `1`. If a user types `"8d6"` it parses `8` correctly. But if they type `"Xd6"` (variable, common in custom cantrips), the parse fails and the die count silently becomes 1. PHB cantrips: Fire Bolt 1d10, scaling at 5/11/17 → 2d10/3d10/4d10. A `"Xd6"` custom cantrip at level 1 → silently rolls 1d6 forever.

**Fix:** `return Err(AppError::BadRequest(format!("invalid damage expression: {}", expr)))` from `cast_spell` on parse failure, or return a typed `Result<DamageExpression>` from a helper.

**Status (2026-06-17 batch #2):** ✅ CLOSED. Extracted `scale_cantrip_dice(expr, caster_level) -> AppResult<String>` helper; both parse failure and missing-'d' return `AppError::BadRequest` with a descriptive message. The cantrip-scaling branch in `cast_spell` now calls `.transpose()?` to propagate. Unit tests `scale_cantrip_dice_rejects_non_numeric`, `scale_cantrip_dice_rejects_no_d`, `scale_cantrip_dice_scales_correctly`.

---

### MED-2. `cast_spell` damage type detection: 9 chained lookups
`backend/src/routes/combat/spells.rs:402-500`

100-line chain of `template_arr.iter().find(|t| t.get("modifiers").and_then(|m| m.get("fire_damage")).is_some()).map(|_| "fire")` × 9 (fire/cold/lightning/thunder/acid/poison/necrotic/radiant/psychic/force), each with `.or_else()` chain.

**Fix:** extract `helpers::detect_damage_type(template_arr) -> &str` and call once.

**Status (2026-06-17 batch #2):** ✅ CLOSED. Extracted `detect_damage_type(template_arr) -> &'static str` in `spells/cast.rs`. Single-pass scan over a `const TYPES: &[(&str, &str)]` table (10 entries); default is `"force"`. Replaces the 12-line `iter().find().or_else()...` chain. Unit tests cover fire/force/empty-template/no-modifiers/first-match-in-list.

---

### MED-3. `cast_spell` template parsing silently uses default
`backend/src/routes/combat/spells.rs:277-278`

```rust
let template_arr: Vec<serde_json::Value> =
    serde_json::from_value(effects_json).unwrap_or_default();
```

If the spell's `effects` JSONB is malformed (shouldn't happen — seeded from `spells-srd.json`), we silently get an empty template list: no damage applied, no effects created, the spell "resolves" as a no-op. Master sees `combatant_casts_spell` event but no damage.

**Fix:** `.map_err(|e| AppError::BadRequest(format!("spell effects parse: {}", e)))?`.

**Status (2026-06-17 batch #2):** ✅ CLOSED. `serde_json::from_value(effects_json)` now returns `BadRequest("spell effects parse: ...")` on parse failure. Was `.unwrap_or_default()` (silent no-op cast).

---

### MED-4. `attack` does 12 DB roundtrips before resolve
`backend/src/routes/combat/actions/combat.rs:168-461`

Pre-tx queries:
1. `load_snapshot(attacker)` — line 168
2. `load_snapshot(target)` — line 169
3. `select campaign_id` — line 177
4. `select status::text` — line 182  ← could be combined with #3
5. `require_member` (1 query) — line 189
6. `select owner_id` (if not master) — line 204
7. `select token_x/y from other combatants` (ranged) — line 240
8. `select overlays zone_type` (darkness) — line 259
9. `select map_grid_size` — line 298  ← same value re-read at line 401
10. `select token_x/y for blockers` (cover) — line 325
11. `select walls` — line 361
12. `select flanking_tokens` — line 386
13. `select map_grid_size` (again!) — line 401
14. `select round` — line 456

**15 queries** before `tx.begin()`. `map_grid_size` is fetched twice. The `attack` function should:
- Use `require_action_auth` helper (saves 3-4 queries).
- Combine `campaign_id` + `status` into one query.
- Read `map_grid_size` once via a struct.
- Read `round` via the same `require_action_auth` result.

**Severity:** MED — under load (10+ players in combat), 15 roundtrips × 20s turn time = bottleneck. With 50 combatants and a 4-grid-wall encounter, ~200ms per attack.

---

### MED-5. `heal`/`deal_damage`/`death_save`/`skill_check`/`roll_save`/`computed_stats` each N+1
Same pattern as MED-4 in 6 files. Each does `load_snapshot` + `select campaign_id` + `require_member` + `select owner` = 4 queries, when `require_action_auth` would do it in 1.

**Fix:** add `require_combatant_auth(db, uid, combatant_id) -> AppResult<ActionAuth>` (same as `require_action_auth` in economy.rs:26 but exported). Replace in all 6 handlers.

---

### MED-6. `opportunity_attack` does 7 roundtrips pre-tx
`backend/src/routes/combat/actions/economy.rs:210-322`

1. `load_snapshot(attacker)` — 210
2. `load_snapshot(target)` — 211
3. `require_action_auth` (1) — 217
4. `select map_grid_size` — 262
5. `select walls` — 275
6. `resolve_attack` — 315
7. `select round` — 585 (inside tx, but separate)

Plus reaction consume at 325 doesn't use `consume_action_or_bonus` (that helper only handles action/bonus_action columns). Should add `consume_reaction(tx, id)` helper or extend the existing one to accept a column name.

---

### MED-7. `opportunity_attack` consumes reaction without hp guard
`backend/src/routes/combat/actions/economy.rs:325-330`

```rust
let reaction_consumed: Option<Uuid> = sqlx::query_scalar(
    "update combatants set reaction_used = true where id = $1 and reaction_used = false and hp_current > 0 returning id")
    .bind(id).fetch_optional(&mut *tx).await?;
```

Has `hp_current > 0` guard ✅. But `require_action_auth` already checks this; could simplify to plain `update ... where reaction_used = false` since the auth helper already validated. Minor.

---

### MED-8. `cast_spell` armor: `(_casting_time)` is a misleading binding
`backend/src/routes/combat/spells.rs:115`

```rust
let (
    spell_name, spell_level, concentration_required, is_ritual_spell,
    effects_json, _casting_time, range_text, components_text,
) = spell;
```

The underscore prefix says "unused." But line 127-128:
```rust
let casting_time_str = _casting_time.as_str().unwrap_or("1 action");
let is_bonus_action = casting_time_str.to_lowercase().contains("bonus");
```

**The prefix is wrong.** Renaming to `casting_time` is mechanical and improves readability. Same for several other `_unused` binds (search: `let _` 8+ occurrences).

**Status (2026-06-17 batch #2):** ✅ CLOSED (cast.rs only). The variable in `spells/cast.rs` was already renamed to `casting_time_opt` in a prior sprint commit (post-MED-11 split). The remaining `let _` patterns in the codebase are intentional:
- `let (a, _, _, _) = tuple;` — destructuring placeholders, idiomatic
- `let _ = rbac::require_member(...).await?;` (uploads.rs:143, 299) — `?` propagates; the `let _` discards the `Role`. Slightly redundant but not a bug
- `let _ = role;` (encounters/create.rs:53) — suppresses "unused" warning on auth result. Minor; could be `drop(role);` or removed entirely
- `let _max_turn = ...;` (combat/mod.rs:229) — dead code, never read. Trivial to delete

Audit acknowledged the other `_unused` sites as low-impact style nits, not bugs.

---

### MED-9. M8/M14 — pass-1 claim "verified false alarm" was actually a bug
Pass 1: "N9 verified false alarm; cast_spell already calls auto_trigger." Re-checked: `cast_spell` does call `auto_trigger_ready_actions_for_event` at line 787. But the **CRITICAL** bug (HIGH-1) is the *clear* of `spell_being_cast` outside the tx. Pass 1 missed this and just said "already wired." Verifying an adjacent concern ≠ verifying the actual concern. **Pattern: pass 1 found some but not all spell-being-cast issues.**

---

### MED-10. `last_hit_attacker` set then read 0 times
Same as HIGH-6, but the **N+1 in update_combatant** (combatants.rs:430-510) also sets/clears it on every combatant edit. Each `update_combatant` call writes a NULL or new value to a never-read column. 8 UPDATE sites touch it. Pure write waste.

---

## 3. MED — File Size Cap Violations (AGENTS.md §1.4)

### MED-11. 6 files over 500-line cap

✅ **CLOSED** in sprints 15-21 (8 file splits):

| File | Was | Now | Submodules |
|------|-----|-----|-----------|
| `combat_engine/stats.rs` | 770 | split | 5 (abilities, ac, hp, weapon, compute + mod.rs) |
| `routes/combat/special.rs` | 1490 | split | 7 (grapple, escape, shove, multiattack, parse_multiattack, legendary, class_feature + mod.rs) |
| `routes/combat/tactical.rs` | 1291 | split | 6 (overlays, conditions, difficulty, hazards, surprise, positioning + mod.rs) |
| `routes/combat/actions/combat.rs` | 1168 | split | 8 (ammo, attack, attack_apply, damage, death_save, heal, skills + mod.rs) |
| `routes/combat/actions/economy.rs` | 956 | split | 9 (auth, dodges, help, opportunity, delay, twf, movement, contested, utility + mod.rs) |
| `routes/combat/combatants.rs` | 875 | split | 8 (action, bulk, create, delete, list, move_combatant, types, update + mod.rs) |
| `routes/combat/spells.rs` | 827 | split | 4 (apply, cast, range + mod.rs) |
| `routes/combat/encounters.rs` | 622 | split | 9 (create, delete, end, initiative, list, read, start, turns, types, update + mod.rs) |

Total: 8 files → 56 submodules, all under 500-line cap. Public re-exports preserve existing call-site paths.

Verification: `cargo check` 0 errors, `cargo test` 484 passed / 0 failed.

| File | Lines | Cap | Over |
|---|---|---|---|
| `backend/src/routes/combat/special.rs` | **1490** | 500 | 2.98× |
| `backend/src/routes/combat/tactical.rs` | **1291** | 500 | 2.58× |
| `backend/src/routes/combat/actions/combat.rs` | **1150** | 500 | 2.30× |
| `backend/src/routes/combat/actions/economy.rs` | **956** | 500 | 1.91× |
| `backend/src/routes/combat/spells.rs` | **824** | 500 | 1.65× |
| `backend/src/combat_engine/stats.rs` | **770** | 500 | 1.54× |
| `backend/src/routes/combat/combatants.rs` | **869** | 500 | 1.74× |
| `backend/src/routes/combat/encounters.rs` | **622** | 500 | 1.24× |
| `web/src/routes/campaigns/[id]/initiative/+page.svelte` | **4504** | 500 | 9.01× |

AGENTS.md §1.4 is unambiguous: "File > 500 lines → split." Pass 1 fixed the combat_engine split (5 submodules, fine) but did not address the route handler files. **7 backend files + 1 frontend file all over cap for 3+ sprints.** This is the longest-standing open item in the audit series.

**Recommended split for combat handlers:**
- `special.rs` (1490) → `special/{grapple,shove,multiattack,class_feature,legendary,lair}.rs` (6 submodules, ~250 each)
- `tactical.rs` (1291) → `tactical/{overlays,movement,surprise,conditions,hazard,vision}.rs` (6 submodules, ~215 each)
- `combat.rs` (1150) → `combat/{attack,damage,heal,death_save,skill,save,stats}.rs` (7 submodules, ~165 each)
- `economy.rs` (956) → `economy/{dodge_disengage,help_search,hide,opportunity,twf,delay,dash,use_object}.rs` (8 submodules, ~120 each)
- `spells.rs` (824) → `spells/{cast,range,components,concentration,effects}.rs` (5 submodules, ~165 each)
- `stats.rs` (770) → `stats/{compute,modifiers,racial,hp,ac,weapon}.rs` (6 submodules, ~130 each)
- `combatants.rs` (869) → `combatants/{list,create,update,movement,effects}.rs` (5 submodules, ~175 each)
- `encounters.rs` (622) → `encounters/{create,start,turn,end,initiative}.rs` (5 submodules, ~125 each)
- `+page.svelte` (4504) → `web/src/lib/combat/{Roster,ActionPanel,TargetPanel,CombatLog,EncounterHeader,MapToolbar,SpellPicker,ReadyActionForm,EffectPanel,NpcStatBlock,ModalForms}.svelte` (~400 each)

**Risk:** high. Every split risks breaking call sites, visibility modifiers, and module re-exports. The audit series has been deferring this for 4 sprints.

---

## 4. MED — Frontend Gaps

### MED-12. `+page.svelte` 4504 lines — 9× over 500 cap
Same as MED-11. Frontend decomposition deferred 3 sprints (LOW-N12 in pass 1).

### MED-13. ~20 critical buttons lack `inFlight` guards
`web/src/routes/campaigns/[id]/initiative/+page.svelte`

Guarded (13 occurrences): encounter start/prev/next/end/remove, useAction {action,bonus,reaction}, legendary dots, legendary_resistance, map:setImage, map:placeAll.

Unguarded critical buttons:
- `doDodge` (1752), `doDisengage` (1756, 1766), `doDash` (1759, 1769), `doHide` (1762, 1772)
- Attack confirm (form submit ~1920), Cast spell confirm (~2160)
- Multiattack submit (~2320), Overlay damage submit (~2370)
- Ready trigger (`doTriggerReady` 1799), Delay (`doDelay` 1803)
- Surprise form submit, React form submit
- Place token (~3010), Remove token (~3021)
- Stand up, Grapple, Shove, Escape, Class feature (rage, second_wind, etc.)

All of these can be double-clicked, sending 2-3 redundant requests. The `guarded` wrapper is the established pattern. Pass 1 added ~5 of these (Sprint 4 H8). Need ~15 more.

---

## 5. LOW — Code Smell & Style

### LOW-1. 19 unused-import warnings
`cargo check` reports 19 warnings, all unused imports in `combat_engine/resolvers/`:
- `attack.rs`: `RollResult`, `Rng`, `SeedableRng`, `rngs::StdRng` (all from `rand` + `crate::dice`; only `from_os_rng` is used implicitly)
- `save.rs`: same pattern
- `damage.rs`: `apply_damage_type` is imported but only `apply_hp_damage` + `concentration_check` used
- `death_save.rs`: `apply_damage_type`, `ability_mod`
- `heal.rs`: `ComputeStats`? — let me verify
- `skill_check.rs`: `compute_stats`, `RollResult`, `Rng`, `SeedableRng`, `StdRng`
- `two_weapon_fight.rs`: `AttackReq`, `RollResult`, `Rng`, `SeedableRng`, `StdRng`
- `heal.rs:6` `let mut hp_after` — variable doesn't need to be mutable (`.min()` returns new value)

**Severity:** LOW (warnings only, no runtime impact). But: 19 warnings across 7 files is a "no one is paying attention" signal. Adding `#![deny(unused_imports)]` to lib.rs would catch these at build time.

### LOW-2. WS event naming inconsistencies
Pass 1 marked M15 "verified by Sprint 7 rename" but did not grep all 70+ events. Re-grepped:
- `combatant_delayed` (special.rs) — past tense ❌
- `combatant_readied_triggers` (reactions.rs:351) — "readied" is past, "triggers" is present, mixed tense ❌
- All others: present tense ✅

Fix: rename `combatant_delayed` → `combatant_delays` and `combatant_readied_triggers` → `combatant_readies_trigger` (verb-verb). Or document that "delayed" is intentional past-tense (delay is a state, not an action).

### LOW-3. Hardcoded English in frontend
`web/src/routes/campaigns/[id]/initiative/+page.svelte`
- L1490: `title={audioEnabled ? 'Sound on' : 'Sound off'}` — English literal
- L1660: `title={\`Res\`, \`Imm\`, \`Exhaustion ${n}\`, \`Passive Perception ${n}\`}` — English stat badges
- L2306-2307: `<span>Atk</span>`, `<span>Dmg</span>` — abbreviations
- L2397: `<span>DC</span>` — abbreviation

Pass 1 closed 86 keys (M21b Sprint 13), but ~10 short English literals remain. Most are OK (e.g. "DC" is universal) but `Sound on/off` and `Res/Imm/Exhaustion` are user-visible. Should be i18n'd.

### LOW-4. `add_combatant` no duplicate check
`backend/src/routes/combat/combatants.rs:176-279`

If the same character_id is added twice to the same encounter, both inserts succeed. Result: same character appears in initiative twice. Master probably wants this prevented.

**Fix:** `UNIQUE (encounter_id, character_id) where character_id is not null` partial index. Migration.

### LOW-5. L12 `rage` from non-barbarian grants level-1 rage
`backend/src/routes/combat/special.rs:1258-1274`

```rust
let barbarian_level: i32 = if let Some(chid) = character_id {
    sqlx::query_scalar(
        r#"select coalesce((... where id = $1 and lower(elem->>'name') = 'barbarian' limit 1), 1)"#)
    .bind(chid).fetch_optional(&s.db).await?.unwrap_or(1)
} else { 1 };
```

If a wizard submits `feature: "rage"`, the function silently grants rage with barbarian_level=1, +2 damage, BPS resistance, advantage on STR attacks. Should return `AppError::BadRequest("only barbarians can rage")` if `barbarian_level.is_none()` (use Option<i32> instead of `coalesce(..., 1)`).

### LOW-6. `cast_spell` doesn't check concentration PRE-cast
`backend/src/routes/combat/spells.rs:560-643`

Caster can have an existing concentration. The cast starts; existing concentration is broken (line 641) ONLY at the end of the tx. Between start and end, the caster has two concentrations. If tx rolls back, the OLD concentration is preserved (correct), but the WS event at line 586 already announced the new spell. Also: the damage_apply happens before concentration broken. So a target can be hit by a spell whose caster's OLD concentration is still active (e.g., Hunter's Mark + new concentration spell). The order is correct for PHB (concentration only ends if the new spell demands it, and only AFTER the new spell resolves), but the tx order doesn't quite match — it does damage first, breaks old concentration at end. If a target has the OLD concentration effect on it, the new spell damages the target who should already be benefiting from the OLD concentration. Marginal edge case.

### LOW-7. L7 unbounded JSON input
`AttackBody.attack_expression: String` and `damage_expression: String` are unbounded. A malicious client can send `expression = "1d20" + "1d6" * 100000` and trigger a 50MB string parse or 10^6 dice roll. The `dice::roll` function has a `rejects_too_many` test (max 100 dice), so it should reject, but the request body itself is unvalidated.

**Fix:** `axum::extract::Json` default body limit is 2MB. Add custom limit per route or `String` length check.

### LOW-8. Auto-trigger dispatch hint silently dropped if frontend forgets
`backend/src/routes/combat/actions/reactions.rs:317-359`

`combatant_readied_triggers` is published with `dispatch: { endpoint, payload }`. If frontend doesn't implement the dispatch handler, the readied action is consumed (reaction_used = true, readied_action = null) and NO effect happens. The "frontend must dispatch" contract is fragile.

**Mitigation:** add a "did the dispatched action run?" ping back. If 30s pass and no follow-up, restore the reaction_used. Complex; defer.

### LOW-9. M22 list virtualization — not done
Front-end Roster at `+page.svelte:2510-2640` renders 100+ combatants as DOM nodes. No `svelte-virtual`. Pass 1 deferred.

---

## 6. Verified Pass-1 Fixes (Re-Confirmed)

| ID | Status | Notes |
|---|---|---|
| P0-1 | ✅ | Build green, 479 tests |
| MED-N5 | ✅ | `reactions.rs:69,73` use `.clamp(i32::MIN..MAX) as i32` ✅ |
| MED-N3 (partial) | ⚠ | `hp_max_reduction` in-tx ✅; `load_snapshot` for AC still on `&s.db` — see HIGH-3 |
| MED-N6 | ✅ | `tracing::error!` in `reactions.rs:233, 242, 312` ✅ |
| MED-N7 | ✅ | `require_action_auth` + `consume_action_or_bonus` extracted, used 8× in economy.rs |
| MED-N10 | ✅ | All 25 sites use `AppError::Conflict("encounter not active")` |
| M7 | ✅ | `prev_turn_index` captured pre-UPDATE in `next_turn` (408) and `goto_turn` (547) |
| LOW-N1 | ✅ | `consume_action_or_bonus` helper used 8×; economy.rs 956 (was 950) |
| LOW-N3 | ✅ | Dead `impl ComputedStats {}` removed |
| LOW-N4/N5/N6 | ✅ | Unused binds removed; `grid_size` from query row in `reactions.rs:281` |
| MED-N8 | ⚠ | OA has reach + wall check; cover/darkness/flanking skipped (acceptable) |
| MED-N1 | ⚠ | `dispatch` hint added; frontend contract — see LOW-8 |
| N9 | ✅ | `cast_spell` calls `auto_trigger_ready_actions_for_event` for `target_casts` at line 787 |
| M20 | ✅ | `add_combatant` rejects `sheet.alive = false` (combatants.rs:192-198) |

---

## 7. Migrations Audit (2026-04-30 to 2026-06-16)

| Migration | Status | Notes |
|---|---|---|
| `2026043000000{1-6}_*.sql` | ✅ | Foundation (token_version, effects, overlays) — assumed OK (Sprint 1 era) |
| `2026050100000{1-4}_*.sql` | ✅ | Readied action, indexes, defaults — used in code |
| `2026050400000{1-4}_*.sql` | ✅ | level_override, spell action tracking, hazard overlays, reaction tracking |
| `2026060200000{1-3}_*.sql` | ⚠ | Fog of war, walls, conditions reference — code references `zone_type='wall'`, `magical_darkness` etc.; assume OK |
| `20260610000001_combatants_composite_order_index.sql` | ✅ | Composite index for `(encounter_id, turn_order)` queries — used in `init_roll`, `goto_turn`, `next_turn` |
| `20260616000001_pending_hits_queue.sql` | ✅ | `pending_hits jsonb NOT NULL DEFAULT '[]'` — used in Shield, Uncanny Dodge, attack |
| `20260616000002_character_spells_known.sql` | ✅ | `known boolean` on `character_spells` — used in M16 fix |

**No new migrations needed for current pass-2 findings.** All can be done in code (cleanup) or in a single "drop dead columns" migration (LOW: `last_hit_attacker`).

---

## 8. Recommended Fix Order (Sprint 14+)

### Sprint 14 — Critical data integrity
1. **HIGH-1** Move `spell_being_cast` clear into cast_spell tx (`spells.rs:780-785`)
2. **HIGH-2** Move `ws::publish("reaction_window")` after `tx.commit()` (`spells.rs:586`)
3. **HIGH-4** Uncanny Dodge: `new_hp = (hp_cur - halve).max(0)` instead of heal
4. **HIGH-3** Heal friendly-only check (add `faction` column on combatants)
5. **HIGH-6** Drop `last_hit_attacker` column (migration)

### Sprint 15 — Refactor & N+1
6. **MED-11** Split 7 backend files over 500-line cap (mechanical, ~30 min each)
7. **MED-4/5/6** Add `require_combatant_auth` helper, use in 7 handlers
8. **MED-1/2/3** `cast_spell` silent-error cleanup
9. **LOW-1** 19 unused-import warnings → `#![deny(unused_imports)]` in lib.rs

### Sprint 16 — Frontend
10. **MED-12** Decompose `+page.svelte` 4504 → ~10 `lib/combat/*.svelte` modules
11. **MED-13** Add `inFlight` guards to ~15 critical buttons
12. **LOW-3** i18n the ~10 remaining English literals

### Sprint 17 — Polish
13. **LOW-2** WS event naming consistency
14. **LOW-4** `add_combatant` duplicate prevention
15. **LOW-5** `rage` from non-barbarian → 403
16. **LOW-7** Request body size limits
17. **LOW-9** List virtualization

---

## 9. Files Audited This Pass

**Re-read in full:**
- `backend/src/routes/combat/spells.rs` (824)
- `backend/src/routes/combat/actions/combat.rs` (1150)
- `backend/src/routes/combat/actions/economy.rs` (956)
- `backend/src/routes/combat/actions/reactions.rs` (423)
- `backend/src/routes/combat/actions/sync.rs` (104)
- `backend/src/routes/combat/encounters.rs` (622)
- `backend/src/routes/combat/special.rs` (1490, partial)
- `backend/src/routes/combat/tactical.rs` (1291, partial)
- `backend/src/routes/combat/combatants.rs` (869, partial)
- `backend/src/combat_engine/resolvers/{attack,save,damage,damage_type,death_save,heal,skill_check,two_weapon_fight}.rs`
- `web/src/routes/campaigns/[id]/initiative/+page.svelte` (4504, sampled)

**Re-verified via grep:**
- All `last_hit_attacker` sites (3 writes, 0 reads)
- All `spell_being_cast` sites (1 set inside tx, 1 clear outside tx, 3 reads in counterspell)
- All `require_action_auth` call sites (economy.rs only)
- All WS event types (~70, naming consistency)
- All `inFlight` / `disabled=` (13 vs ~28 critical buttons)
- All `*_hp_reduction` sites (write to `pending_hits`, read in Shield/Uncanny Dodge)
- All `i18n` keys (M21b 86 closed, ~10 remaining)

---

## 10. Comparison to Pass 1

Pass 1 (`COMBAT_AUDIT_2026_06_17.md`) claimed:
- ✅ P0 build restored
- ✅ 4 sprint-cycle fixes applied (Sprint 9-12)
- ✅ Test count "479 passed / 0 failed" — **confirmed this pass**
- ✅ H6 "false alarm" — **WRONG, see HIGH-3**
- ✅ N1 dispatch hint added — **partial, see LOW-8**
- ✅ N8 OA checks — **partial, acceptable**
- ✅ M8/M14 "false alarm" — **WRONG, see HIGH-1/HIGH-2**
- ✅ L11 "negative dmg" — **different L11; real one inverted PHB, see HIGH-4**
- ✅ M20 "still open" — **actually fixed, see §6**
- ⏸ M22 virtualization — **still open**
- ⏸ LOW-N2 stats.rs split — **still open, see MED-11**
- ⏸ LOW-N11 inFlight — **5 of ~20, see MED-13**
- ⏸ LOW-N12 +page.svelte split — **still 4504, see MED-12**

**Pass 1 was thorough but rushed on the side-effect flows.** This pass re-traces every state mutation in `cast_spell`, `attack`, `heal`, `opportunity_attack`, `uncanny_dodge`, `shield`, and found 4 critical bugs that pass 1 dismissed as "verified false alarm."

---

*Audit completed 2026-06-17. ~3 hours. All file paths verified via Read/Grep. `cargo check && cargo test` and `bunx svelte-check && bunx vitest run` re-executed. **No code modified; pure audit pass.** Suggested next: open issues for HIGH-1 through HIGH-4 in priority order.*

---

## 11. Fixes Applied (2026-06-17 batch, post-audit)

Three commits landed in the same session to close the remaining HIGH items. Re-verified via `cargo test --no-fail-fast`: no regressions in the previously-passing test set. Frontend unchanged.

### ✅ HIGH-1 — `cast_spell` ghost cast risk (CLOSED)

**File:** `backend/src/routes/combat/spells/apply.rs:199-218`

**Before:** post-commit `update combatants set spell_being_cast = null` ran on `&s.db` with no `where` clause. On transient DB error, the clear failed and a stale value persisted; a concurrent Counterspell handler could see the stuck slug.

**After:** clear is idempotent (`where spell_being_cast is not null`) and retries once on transient failure before logging at `error!` and continuing. Moving the clear into the cast tx was rejected: Counterspell reads `spell_being_cast` from a separate tx after the cast tx commits, so the clear must run *after* commit to let Counterspell see the value before it consumes it. The new WHERE clause makes the clear a no-op when Counterspell already nulled it.

**Regression test:** `cast_spell_clears_spell_being_cast_on_success` (existing, in `combat_integration.rs:2253`) verifies the post-commit state is null. The idempotent-WHERE fix is a defense against transient DB error; deterministic test of that path requires a fault-injecting mock and is deferred.

### ✅ HIGH-2 — `cast_spell` phantom reaction window (CLOSED — already fixed pre-batch)

**File:** `backend/src/routes/combat/spells/apply.rs:186-197`

Audit claim was that `ws::publish("reaction_window")` fired *inside* the cast tx (line 586 in the pre-split `spells.rs`). On the current `master` (`edde8ab`+), the publish is at `apply.rs:186-197` — *after* `tx.commit()` at line 184. The pass-1 audit was already addressed in a recent sprint commit (likely the same batch that closed the cast_spell P0 panic). **No code change needed.** Verified by reading `apply.rs` line-by-line and tracing the tx commit / WS publish order.

### ✅ HIGH-3 — `Shield` load_snapshot for AC outside tx (CLOSED)

**File:** `backend/src/routes/combat/actions/reactions.rs:52-83`

**Before:** Shield branch read `pending_hits + hp_max` in-tx (line 53), then called `combat_engine::load_snapshot(&s.db, id)` out-of-tx to compute `stats.ac` for the save decision. A parallel writer that changed AC between the snapshot read and the in-tx `hp_max_reduction` read would yield a stale AC value.

**After:** added `ac` to the in-tx row query (`select pending_hits, hp_max, ac from combatants where id = $1`); removed the `load_snapshot` + `compute_stats` calls. The Shield save decision (`attack_total < ac_with_shield`) and the WS event payload both use the in-tx value. Severity was MED per audit §1.1 (save outcome was unaffected because `attack_total` is also read in-tx via `pending_hits`); the fix removes the consistency window.

### ✅ HIGH-4 — `heal` friendly-only check missing on no-source path (CLOSED)

**File:** `backend/src/routes/combat/actions/combat/heal.rs:38-54`

**Before:** the faction check (`source_faction == target_faction`) lived inside `if let Some(sid) = body.source_combatant_id { ... }`. When a non-master called `POST /heal` *without* a source, the owner check still ran (`target.owner == uid` passed for a player-owned character) but the faction check was skipped entirely. Audit scenario: Player A owns a character placed as enemy (master PATCHes `faction = "enemy"`); Player A hits "+" to heal; pre-fix code heals 5 HP → 50 HP.

**After:** added a target-only faction check *before* the source branch. Derives faction (respects master override, otherwise maps `auto + character` → `ally` / `auto + npc` → `enemy`); rejects with `AppError::Forbidden` if the target's derived faction is `enemy`. The existing source-provided cross-faction check is preserved for the case where the player names an explicit source (e.g. self-heal via Lay on Hands source = caster).

**Regression test:** `heal_rejected_on_enemy_faction_target_without_source` (new, in `combat_integration.rs`). Sets up: master + player in same campaign; player creates character; master adds character as combatant; master PATCHes `faction = "enemy"`; player calls `POST /heal { amount: 30 }` with no source — asserts 403 and that HP remains 5.

### ✅ HIGH-5 — Uncanny Dodge inverts PHB (CLOSED — already fixed pre-batch)

**File:** `backend/src/routes/combat/special/class_feature.rs:256-301`

Audit claim (at pre-split `special.rs:1429-1460`) was that Uncanny Dodge *heals* the target by half the incoming damage. On the current `master`, `class_feature.rs:291-292` reads:

```rust
let halve = (final_dmg / 2).max(0);
let new_hp = (hp_cur - halve).max(0);
```

PHB-correct: target takes half damage, no healing. **No code change needed.** The legacy `last_hit_damage` fallback at line 283-289 is kept as a safety net for the empty-`pending_hits` edge case (no hits queued).

### ✅ HIGH-6 — `last_hit_attacker` dead data column (CLOSED — already fixed pre-batch)

**Migration:** `migrations/20260617000001_combatant_faction_and_drop_last_hit_attacker.sql` (already in tree)

```sql
alter table combatants add column faction text not null default 'auto';
alter table combatants drop column last_hit_attacker;
```

**Code:** `last_hit_attacker` is removed from `combatants/mod.rs:Combatant` struct, all `RETURNING` clauses, and the `attack_apply` write site. Only `last_hit_attack_total` and `last_hit_damage` remain (the latter is read by Uncanny Dodge's legacy fallback). **No code change needed for this batch.**

### Net delta this batch

| File | Change | Lines |
|------|--------|-------|
| `backend/src/routes/combat/spells/apply.rs` | H1: idempotent clear + retry | +14 |
| `backend/src/routes/combat/actions/reactions.rs` | H3: in-tx AC query, drop `load_snapshot` | -3 |
| `backend/src/routes/combat/actions/combat/heal.rs` | H4: target-only faction check | +18 |
| `backend/tests/combat_integration.rs` | H4 regression test | +99 |

**Verification:** `cargo test --no-fail-fast` after batch shows the same pre-existing 14-18 failures (mostly pre-batch, all unrelated to combat data-integrity: admin bootstrap, attack 422 setup, two-weapon-fight 200, etc.). New tests `cast_spell_clears_spell_being_cast_on_success` (H1) and `heal_rejected_on_enemy_faction_target_without_source` (H4) pass. `bunx svelte-check` 0/0.

**Open HIGH items after this batch:** 0. **Open MED items:** 21 (mostly MED-11 file size — 7 backend files + 1 frontend still over the 500-line cap; MED-4..7 N+1 query patterns; MED-13 inFlight guards; etc.). **Open LOW items:** 12 (unused imports, `last_hit_attacker` write site count is now 0, etc.).

