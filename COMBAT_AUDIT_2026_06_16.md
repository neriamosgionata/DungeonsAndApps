# Combat System Audit — 2026-06-16

> **Scope:** full combat stack — `backend/src/routes/combat/*.rs` (8 files, 6,904 lines) + `backend/src/combat_engine.rs` (2,572 lines) + battle-map / spells / WS events + `web/src/routes/campaigns/[id]/initiative/+page.svelte` (4,464 lines) + related components + 8 test files (437-test backend suite).
>
> **Method:** 9 parallel investigations covering structure, action economy, reaction lifecycle, combatant↔character sync, WS coverage, RBAC/tx/races/errors, schema-column alignment, frontend UI, and test coverage.
>
> **Severity scale:** HIGH (data loss / data corruption / authz bypass / panic in production) · MEDIUM (race / partial state / PHB-rule violation / leak) · LOW (code smell / style / minor correctness).

---

## Executive Summary

| Category | HIGH | MEDIUM | LOW | Total |
|---|---|---|---|---|
| Data integrity (silent errors, swallos, races) | 5 | 22 | 8 | 35 |
| Authz / RBAC gaps | 1 | 5 | 2 | 8 |
| PHB rule gaps | 1 | 6 | 1 | 8 |
| Frontend i18n / state / UX | 0 | 8 | 4 | 12 |
| File / function size | 0 | 0 | 45+ | 45+ |
| Test coverage | 0 | 7 | 8 | 15 |
| **Total** | **7** | **48** | **68+** | **123+** |

**Top 3 things to fix first:**
1. **`combatants.rs:271-295` `bulk_add_combatants`** — INSERT errors silently swallowed. Any failed combatant is dropped without a trace.
2. **`combat_engine.rs:1841, 2145`** — `unwrap()` / `.expect("valid expression")` in `resolve_save` and `concentration_check`. Production panic if the dice expression parser ever changes behavior.
3. **`cast_spell` / `attack` / `opportunity_attack` / `two_weapon_fight`** — no `encounter.status == 'active'` check. Players can cast spells and attack in `planned` or `ended` encounters.

---

## 1. HIGH Severity — Data Integrity

### H1. `bulk_add_combatants` silently drops failed INSERTs
`backend/src/routes/combat/combatants.rs:271-295`

```rust
if let Ok(c) = sqlx::query_as::<_, Combatant>(...).fetch_one(&s.db).await {
    added.push(c);
}
```

**Symptom:** any DB error on a single combatant silently drops it. No log, no error returned, partial result returned to caller as "success".

**Fix:** replace `if let Ok` with `?` and accumulate row-level errors into a response struct.

---

### H2. Production panics from `unwrap` / `expect` in `combat_engine.rs`
- `backend/src/combat_engine.rs:1841` — `roll("1d20", &mut rng).unwrap()` inside `resolve_save` (auto-fail STR/DEX save path)
- `backend/src/combat_engine.rs:2145` — `roll(&expr, rng).expect("valid expression")` inside `concentration_check`

**Symptom:** hardcoded `"1d20"` is currently infallible but couples to parser internals. Any future dice-expression change panics the backend in production.

**Fix:** use `?` with `AppError` and treat parser failure as a `BadRequest("invalid dice expression")`.

---

### H3. `cast_spell` / `attack` / `opportunity_attack` / `two_weapon_fight` skip encounter status check
- `backend/src/routes/combat/spells.rs:64-176` `cast_spell`
- `backend/src/routes/combat/actions.rs:146-552` `attack`
- `backend/src/routes/combat/actions.rs:1409-1542` `opportunity_attack`
- `backend/src/routes/combat/actions.rs:1788-1948` `two_weapon_fight`

**Symptom:** players can act in `planned` or `ended` encounters. `e.status == "active"` is not asserted. Spells, attacks, reactions, OA all bypass the gate.

**Fix:** add a single `e.status == "active"` check after the encounter fetch, returning `AppError::Conflict("encounter not active")`.

---

### H4. `last_hit_attacker` is dead data with stale-state risk
- Set: `actions.rs:435` on every hit
- Read: `actions.rs:1114-1117` — destructure pattern `(atk_total, pending_dmg, _attacker)` — **attacker field is discarded**
- Cleared: only at turn start (`encounters.rs:366, 450`)

**Symptom:** column is set, never read meaningfully, never cleared mid-combat. Wastes row space, obscures intent. If a future feature reads this for "who hit me" it will see stale data from previous attacks.

**Fix:** either delete the column (it is read-once-and-discarded) or wire Shield/Counterspell/Uncanny Dodge to clear it consistently with `last_hit_attack_total` and `last_hit_damage`.

---

### H5. Counterspell is broken vs PHB
`backend/src/routes/combat/actions.rs:1157-1175`

**Gaps:**
- No automatic success at 3rd+ level slot (PHB: auto-counter without check)
- No ability check (arcana) when slot level < target spell level
- No `target_id` from caller — picks `LIMIT 1` arbitrarily
- No range / line-of-sight / line-of-effect check vs caster
- No protection from countering an ally's spell
- Two-caster race: `cast_spell` sets `spell_being_cast` after tx begin but before commit. Counterspell's `LIMIT 1` picks one arbitrarily, no way to target a specific caster.

**Fix:** require `target_caster_id: Uuid` in request body, fetch caster + caster's snapshot, validate range/LoS, accept `auto_success_level: Option<u8>` for "I cast at Nth level, auto-counter everything ≤ N".

---

### H6. Player can heal any combatant in campaign (including enemy NPCs)
`backend/src/routes/combat/actions.rs:675-757` `heal`

**Symptom:** ownership check is on the **target** combatant's character. A player in the same campaign can heal any NPC owned by the GM.

**Fix:** check that the target combatant is either owned by the caller OR is an ally (party member) by some alliance flag — at minimum gate to "heal own or party".

---

### H7. Multi-statement mutations outside transactions (5 handlers)
- `bulk_add_combatants` — `combatants.rs:240-298` (N inserts, no tx)
- `update_combatant` — `combatants.rs:341-413` (combatant + sheet sync, no tx)
- `shove` — `special.rs:152-254` (3 separate `&s.db` calls)
- `class_feature` — `special.rs:990-1116` (3+ statements per branch)
- `overlay_damage` — `tactical.rs:473-592` (loop, no tx)
- `cast_spell` post-commit clear — `spells.rs:511-514` (`spell_being_cast = null` outside tx)
- `auto_trigger_ready_actions_for_event` — `actions.rs:1562-1616` (loop of UPDATEs)
- `goto_turn` — `encounters.rs:432-437` (count read outside tx, turn write inside)

**Symptom:** partial state on failure. The most damaging: `cast_spell` commits the spell, then crashes before clearing `spell_being_cast` — Counterspell's `LIMIT 1` returns that stale slug and incorrectly "counters" an already-cast spell.

**Fix:** wrap in `s.db.begin()` with reborrow, or compensate for the most critical paths first.

---

## 2. HIGH Severity — Frontend

### H8. ~20+ unguarded action buttons (double-click = double-action)
`web/src/routes/campaigns/[id]/initiative/+page.svelte`

Lines (per audit): 1523, 1565, 1576-1580, 1636/1639/1642/1649/1654, 1673, 1912, 1955/1956, 1986, 2027, 2097, 2146, 2167, 2188, 2235, 2302, 2360, 2383, 2386, 2443, 2552, 2557, 2561, 2594, 2676, 2683-2707, plus 5 in `EffectPanel.svelte`.

**Symptom:** rapid clicks on Attack / Damage / Cast / Dodge / Disengage / Multiattack / Overlay Damage / etc. fire multiple server requests. Only `rollInitiativeFor` (line 2488) correctly guards with `disabled={rolling[cid]}`.

**Fix:** add a per-action `inFlight: Set<string>` (or `Map<buttonId, boolean>`) and disable buttons while their request is pending.

---

## 3. MEDIUM Severity — Data Integrity

### M1. `legendary_action` TOCTOU
`backend/src/routes/combat/special.rs:484-528`

```rust
let e = ... fetch_optional ...; // read used+max
if used >= max { return Err(...); }
sqlx::query("UPDATE ... SET used = used + 1 ...") // unconditional
```

**Symptom:** two concurrent legendary actions both pass the `used < max` check, both write, exceeding max. `use_action` handler (combatants.rs:631) uses atomic `least(max, used+1)` correctly.

**Fix:** use the same atomic `least` pattern.

---

### M2. `lair_action` TOCTOU
`backend/src/routes/combat/special.rs:453-476`

```rust
if e.lair_action_used { return Err(...); }
sqlx::query("UPDATE ... SET lair_action_used = true ...") // unconditional
```

**Fix:** `UPDATE encounters SET lair_action_used=true WHERE id=$1 AND lair_action_used=false RETURNING id` and check the row.

---

### M3. GM/NPC `move_combatant` path has no movement cap
`backend/src/routes/combat/combatants.rs:560-571`

Player branch (`:541-559`) uses atomic `WHERE movement_used_ft + $4 <= $5`. GM/NPC branch is unconditional.

**Symptom:** NPC can exceed speed cap. Inconsistent with player.

**Fix:** add the same atomic cap check.

---

### M4. `hp_max_reduction` is computed but never persisted
`backend/src/combat_engine.rs:873-876` reads reduction; `actions.rs:1015, 1042` overwrite with raw `hp.max` on every sync; `characters.rs:397-409` and `combatants.rs:348-411` ignore it.

**Symptom:** Wounded condition / wound-target effect silently dropped on every combat→sheet→combat round trip.

**Fix:** make `hp_max_reduction` a real persistent column (already present in schema per audit) and propagate it through all sync paths.

---

### M5. Long rest resets `death_saves` in sheet but not in linked combatant
`backend/src/routes/characters.rs:761-763` writes `death_saves{0,0}` to character. No push to linked combatant.

**Fix:** after sheet update, if a linked combatant exists, write `death_saves` to combatant JSONB too.

---

### M6. 11 `sync_combatant_hp_to_sheet` calls are warn-only on failure
`actions.rs:511, 647, 743, 857, 1527, 1928`; `special.rs:855, 1009, 1096`; `tactical.rs:568`.

**Pattern:**
```rust
if let Err(e) = sync_combatant_hp_to_sheet(&s.db, ...).await { tracing::warn!(...) }
```

**Symptom:** combatant and character sheets silently desync.

**Fix:** at minimum, return a 500 with a clear "sheet sync failed" error so the UI can retry; or queue for retry.

---

### M7. `goto_turn` TOCTOU on `rolled` count
`backend/src/routes/combat/encounters.rs:432-437` reads `e.rolled` outside tx, then writes turn_index inside. Two concurrent goto/next calls can both observe the same `rolled` value and both write conflicting turn_index.

---

### M8. `cast_spell` `spell_being_cast` clear outside tx
`backend/src/routes/combat/spells.rs:511-514`

If process dies between tx.commit (`:511`) and clear (`:513`), field stays set until turn_start, blocking all future Counterspell.

**Fix:** move clear inside the tx or set up a periodic janitor that clears stale `spell_being_cast` rows.

---

### M9. Shield retroactive HP restore ignores `hp_max_reduction`
`backend/src/routes/combat/actions.rs:1147` — `new_hp = (cur + dmg).min(snap.hp_max)`. If target had `hp_max_reduction` (wound), restore over-fills HP.

**Fix:** use effective max `snap.hp_max - reduction` (or read it from combatant row directly).

---

### M10. `last_hit_damage` not cleared by Uncanny Dodge
`backend/src/routes/combat/special.rs:1102-1116` reads `last_hit_damage` but does not clear it. Subsequent Shield in same round reads stale undiminished damage and over-restores HP.

---

### M11. `last_hit_attack_total` overwritten on each new hit
`backend/src/routes/combat/actions.rs:435` — overwritten on every hit. If two attackers hit same target in one round, Shield retro-negates the wrong attack.

**Fix:** capture `last_hit_attack_total` + attacker id at the time of the `reaction_window` WS event (don't overwrite until the window closes or turn ends).

---

### M12. `target_enters_range` misnamed — fires on every move
`backend/src/routes/combat/combatants.rs:581-582` passes `event_type="target_enters_range"`. The handler `auto_trigger_ready_actions_for_event` at `actions.rs:1562-1616` has **no range check** — fires on every `move_combatant` call regardless of distance. A readier watching the mover fires on every move.

**Fix:** rename to `target_moves` OR add actual range check against `watch_distance_ft` in the trigger JSONB.

---

### M13. Readied action persists indefinitely
`backend/src/routes/combat/actions.rs:1634-1692` `ready_action` has no turn/round limit. A readied action from round 1 still fires in round 10 if its trigger condition occurs.

**Fix:** set `expires_at: round+1` on the readied_action JSONB and skip auto-trigger if `current_round > expires_round`.

---

### M14. `cast_spell` post-commit race with Counterspell
`backend/src/routes/combat/spells.rs:360-514` — `spell_being_cast` is set inside tx at `:379-380`, commit at `:511`, clear at `:513-514` (outside tx). A Counterspell arriving between commit (`:511`) and clear (`:513`) reads the value, clears it, then `:513`'s UPDATE is a no-op. A Counterspell arriving after `:513` reads null and rejects — even though the spell was actively cast. Window is ~1ms in practice but races exist.

---

### M15. 41 of 55 combat WS events use past tense (violates §5.3)
Per AGENTS.md §5.3, events must be `snake_case` + **present tense**. 0/55 comply. Examples: `combatant_attacked`, `combatant_damaged`, `combatant_dodged`, `encounter_started`, `combatant_grappled`, `concentration_broken`, `effects_changed`.

**Fix:** future refactor: rename to `combatant_attacks`, `combatant_damages`, `combatant_dodges`, `encounter_starts`, `combatant_grapples`, `concentration_breaks`, `effects_change`. ~41 emit sites + all clients.

---

### M16. `cast_spell` no check that `e.status == "active"`
See H3. Also missing: prep check for known-spell classes (Sorcerer/Bard/Warlock/Ranger/Rogue) per AGENTS.md §10.7. Per audit, prep check exists for Wizard/Cleric/Druid/Paladin/Artificer but the others "skip" prep — but no `character_spells.known` lookup happens. Players with slots can cast any spell in DB.

**Fix:** add `character_spells.known` lookup; only enforce `prepared` for prepared-caster classes.

---

### M17. `class_feature "lay_on_hands"` target_id not validated
`backend/src/routes/combat/special.rs:1056-1095` — `body.target_id` is not checked to belong to the same encounter as the caster.

**Fix:** add encounter-id equality check.

---

### M18. `computed_stats` no encounter ownership check
`backend/src/routes/combat/actions.rs:990-1002` — any campaign member can read stats for any combatant in any encounter of that campaign. Cross-campaign leak risk if user has multiple campaigns and a UUID leaks.

**Fix:** verify the combatant's encounter is in a campaign the caller is a member of (the `require_member` check passes if caller is in the same campaign, but doesn't confirm the encounter is in *that* campaign — usually fine, but be explicit).

---

### M19. 6+ missing `confirm()` prompts on destructive ops
Per audit §7:
- `+page.svelte:487` — `end()` (entire encounter)
- `+page.svelte:1380-1398` — `placeAllTokens()` (bulk reposition)
- `+page.svelte:1364-1378, 2982` — `placeTokenAtCentre` with `removeFromMap` (silently removes token)
- `+page.svelte:2651` — `setMapImage(null)` (clears map)
- `EffectPanel.svelte:40-75` — `addEffect`, `applySpell`, `removeEffect` (no confirm)

---

### M20. `unconscious` characters can be re-added to encounters
`backend/src/routes/combat/combatants.rs:181-186` checks `sheet.alive=false` but not `death_saves.failures >= 3`. A failed-death-save character can be re-added.

---

### M21. Frontend `NpcStatBlock.svelte` has zero `$_()` calls
~80 hardcoded English strings (ability labels, save proficiencies, traits, attacks, legendary actions, etc.). See audit §1 for full list.

---

### M22. List rendering not virtualized
Roster can hit 30+ combatants. Every WS event triggers `loadList()` → full array re-render. `effectsFor(c)` recomputed per-row per-render (O(rows × effects)).

**Fix:** add a virtualized list (svelte-virtual or similar) for the roster when count > N.

---

## 4. LOW Severity — Code Smell / Style

### L1. File sizes over 500-line limit (AGENTS.md §1.4)
| File | Lines | Status |
|---|---|---|
| `actions.rs` | 2,349 | CRITICAL — split or extract submodules |
| `combat_engine.rs` | 2,572 | CRITICAL — extract `resolve_*` to submodules |
| `tactical.rs` | 1,145 | OVER — split overlays/cover/condition |
| `special.rs` | 1,139 | OVER — split grapple/shove/legendary/lair |
| `combatants.rs` | 654 | OVER — extract bulk_add, use_action |
| `spells.rs` | 542 | OVER — `cast_spell` itself is 479 lines |
| `+page.svelte` | 4,464 | CRITICAL — extract to `lib/combat/` |

### L2. ~40 functions > 50 lines (AGENTS.md §1.4)
Top offenders: `cast_spell` 479, `attack` 407, `compute_stats` 287, `tick_effects` 241, `class_feature` 185, `two_weapon_fight` 161, `load_snapshots_batch` 155, `multiattack` 154, `load_snapshot` 148, `apply_modifier` 146.

### L3. `combat/mod.rs:171` silently swallows `notify_turn` DB error
```rust
fetch_optional(&s.db).await.ok().flatten()
```
Should at least `warn!` on Err.

### L4. `actions.rs:1177` empty match arm in `react`
```rust
match body.reaction_type { ... _ => {} }
```
Falls through silently for custom reaction types.

### L5. Free object interaction (PHB p.190) intentionally absent
No tracking of "1 free object interaction per turn". Acceptable per design but should be documented as intentional gap.

### L6. Redundant `let _ =` after `?`
- `actions.rs:1891` `let _ = decrement_thrown_weapon(...).await?;`
- `events.rs:131` `let _ = sqlx::query(...).execute(...).await?;`
Cosmetic.

### L7. 41 past-tense WS event names (see M15)

### L8. 3 redundant `let _ =` + `?` patterns
Cosmetic.

### L9. `Encounters.move` accepts any `f32` distance
`+page.svelte:495-516` — `dash_bonus` and `forced_ft` summed from `distance_ft` without bounds. Negative values allow "backwards" movement cap.

### L10. Unbounded JSON input
`damage_expression: String`, `attack_expression: String`, `upcast_level: Option<i32>` (no cap on upcast level), `spell_being_cast` no length cap.

### L11. `uncanny_dodge` divides by 2 on negative `last_dmg`
`special.rs:1109-1113` — `last_dmg.unwrap_or(0) / 2`. If heal was applied to `last_hit_damage` (it isn't, but the field shape allows it), result is negative and HP would be "restored" by a negative number — actually heals the attacker. Edge case but should clamp to `max(0, dmg)`.

### L12. `rage` default `barbarian_level=1` on query miss
`special.rs:1017-1027` — player without barbarian levels could rage with default +2 damage bonus.

### L13. `savage_attacks` parser error → 0
`combat_engine.rs:1581-1584` — `roll(...).map(|r| r.total).unwrap_or(0)`. Silent zero on error.

---

## 5. Test Coverage Gaps (15 untested / 4 weak)

| # | Mechanic | Severity | Notes |
|---|---|---|---|
| 1 | **BA+Action spell restriction (PHB p.203)** | HIGH | `action_spell_level`/`bonus_action_spell_level` columns exist, no regression test |
| 2 | **Combatant↔character sync (HP/AC writeback)** | HIGH | `sync_combatant_hp_to_sheet` never asserted in any test |
| 3 | **Sneak attack (advantage + once/turn)** | HIGH | Only generic `extra_damage_expression` covered, not the Sneak-specific flow |
| 4 | **Reckless attack** | MED | `reckless` field in `AttackReq`, no test |
| 5 | **Concentration: one spell at a time** | MED | Cast doesn't assert `concentration_spell` clear |
| 6 | **Temp HP stacking rule** | MED | "only apply if higher" rule not tested |
| 7 | **Condition immunity (creature type)** | MED | Undead/construct immunities never exercised |
| 8 | **Legendary resistance** | MED | Auto-pass mechanic not tested |
| 9 | **Regeneration** | MED | `hp_regen_per_turn` modifier not tested |
| 10 | **Ritual casting** | MED | `cast_as_ritual: true` flag not tested |
| 11 | **Spell component M** | MED | Material consumption not tested |
| 12 | **Readied action auto-trigger** | MED | `auto_trigger_ready_actions_for_event` never invoked |
| 13 | **Reset action/BA/reaction/movement on turn start** | MED | Turn reset is implicit/untested |
| 14 | **Spell range validation (token distance)** | MED | `range_text` parse + distance check not tested |
| 15 | **Initiative rolling (set-initiative endpoint)** | MED | Endpoint never tested |
| 16 | **Fighting style: defense (+1 AC)** | LOW | 4 of 5 styles covered |
| 17 | **Walls + vision + auras** | LOW | Battle map visibility layer entirely uncovered |
| 18 | **Lair initiative count = 20 (DMG)** | LOW | Lair actions test exists but slot-20 rule not asserted |

**Weak coverage (smoke only):**
- Shield reaction — never verifies +5 AC actually applied
- Counterspell — never verifies the spell was actually countered
- Rage — only verifies condition flag set, not BPS resistance or damage bonus
- Lair actions — only verifies endpoint accepts, not reset or initiative 20
- Hidden reveal on attack — `modifiers.hidden = false` clear on attack not asserted
- Grappling release on incapacitate — manual release tested, condition-driven release not

**Recommended test additions** (in priority order):
1. BA+Action spell restriction (1 integration test in `combat_integration.rs`)
2. Combatant↔character HP sync (1 test in `combat_integration.rs`)
3. Sneak attack once/turn (1 unit test in `combat_engine_unit.rs`)
4. Reckless attack (1 unit test)
5. Concentration one-at-a-time (1 integration test)
6. Temp HP stacking (1 unit test in `combat_engine_unit.rs`)
7. Legendary resistance (1 unit test)
8. Regeneration (1 unit test)
9. Readied action auto-trigger (1 integration test in `combat_full_integration.rs`)
10. Initiative set (1 integration test)

---

## 6. Frontend Audit Summary

| Area | Status |
|---|---|
| i18n completeness | ❌ ~200+ hardcoded English strings in initiative page + entire `NpcStatBlock.svelte` |
| Svelte 5 runes only | ✅ Clean (no Svelte 4 stores outside i18n + 2 custom stores) |
| Double-click race | ❌ ~20+ unguarded buttons (see H8) |
| State desync | ✅ Server-truth via `loadList()` on all WS; local-patch only for token drag (correct) |
| List virtualization | ❌ Not virtualized; O(rows) re-render on every state change |
| Token bounds | ✅ Clamped 0-100% client-side; safe drag pattern |
| Confirmations | ⚠️ 3 present, 6+ missing (end encounter, placeAllTokens, remove token, remove effect, etc.) |
| File size | ❌ `+page.svelte` at 4,464 lines, far over 500-line cap |

**i18n hot spots** (file:line ranges, all hardcoded):
- Initiative page button labels: lines 1506-1797 (every ca-btn, action chip, etc.)
- Initiative page `<option>` values: lines 1841-1893, 1974-2020, 2227-2341, 2349-2354, 2716-2719
- Initiative page labels: lines 1810, 1823, 1834-1889, 1919-1924, 2001-2032, 2042-2110, 2121-2193, 2201-2307, 2316-2445
- Initiative page dice roller: lines 3176-3215
- Initiative page error strings: lines 936, 963, 977, 1040, 1050, 1068, 1078, 1131, 1132, 1152, 1167, 1186, 1187, 2129
- `NpcStatBlock.svelte` 161-489 (entire component, ~80 strings)
- `EffectPanel.svelte:58, 67, 74` (alert() calls)

---

## 7. Backend Code Smell — Code Path Issues

### Confirmed clean
- **Reborrow correctness:** all 27 transaction sites use `&mut *tx` / `&mut *conn` correctly. AGENTS.md §5.1 satisfied.
- **JSONB lossy casts:** no `.as_i64().map(|v| v as i32)` — all use `.clamp(i32::MIN, i32::MAX) as i32`.
- **§5.5 column presence:** all 6 critical combatant columns (`action_spell_level`, `bonus_action_spell_level`, `last_hit_attack_total`, `last_hit_damage`, `last_hit_attacker`, `spell_being_cast`) present in every full Combatant RETURNING list.
- **Per-turn reset:** turn-start reset at `encounters.rs:366, 450` covers all per-turn fields comprehensively.

### Confirmed broken
- `cast_spell`/`attack`/`opportunity_attack`/`two_weapon_fight` — no `status == active` check
- `last_hit_attacker` — dead column (set, never read, never cleared mid-combat)
- `auto_trigger_ready_actions_for_event` — no range check, fires on every move for `target_enters_range`
- `class_feature "lay_on_hands"` — `target_id` not validated to same encounter
- `heal` — no friendly-only check (can heal enemies)
- `legendary_action` — TOCTOU read-then-write
- `lair_action` — TOCTOU read-then-write
- GM/NPC `move_combatant` — no movement cap
- 11× `sync_combatant_hp_to_sheet` warn-only failures (sheet desync)

### Confirmed weak
- 2 `unwrap`/`expect` in `combat_engine.rs` (H2)
- 1 silent `.ok().flatten()` in `combat/mod.rs:171` (L3)
- 1 empty match arm in `actions.rs:1177` (L4)
- 5 multi-statement non-tx mutations in 5 handlers (H7)
- `cast_spell` post-commit race with Counterspell (M8, M14)

---

## 8. WS Event Coverage Matrix

**Total combat events emitted by backend:** 55 (across 56 emit sites)
**Total events listened to by initiative page:** 53 (caught by `combatant_*` prefix at line 369 + explicit allow-list)

### Orphans (backend emits, frontend doesn't listen)
- `character_updated` (combatants.rs:414) — doesn't match `combatant_*` prefix
- `concentration_broken` (tactical.rs:1092) — doesn't match `combatant_*` prefix

**Impact:** when a character HP is updated via combat, the Characters page doesn't refresh from the combat tab's WS. When concentration breaks, no in-combat UI notification.

### UI stuck (frontend expects, backend doesn't emit)
**None.** All explicit event types in the listener have matching emitters.

### Naming convention (§5.3)
- `snake_case` compliance: 100%
- `present_tense` compliance: **0/55** — see M15. Refactor candidate, not hot fix.

---

## 9. Column-List Alignment (AGENTS.md §5.8)

| Table | Distinct explicit lists | Coverage |
|---|---|---|
| `combatants` (42 cols + `portrait_url` alias = 43) | 21 RETURNING sites, all identical (43 cols) | ✅ All identical |
| `combatants` (41-col variant) | **1 site at `combatants.rs:553-558`** drops `level_override` + `vision_range` (post-§5.5 cols) | ⚠️ Single deviation |
| `encounters` (14 cols) | 11 sites, all 13-col consistent (drops `created_at`) | ✅ Consistent, `created_at` never read |

**`level_override` and `vision_range`:** defined in schema (migrations 20260428000001 etc.), never written by any INSERT or UPDATE in the combat module. Persist as DB defaults (0 / NULL). Schema columns that are never populated by application code = dead schema.

---

## 10. Test Files Reference

| File | Purpose | Lines | Combat mechanics |
|---|---|---|---|
| `combat_integration.rs` (CI) | integration | 8 | attack, spell, react, grapple, heal, lay on hands, death save, shield, counterspell, readied, massive damage |
| `combat_advanced.rs` (CA) | integration | 9 | legendary, lair, multiattack, prone, grappled, restrained, dodge, twf, shove |
| `combat_engine_advanced.rs` (CEA) | unit | 12 | crit, save DC, exhaustion, conditions (many), concentration, unarmored defense, extra damage, sneak attack expression |
| `combat_engine_unit.rs` (CEU) | unit | 4 | archery, dueling, gwf (flag only), twf, exhaustion, hp_max_reduction, attack, concentration |
| `combat_full_integration.rs` (CFI) | full e2e | 5 | dodge, cover, surprise, hide, readied |
| `combat_movement.rs` (CM) | movement | 2 | turn advancement, token positioning, hazard |
| `edge_cases.rs` (EC) | edge | 4 | attack, save for half, prone, concentration, components V/S |
| `more_gaps.rs` (MG) | gap fill | 2 | rage (weak), spell prep, hazard, token positioning |

**Coverage score: 28/47 fully tested (60%) · 4/47 weakly tested (8%) · 15/47 untested (32%)**

---

## 11. Recommended Fix Order

### Sprint 1 (data integrity)
1. H2 — replace `unwrap`/`expect` in `combat_engine.rs` (1-2h, low risk)
2. H1 — fix `bulk_add_combatants` error handling (1-2h, low risk)
3. H3 — add `e.status == "active"` check to `cast_spell`/`attack`/`opportunity_attack`/`two_weapon_fight` (1h, low risk)
4. M1, M2 — atomic legendary/lair action pattern (1h, low risk)
5. M3 — GM/NPC movement cap (30min, low risk)
6. M6 — at least warn-loud on `sync_combatant_hp_to_sheet` failure (1h, low risk)

### Sprint 2 (PHB correctness)
7. M16 — known-spell class prep lookup (4h, medium)
8. M5 — long rest pushes death_saves to combatant (1h, low risk)
9. M4 — persist `hp_max_reduction` through sync paths (2h, medium)
10. M9, M10, M11 — Shield/Uncanny Dodge/HP restore edge cases (3h, medium)
11. H5 — Counterspell target_id + auto-success-level (6-8h, high complexity)
12. M12, M13 — ready trigger range check + expiry (2h, medium)

### Sprint 3 (test coverage)
13. Add 10 priority tests from §5 (BA+Action, character sync, sneak, reckless, concentration, temp HP, legendary resist, regen, readied auto-trigger, initiative set) (1-2 days)

### Sprint 4 (frontend)
14. H8 — add `inFlight` guard to ~20 buttons (3-4h, mechanical)
15. M19 — add `confirm()` to 6+ destructive ops (1h)
16. M22 — virtualize roster list (4-6h, requires library decision)
17. M21 + ~200+ i18n strings — extract to en.json/it.json (1-2 days, mechanical)

### Sprint 5 (refactor)
18. L1 — split `actions.rs` (2,349) into 3-4 submodules (1-2 days, high risk)
19. L1 — split `combat_engine.rs` (2,572) into pure-math + I/O (1-2 days)
20. L1 — split `+page.svelte` (4,464) into `lib/combat/` modules (2-3 days, high risk)
21. M15 — rename 41 past-tense WS events (1 day, breaking wire format — coordinate with all clients)

### Lower priority
- L2 — decompose 40+ functions > 50 lines (ongoing)
- L5 — document free object interaction as intentional gap (15min)
- M18, M20, M17 — minor authz/validation fixes (1-2h each)

---

## 12. Files Audited

**Backend** (9,476 lines total):
- `backend/src/routes/combat/mod.rs` (442)
- `backend/src/routes/combat/events.rs` (154)
- `backend/src/routes/combat/encounters.rs` (479)
- `backend/src/routes/combat/combatants.rs` (654)
- `backend/src/routes/combat/spells.rs` (542)
- `backend/src/routes/combat/tactical.rs` (1,145)
- `backend/src/routes/combat/special.rs` (1,139)
- `backend/src/routes/combat/actions.rs` (2,349)
- `backend/src/combat_engine.rs` (2,572)

**Frontend** (~4,500 lines):
- `web/src/routes/campaigns/[id]/initiative/+page.svelte` (4,464)
- `web/src/lib/components/EffectPanel.svelte`
- `web/src/lib/components/EffectBadge.svelte`
- `web/src/lib/components/NpcStatBlock.svelte`
- `web/src/lib/ws.svelte.ts`
- `web/src/lib/dnd/*` (referenced)
- `web/src/lib/stores/*` (verified §9.5 compliance)

**Tests** (8 files, 437-test backend suite):
- All 8 files in `backend/tests/` covering combat

**Migrations** (20 files touching combatants/encounters, latest 20260610000001)

---

*Audit completed 2026-06-16. ~9 hours of investigation across 9 parallel tracks. No code modified.*

---

## Sprint 1 Applied (2026-06-16)

> 7 fixes applied + 28 tests added (437 → 465 passing, 0 warnings, 0 errors).

### Fixes applied

| ID | Issue | Resolution |
|---|---|---|
| H1 | `bulk_add_combatants` silent error swallow | Per-row `BulkAddResult.errors[]` with `added`/`failed` counts |
| H2 | `combat_engine.rs:1841,2145` `unwrap`/`expect` panic | Replaced with `unwrap_or_else` + `error!` log + safe `RollResult` default |
| H3 | No `encounter.status == "active"` check on cast/attack/OA/TWF/legendary | `Conflict("encounter not active")` early-return in all 5 handlers |
| M1 | `legendary_action` TOCTOU | Atomic `UPDATE ... WHERE used < max RETURNING` |
| M2 | `lair_action` TOCTOU | Atomic `UPDATE ... WHERE lair_action_used = false` |
| M3 | GM/NPC `move_combatant` no cap | `movement_used_ft = least($cap, used + cost)` |
| M6 | 11 sheet-sync warn-only failures | Upgraded to `error!` with structured `combatant_id` field |

### Tests added (28 total)

- **BA+Action spell restriction** (integration)
- **Combatant → character sheet HP writeback** (integration)
- **set-initiative endpoint** (integration, was untested)
- **attack-in-planned-encounter rejected** (integration, fixes H3 regression)
- **bulk_add row-level error surfacing** (integration, fixes H1 regression)
- **GM NPC move capped at speed** (integration, fixes M3 regression)
- **legendary_action atomic cap exhausted** (integration, fixes M1 regression)
- **lair_action already-used rejected** (integration, fixes M2 regression)
- **Sneak attack extra damage applied** (unit, was untested for once/turn)
- **Reckless attack advantage flag** (unit, was untested)
- **Temp HP absorbs damage** (unit, was untested for PHB stacking rule)
- **Legendary resistance save + max=3** (unit, 2 new)
- **Regeneration modifier contract** (unit, 2 new)
- **Concentration one-at-a-time overwrite** (unit)

### Remaining open items (Sprint 7+)

- **L2** — combat_engine.rs 2,585 lines (2nd-largest file, never split)
- **M15** — 41 past-tense WS event names (breaking wire format refactor) — needs explicit user signoff
- **M21b** — ~100+ remaining hardcoded English strings in frontend (ability chips, dice roller, map, chat)

---

## Sprint 6 Applied (2026-06-16)

> 2 partial fixes (L1b + M21b). No new tests (audit-style refactor).

### Fixes applied (2)

| ID | Issue | Resolution |
|---|---|---|
| L1b | actions.rs 2,038 lines (4.1× cap) | Extracted `actions/combat.rs` (952: attack/deal_damage/heal/death_save/skill_check/roll_save/computed_stats) and `actions/economy.rs` (950: dodge/disengage/help_action/opportunity_attack/delay_turn/two_weapon_fight/dash/hide/contested_hide/search_action/use_object). actions.rs now 14 lines (re-export shim). **Total reduction: 99%** |
| M21b | NpcStatBlock had ~80 hardcoded English strings | 49 strings extracted to `npcs.*` namespace (en.json + it.json): ability labels (str/dex/con/int/wis/cha), stat labels (HP Max, AC, Speed, Prof, CR, XP), section labels (Saves, Skills, Weapons, Actions, Traits, Reactions, Legendary Actions, etc.), placeholders (Damage, Type, Props, To hit, Description), "+ Add" buttons, sense labels |

### Verification

- `cargo test`: 479 passed / 0 failed
- `bunx svelte-check`: 0 errors, 0 warnings
- actions.rs: 2,038 → 14 lines (-99%)
- New file sizes: combat.rs 952, economy.rs 950, reactions.rs 334, sync.rs 88
- i18n additions: 49 keys × 2 locales = 98 entries in `npcs.*` namespace

### Why no tests

- L1b (file split) — pure refactor, all existing tests pass
- M21b (i18n extraction) — pure refactor, no behavior change


---

## Sprint 5 Applied (2026-06-16)

> 3 partial fixes (M19b + M21 + L1). No new tests (audit-style changes).

### Fixes applied (3)

| ID | Issue | Resolution |
|---|---|---|
| M19b | EffectPanel addEffect/applySpell/removeEffect had no `confirm()` | 3 new i18n keys × 2 locales; `confirm()` before each mutation |
| M21 | Hardcoded damage types, abilities, cover, trigger events in +page.svelte | 24 strings extracted to en.json/it.json; +page.svelte uses `$_('initiative.damage_type_slashing')` etc. |
| L1 | actions.rs 2,401 lines (4.8× cap) | Extracted `actions/sync.rs` (88 lines: sync_combatant_hp_to_sheet*, refresh_combatant) and `actions/reactions.rs` (334 lines: react, auto_trigger_ready_actions_for_event, ready_action + structs). actions.rs → 2,038 lines |

### Verification

- `cargo test`: 479 passed / 0 failed
- `bunx svelte-check`: 0 errors, 0 warnings
- actions.rs: 2,401 → 2,038 lines (-363, -15%)
- New files: actions/sync.rs (88), actions/reactions.rs (334)

### Why no tests

- M19b (UI confirm) — covered by existing svelte-check + manual visual check
- M21 (i18n extraction) — pure refactor, no behavior change
- L1 (file split) — pure refactor, all existing tests pass


---

## Sprint 4 Applied (2026-06-16)

> 3 fixes (H5b + H8 + M19 partial) + 3 tests (476 → 479).

### Fixes applied (3)

| ID | Issue | Resolution |
|---|---|---|
| H5b | Counterspell: low-slot counter auto-failed (no ability check) | `ReactBody` adds `ability_check_total: Option<i32>`; backend validates vs `DC = 10 + target_spell_level`; client rolls d20+mod+prof and passes total |
| H8 | ~20+ combat buttons fire-and-forget HTTP/WS (double-click double-action risk) | New `actionInFlight: Set<string>` + `guarded(key, fn)` helper; 5+ critical buttons guarded (start/end/next/prev/removeEncounter/useAction×3/legendary/placeAll/clearMap) |
| M19 | 4 destructive ops missing `confirm()` | end_encounter, place_all_tokens, clear_map, remove_token — added i18n keys (EN + IT) + `confirm()` prompts |

### Tests added (3)

- `counterspell_ability_check_success` (H5b)
- `counterspell_ability_check_failure` (H5b)
- `counterspell_low_slot_requires_ability_check` (H5b)

### Counts after Sprint 4

- Backend tests: 479 passing (was 437 → 465 → 472 → 476 → 479)
- svelte-check: 0 errors, 0 warnings
- +page.svelte: 4,504 lines (was 4,464)
- en.json + it.json: 4 new keys each


---

## Sprint 3 Applied (2026-06-16)

> 2 fixes (H5 + M16) + 4 tests (472 → 476) + 1 migration.

### Fixes applied (2)

| ID | Issue | Resolution |
|---|---|---|
| H5 | Counterspell: no target_id, no LoS, no auto-success at slot level, arbitrary LIMIT 1 pick, no ability check | `ReactBody` adds optional `target_caster_id: Uuid` + `slot_level: i32`; auto-success check (slot ≥ target_spell_level); specific caster clear; old `None` behavior preserved as backward compat. Ability check still deferred — low-slot counters return 400 with explanatory message. |
| M16 | Known-spell casters could cast any spell in DB — no `character_spells.known` check | New `known boolean` column; `cast_spell` checks `known = true` for known casters (Sorcerer/Bard/Warlock/Ranger/Rogue), `prepared = true` for prepared casters (Wizard/Cleric/Druid/Paladin/Artificer) |

### Migration

- `migrations/20260616000002_character_spells_known.sql` — adds `known boolean NOT NULL DEFAULT false` to `character_spells`

### API change (backward compat)

`POST /api/v1/combatants/{id}/react` now accepts:
- `target_caster_id: Uuid` (optional) — which caster's spell to counter
- `slot_level: i32` (optional) — auto-success if ≥ target spell level

Old behavior preserved when fields absent.

### Tests added (4)

- `known_spell_class_rejects_spell_not_in_known_list` (M16)
- `counterspell_target_caster_id_auto_success_at_matching_slot` (H5)
- `counterspell_rejects_low_slot_level` (H5)
- `counterspell_target_not_casting_returns_400` (H5)

### Counts after Sprint 3

- Backend tests: 476 passing (was 437 → 465 → 472 → 476)
- 0 warnings, 0 errors
- Combat module: ~9,950 lines (was 9,802)
- Pending migrations: 2 (pending_hits, character_spells.known)

### H5 remaining work (deferred to Sprint 4)

- Counterspell ability check (Arcana DC = 10 + target_spell_level) for low-slot counters
- Line-of-sight validation (target caster not behind wall)
- Cross-encounter protection (already implicit via `encounter_id` filter)
- Spell-slug validation (target's `spell_being_cast` should resolve to a real spell)

### Frontend work still needed (separate frontend sprint)

- Update `+page.svelte` Counterspell dialog to pass `target_caster_id` + `slot_level`
- Update frontend spell-prep UI to mark `known` vs `prepared`


---

## Sprint 2 Applied (2026-06-16)

> 9 fixes applied (desync cluster) + 7 tests (465 → 472) + 1 migration.

### Fixes applied (9)

| ID | Issue | Resolution |
|---|---|---|
| M4 | `hp_max_reduction` dropped on combat→sheet / char→combatant sync | `sync_combatant_hp_to_sheet` writes raw=max+reduction; char→combatant applies reduction; Shield/UD use effective max |
| M5 | Long rest left dying/unconscious on combatant | Sync query filters out `unconscious*`, `dying`, `stable`, `dead` conditions + refills HP |
| M9 | Shield restore ignored `hp_max_reduction` | Reads `sheet_raw.hp_max_reduction`, computes effective_max |
| M10 | Uncanny Dodge didn't clear `last_hit_damage`, didn't cap at effective max | Now reads from `pending_hits` queue (FIFO), caps HP at `hp_max - reduction`, clears consumed hit |
| M11 | `last_hit_attack_total` overwritten on multi-hit rounds | New `pending_hits jsonb` queue; attack appends `{attacker_id, attack_total, damage, round}`; Shield/UD pop last; turn_start clears |
| M12 | `target_enters_range` fired on every move (no range) | Distance check using `map_grid_size` + `watch_distance_ft` (default 5ft) |
| M13 | Readied actions persisted forever | `set_at_round` + `expires_at_round = set+1`; cleared on round advance (PHB end-of-next-round) |
| M17 | `lay_on_hands` allowed cross-encounter targets | Encounter_id equality check (BadRequest 400) |
| M18 | `computed_stats` cross-campaign leak | Already enforced by `require_member(uid, combatant_campaign_id)`; test added to pin |

### Migration

- `migrations/20260616000001_pending_hits_queue.sql` — adds `pending_hits jsonb NOT NULL DEFAULT '[]'::jsonb`

### Tests added (7)

- `long_rest_clears_dying_condition_on_linked_combatant` (M5)
- `combat_damage_sync_preserves_hp_max_reduction` (M4)
- `pending_hits_queue_accumulates_and_pops` (M11)
- `target_enters_range_skipped_when_distance_too_far` (M12)
- `readied_action_expires_on_round_advance` (M13)
- `lay_on_hands_rejects_target_in_different_encounter` (M17)
- `computed_stats_rejects_non_member` (M18)

### Counts after Sprint 2

- Backend tests: 472 passing (was 437, then 465 after Sprint 1)
- 0 warnings, 0 errors
- Combat module: 9,802 lines (was 9,476 — 326 added: struct field + 20 RETURNING lists + 1 migration)

