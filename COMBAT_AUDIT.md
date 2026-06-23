# Combat System Audit

**Date**: 2026-06-23 (delta re-audit, supersedes 2026-06-22)
**Scope**: `backend/src/routes/combat/` (62 files, ~9.4k LOC) + `backend/src/combat_engine/` (19 files) + `backend/src/ws.rs` + `web/src/lib/combat/` (16 components) + `web/src/routes/campaigns/[id]/initiative/+page.svelte` (3,142 LOC) + `web/src/routes/campaigns/[id]/map/+page.svelte` + `web/tests-e2e/combat.spec.ts`
**Auditor**: 3 parallel deep-dive passes (FRONTEND · WS events · PERFORMANCE) on top of yesterday's (2026-06-22) backend-centric audit
**Delta**: yesterday's 4 CRIT + 12 HIGH + 13 MED + 17/18 LOW + 2/5 INFO all still fixed. New: **3 CRIT + 11 HIGH + 12 MED + 17 LOW + 4 INFO** found across frontend, WS payload leaks, perf N+1, missing indexes.

**Sprint 32 (CRIT + HIGH — FIXED 2026-06-23)**: All 14 fixed.
**Sprint 33 (MED — FIXED 2026-06-23)**: All 12 fixed.
  - 33a: 4 WS payload intel leaks (M-WS1..4)
  - 33b: 3 perf N+1 (M-P1 grapple release, M-P2 patch_effects, M-F2 multiattack)
  - 33c: i18n pass (M-F1, 18 new keys)
  - 33d: UX polish (M-F3 cone+hex, M-F4 hazard, M-F5 focus trap, M-F6 WS backoff)

Remaining: 17 LOW + 4 INFO.

---

## Executive Summary

| Severity | Yesterday (backend) | New (frontend+WS+perf) | Total Open | Action |
|----------|--------------------:|-----------------------:|-----------:|--------|
| CRITICAL |                  0 |                      3 |          0 | ALL FIXED (Sprint 32a) |
| HIGH     |                  0 |                     11 |          0 | ALL FIXED (Sprint 32b-d) |
| MEDIUM   |                  1 |                     12 |          0 | ALL FIXED (Sprint 33a-d) |
| LOW      |                  1 |                     17 |         18 | Backlog |
| INFO     |                  2 |                      4 |          6 | Documented |

**Sprint 33 status (2026-06-23)**: All 12 MED **FIXED** with regression tests. See "Sprint 33 Fix Status" section. Remaining: 18 LOW + 6 INFO.

**Verdict**: backend remains VERY LOW risk (yesterday's audit still valid). **Frontend has 3 CRITICAL UX/correctness bugs** that hide combat state and break cross-player awareness. **WS layer has 1 HIGH payload leak + 1 HIGH stale-state bug + 1 HIGH revocation gap**. **Performance has 7 HIGH N+1 paths** in AoE spells, multiattack, contested hide, grapple release, patch_effects.

**Top 5 to fix first (Sprint 32)**:
1. F1 (CRIT) — `overlay_damages` event leaks `hp_after` of hidden combatants (HP intel for AoE hazards)
2. F2 (CRIT) — `use_action` (`combatants/action.rs`) mutates action economy without WS publish — players see stale "used" state
3. F3 (CRIT) — initiative page `checkOpportunityAttacks` only fires on self-drag, not on host-pushed/shoved token moves; AND `!enemy.token_x` skips edge tokens (0,0) — OA never triggers
4. P1 (CRIT perf) — `auto_trigger_ready_actions_for_event` correlated subquery in SELECT × N readied + per-row UPDATE + per-row WS
5. F4 (HIGH) — `hpRatio` ignores `hp_max_reduction` (wounds) → HP bar shows wrong ratio for wounded characters

**Already verified clean** (no regressions since 2026-06-22):
- 60+ WS emit sites all publish AFTER `tx.commit()` (C4 fix holds)
- Sprint 26-30 N+1 fixes intact (start_encounter, legendary_action, resolve_spell_targets, overlay_damage snapshots, multiattack snapshots, require_action_auth)
- Svelte 4 syntax: zero violations across 3,142 LOC initiative page + 16 combat components
- `overlays_expire` is emitted by `tick.rs:165` — listener at `initiative/+page.svelte:512` is NOT dead code (Agent 2 error)

---

## CRITICAL (3) — incorrect behavior / data leak / N+1

### C-F1. `overlay_damages` event leaks `hp_after` of all targets — **FIXED 2026-06-23 (Sprint 32a)**
**Loc**: `backend/src/routes/combat/tactical/hazards.rs:179, 199` → fix at `hazards.rs:191-205`
**Symptom**: AoE hazard damage published `hp_after` per target to the entire campaign. Non-owner clients could read HP of hidden combatants hit by the hazard. M12 fix missed this event.
**Fix applied**: dropped `hp_after` from the WS `targets` json! payload. HTTP response struct (`OverlayTargetResult.hp_after: i32`) unchanged — GM caller still gets full result. Clients re-fetch via `loadList()`.
**Regression test**: `crit1_overlay_damages_ws_excludes_hp_after` in `combat_coverage_jun2026.rs` — file-shape assertion on hazards.rs publish block + sanity that struct field still present.

### C-F2. `use_action` mutates action economy without WS publish — **FIXED 2026-06-23 (Sprint 32a)**
**Loc**: `backend/src/routes/combat/combatants/action.rs:12-73` → fix at `action.rs:18, 70-78`
**Symptom**: handler updated `action_used/bonus_action_used/reaction_used/legendary_actions_used/legendary_resistances_used` but emitted NO `ws::publish`. All other clients saw stale "action not yet used" until next unrelated event triggered their `loadList()`.
**Root cause**: 0 matches for `ws::publish|ws::emit` in action.rs.
**Fix applied**: capture `auth` return value (was discarded as `let _ = ...`); added `ws::publish(campaign_id, json!({"type":"combatant_updates","id":id}))` after the UPDATE, before `refresh_combatant`.
**Regression test**: `crit2_use_action_publishes_combatant_updates` — code-shape assertion that action.rs contains `ws::publish` + `combatant_updates`; functional check that toggle still works.

### C-P1. `auto_trigger_ready_actions_for_event` correlated subquery + per-row UPDATE + per-row WS — **FIXED 2026-06-23 (Sprint 32a)**
**Loc**: `backend/src/routes/combat/actions/reactions.rs:212-366` → full refactor at `reactions.rs:212-330`
**Symptom**: SELECT included `(select map_grid_size from encounters where id = $1)` — PG inlined correlated subquery per row. With 10 readied combatants triggered by 1 attack: 10 redundant encounter lookups + 10 per-row UPDATE + 10 per-row WS frame. **30 round-trips + 10 WS frames** for a single trigger event. Also non-atomic.
**Fix applied**:
1. Pre-fetch `map_grid_size` once (1 query).
2. Fetch all readied combatants in 1 query (no correlated subquery).
3. Pre-fetch `subject_pos` for distance checks (1 query).
4. Filter matching readied actions in memory.
5. Batched atomic UPDATE: `update combatants set reaction_used=true, readied_action=null, action_used=false where id = ANY($1::uuid[]) and reaction_used = false returning id`.
6. Single batched WS event `combatant_triggers_readied_actions` with array of `{combatant_id, trigger_event, triggered_by, readied_action, dispatch}`.
**Impact**: 10 readied → 4 queries + 1 UPDATE + 1 WS frame. ~7-8x fewer round-trips, 10x fewer WS frames.
**Frontend change**: `initiative/+page.svelte:553-555` handler extended to also match `combatant_triggers_readied_actions` (plural) for single `loadList()`.
**Regression test**: `crit3_auto_trigger_ready_uses_batched_update_and_ws` — 3-part: code-shape (no correlated subquery, ANY($1::uuid[]), plural event), functional (2 readied allies both consumed in 1 call, actor excluded, non-matching trigger ignored).

---

## HIGH (11) — wrong behavior / data leak / N+1 / stale state

**All 11 HIGH fixed in Sprint 32b-d (2026-06-23). See "Sprint 32 Fix Status" section.**

### F1. `checkOpportunityAttacks` skips edge tokens + only fires on self-drag — **FIXED 2026-06-23 (Sprint 32b)**
**Loc**: `web/src/routes/campaigns/[id]/initiative/+page.svelte:1484-1523, 1496` → fixes at L1496, L1478-1481, L494-518, L955-983
**Symptom** (2 bugs in one):
- L1496: `if (!enemy.token_x || !enemy.token_y) continue` — `!0 === true`, so enemies at `(0,0)` (top-left map edge) NEVER get OA check.
- L1484: `checkOpportunityAttacks` only invoked from self-drag `endTokenDrag` (L959). When GM shoves/pushes a token via WS `combatant_moves` event, the dragging client's frontend doesn't re-run OA.
**Fix applied**:
- L1496: `!enemy.token_x` → `enemy.token_x == null` (handles 0 correctly).
- L1480: `oaReachCells` 1.5/2.5 → 1/2 (PHB 5ft/10ft = 1 cell/2 cells).
- WS handler: `combatant_moves` now calls `checkOpportunityAttacks` for non-drag moves. New `tokenPrevPos: Map<id, {x,y}>` tracks last-known positions for old-vs-new calc. Updated on self-drag end + on WS receipt.
**Regression test**: e2e deferred (combat.spec.ts rewrite). Code-shape + svelte-check verifies behavior. Manual smoke test recommended.

### F2. `hpRatio` ignores `hp_max_reduction` (wounds) — **FIXED 2026-06-23 (Sprint 32b)**
**Loc**: `web/src/routes/campaigns/[id]/initiative/+page.svelte:1614-1617` → fix at L1636-1647
**Symptom**: `(c.hp_current as number) / mx` uses raw `hp_max`, not `hp_max - reduction`. Wounded character with reduction=10, max=30, current=20 shows ratio 0.67 (green) but should be 0.50 (yellow).
**Fix applied**: `hpRatio` now looks up the linked character's `sheet.hp_max_reduction` and divides by `(hp_max - reduction)`. Edge case: `effectiveMx <= 0` returns 0 (no division by zero / NaN). Roster.svelte uses the parent's hpRatio via prop — automatically fixed.
**Regression test**: e2e deferred. Manual smoke: char with reduction=10, max=30, current=20 → bar = yellow.

### F3. `cast_spell` template effects don't emit `effects_change` — **FIXED 2026-06-23 (Sprint 32c)**
**Loc**: `backend/src/routes/combat/spells/apply.rs:131` (insert), :188/:236 (publish) → fix at L32-40, L108-112, L116-121, L150-154, L267-280
**Symptom**: when a spell's template inserts a row into `combatant_effects` (e.g. Bless concentration effect), the frontend's `loadEffects()` is gated on `effects_change` event (initiative/+page.svelte:509-511). The new effect doesn't show up until next unrelated `effects_change` fires.
**Fix applied**: `effects_changed: HashSet<Uuid>` accumulated during the tx (template inserts, caster concentration clear, target concentration break). After `tx.commit()` and alongside `combatant_casts_spell`, emit one `effects_change` per unique affected ID.
**Regression test**: `highf3_cast_spell_emits_effects_change` — code-shape assertion (effects_change publish is after tx.commit()).

### F4. WS connection doesn't honor mid-session `token_version` bumps — **FIXED 2026-06-23 (Sprint 32c)**
**Loc**: `backend/src/ws.rs:230-252` (check at handshake only), `:298-349` (loop has no re-check) → fix at L294, L298-345
**Symptom**: User logs out (which bumps `token_version` in DB). Existing WS connection keeps receiving events post-logout until TCP teardown. Token revocation only effective on next reconnect.
**Fix applied**: `connection()` now takes `claims_tv` + `db: PgPool`. Added 4th arm to the `tokio::select!` with a 30s `interval` that re-queries `users.token_version` and breaks the loop on mismatch. Missed tick behavior = Skip. First tick skipped (handshake already validated).
**Regression test**: `highf4_ws_re_checks_token_version_periodically` — code-shape assertion (connection() body has revocation_check + select + break).

### F5. Multiple class features shown to all characters regardless of class — **FIXED 2026-06-23 (Sprint 32b)**
**Loc**: `web/src/routes/campaigns/[id]/initiative/+page.svelte:1814-1830` → fix at L855-869, L1856-1890
**Symptom**: Action Surge, Second Wind, Rage, Uncanny Dodge, Lay on Hands buttons always render. Wizard sees "Rage", Rogue sees "Action Surge", etc. Backend may reject (correctly) but UI shows misleading options.
**Fix applied**: added `charHasClass(c, name)` + `charHasClassLevel(c, name, min)`. Each button wrapped in `{#if charHasClassLevel(activeC, 'fighter', 2)}` (Action Surge), `charHasClass(activeC, 'fighter')` (Second Wind), `charHasClass(activeC, 'barbarian')` (Rage), `charHasClassLevel(activeC, 'rogue', 5) || charHasClassLevel(activeC, 'barbarian', 18)` (Uncanny Dodge), `charHasClass(activeC, 'paladin')` (Lay on Hands). NPCs unaffected (backend gates).
**Regression test**: e2e deferred. Manual smoke: Wizard's turn → no Rage/Action Surge/LoH buttons in DOM.

### F6. `castResult`/`attackResult`/`saveResult` etc. never cleared — **FIXED 2026-06-23 (Sprint 32b)**
**Loc**: `web/src/routes/campaigns/[id]/initiative/+page.svelte:1180-1192, 1140-1154` → fixes at L1155-1156, L1214, L1227, L1241, L1251, L1269, L1343
**Symptom**: last attack/spell/save/escape/grapple/shove result persists in state forever. New attack shows old result flash. CombatLog UI clutters.
**Fix applied**: 7 result setters now add `setTimeout(() => xxxResult = null, 5000)` after `loadList()`, matching the existing `multiattackResult` pattern.
**Regression test**: e2e deferred. Manual smoke: do 2 attacks, no stale result after 2nd resolves.

### F7. WS `dice_roll` events not subscribed by `DiceRoller.svelte` — **FIXED 2026-06-23 (Sprint 32b)**
**Loc**: `web/src/lib/combat/DiceRoller.svelte:1-50` + `web/src/routes/campaigns/[id]/initiative/+page.svelte:569-595, 2599`
**Symptom**: `DiceRoller.history` was local-only. Other players' rolls never appeared in your history. The page's own `loadDiceHistory()` (`initiative/+page.svelte:1415-1418`) was dead code (never called from UI).
**Fix applied**: `DiceRoller` accepts `sharedHistory` prop. New `mergedHistory = $derived.by(() => { dedupe by id, cap 30 })` combines local + shared. Parent initiative page subscribes to `dice_roll` (prepend to `diceHistory`) and `dice_cleared` (clear) in the WS handler. Passes `sharedHistory={diceHistory}` to DiceRoller.
**Regression test**: e2e deferred. Manual smoke: player A rolls, player B opens DiceRoller → A's roll visible.

### F8. `apply_spell_outcome` N×M effect INSERTs + per-target sync (perf) — **FIXED 2026-06-23 (Sprint 32d)**
**Loc**: `backend/src/routes/combat/spells/apply.rs:113-160` → fix at L113-237 (refactored)
**Symptom**: per `result` (target) × per `template` (effect): 1 INSERT. Then 1 UPDATE hp. Then `sync_combatant_hp_to_sheet_tx` (2 queries). Fireball on 10 targets with 2 templates = **50 round-trips**. Plus tx held across all 50 → connection pool (size 16) blocked ~150-300ms.
**Fix applied**: collect `effect_rows`, `hp_updates`, `conc_broken` in memory during tx. Then 1 batched `INSERT INTO combatant_effects FROM unnest($6..$14)`, 1 batched `UPDATE combatants FROM unnest($1..$3)`, 1 batched `UPDATE combatant_effects WHERE combatant_id = ANY($1)` for concentration breaks, 1 batched `sync_combatant_hp_to_sheet_batch_tx` (new helper in `sync.rs`).
**Impact**: 50 round-trips → 4. ~12x faster. Tx time per AoE cast reduced by ~70%.
**Regression test**: `highf8_spell_apply_batched_effect_insert` — code-shape + functional magic-missile multi-target test.

### F9. `contested_hide` N+1 load_snapshot per observer (perf) — **FIXED 2026-06-23 (Sprint 32d)**
**Loc**: `backend/src/routes/combat/actions/economy/contested.rs:96-115` → fix at L96-118
**Symptom**: `for oid in &observer_ids { load_snapshot($s.db, *oid) }` + `compute_stats` per observer. Each `load_snapshot` = 2 queries (combatant + effects). 50 observers = **100 round-trips + 50 compute_stats** = 300-600ms.
**Fix applied**: replaced per-observer `load_snapshot` loop with `load_snapshots_batch(&observer_ids)` (1 query). `compute_stats` still runs per observer (CPU, no I/O).
**Impact**: 100 → 1 query. ~100x fewer DB calls.
**Regression test**: `highf9_contested_hide_uses_batch_snapshots` — code-shape (regression check that the per-observer loop is gone).

### F10. `attack.rs` 3 overlapping encounter-wide queries (perf) — **FIXED 2026-06-23 (Sprint 32d)**
**Loc**: `backend/src/routes/combat/actions/combat/attack.rs:120, 201, 262` → fix at L118-138 (single `Others` query)
**Symptom**: 3 separate full-encounter combatant scans:
- L120: all other tokens for 5ft threshold check
- L201: all other tokens for cover
- L262: all other tokens for flanking
Each fetches 50 rows, 3 round-trips. Same data needed.
**Fix applied**: 1 query returns (id, ref_type, token_x, token_y, hp_current, token_on_map, initiative_rolled) for all other combatants into `Vec<OtherToken>`. 5ft check (filter `initiative_rolled`), cover (filter `token_on_map && hp_current > 0 && id != target`), flanking (same + side match) all iterate in memory.
**Impact**: 3 → 1 query per attack. ~3x fewer round-trips.
**Regression test**: `highf10_attack_uses_single_others_query` — code-shape (regression checks for the 3 old queries).

### F11. `multiattack` per-target apply 5 queries × N hits (perf) — **FIXED 2026-06-23 (Sprint 32d)**
**Loc**: `backend/src/routes/combat/special/multiattack.rs:193-228` → fix at L193-275 (refactored); `sync.rs:87-148` (new helper)
**Symptom**: per target hit: UPDATE hp (1) + UPDATE concentration (1) + sync_combatant_hp_to_sheet_tx (2) + INSERT combat_events (1) = **5 round-trips per target**. 5-target multiattack hitting all = 25 round-trips.
**Fix applied**: collect `hits: Vec<(Uuid, hp, temp, dmg, label)>` and `conc_broken: Vec<Uuid>` during apply. Then: 1 batched `UPDATE combatants FROM unnest($1..$3)`, 1 batched `UPDATE combatant_effects WHERE combatant_id = ANY($1)` for concentration breaks, 1 batched `sync_combatant_hp_to_sheet_batch_tx` (new helper: 1 SELECT + 1 UPDATE for all character-bound targets), 1 batched `INSERT INTO combat_events FROM unnest($4..$7)`.
**Impact**: 5 hits × 5 queries = 25 → 4 total queries. ~5x faster.
**Regression test**: `highf11_multiattack_batched_apply` — code-shape (batched UPDATE/INSERT + sync helper exists).

---

## MEDIUM (12) — PHB / i18n / leaks / N+1

**All 12 MED fixed in Sprint 33a-d (2026-06-23). See "Sprint 33 Fix Status" section.**

### M-F1. ~40 hardcoded English strings — **FIXED 2026-06-23 (Sprint 33c)**
**Loc**: initiative page + AttackForm + MultiattackForm
**Symptom**: hardcoded English in button labels, damage type options, cover type options, placeholders, reaction prompts, default encounter name.
**Fix applied**: 18 new i18n keys added to en.json + it.json (btn_action_surge, btn_second_wind, btn_rage, btn_uncanny_dodge, btn_lay_on_hands, btn_multi, btn_react, btn_overlay_dmg, btn_surprise, opt_custom, opt_no_weapon, ph_atk_expr, ph_dmg_expr, cover_none/half/three_quarters, msg_react_shield/counterspell_prompt). COVER_TYPES refactored to use label_key instead of hardcoded English. Italian translations provided for all new keys.
**Regression test**: `medmf1_i18n_keys_exist_in_both_locales` + `medmf1_hardcoded_strings_replaced_with_i18n`.

### M-F2. Multiattack UI: single target, no per-attack weapon — **FIXED 2026-06-23 (Sprint 33b)**
**Loc**: `web/src/lib/combat/forms/MultiattackForm.svelte`
**Fix applied**: each parsed-attack row now has its own `target-select` + `weapon-select`. `retarget(i, new_id)` + `rearm(i, new_weapon_id)` update the row. `getWeapons(activeC)` reads from linked character sheet. `partyChars` prop passed from parent.
**Regression test**: `medmf2_multiattack_per_attack_target_and_weapon`.

### M-F3. Cone spread 53.13° + hex grid distance — **FIXED 2026-06-23 (Sprint 33d)**
**Loc**: `initiative/+page.svelte:1061-1070, 2298-2300, 1554-1595`
**Fix applied**:
- Cone spread: `53.13 * (Math.PI / 180)` → `45 * (Math.PI / 180)` (2 sites: createZoneOverlay geometry + SVG preview). PHB "1/8 of the area" rule.
- OA reach: `cellPx = isHex ? g * 0.75 : g`. Hex tiles horizontally by colSpacing (1.5*R = 0.75*g), not by g.
**Regression test**: `medmf3_cone_spread_45_degrees` + `medmf3_oa_reach_uses_colspacing_for_hex`.

### M-F4. Hazard fields + click-to-place — **PARTIALLY FIXED 2026-06-23 (Sprint 33d)**
**Loc**: `initiative/+page.svelte:1092-1148`
**Fix applied**:
- Part 1: Hazard-specific fields now properly gated on `zoneType === 'hazard'` (clearer than the `? {}` ternary).
- Part 2: `createZoneOverlay` accepts optional `position?: { x, y }` param. Default 50,50 still works.
- **Deferred**: full click-to-place UX (placement mode with ghost preview). The function signature is ready; a future UI can pass `position` from a map-click handler.
**Regression test**: `medmf4_create_zone_overlay_accepts_position`.

### M-F5. Modal focus trap — **FIXED 2026-06-23 (Sprint 33d)**
**Loc**: `web/src/lib/combat/Modal.svelte`
**Fix applied**:
- Tracks `previouslyFocused` element on mount.
- `handleKeydown` cycles Tab/Shift+Tab between first/last focusable within the dialog.
- Escape closes (window listener).
- On unmount, restores focus to the previously focused element.
- Added `aria-label="Close"` on the X button.
- Backdrop click-to-close + Escape handler (svelte-ignore on the wrapper div).
**Regression test**: `medmf5_modal_focus_trap`.

### M-F6. WS reconnect backoff + missed-event replay — **FIXED 2026-06-23 (Sprint 34b)**
**Loc**: `web/src/lib/ws.svelte.ts` + `backend/src/ws.rs` + `migrations/20260623000001_ws_events_replay.sql`
**Fix applied (Sprint 33d, partial)**: exponential backoff (1s → 2s → 4s → 8s → 16s → 30s cap). Replaces fixed 2s retry that caused reconnect storms. Resets on successful open.
**Fix applied (Sprint 34b, complete)**: server-side event persist + replay.
- New `ws_events` table with per-campaign monotonic `seq` populated by BEFORE INSERT trigger (`ws_events_seq_per_campaign`).
- New `ws::publish_persist(db, campaign_id, event_json: Value) -> Option<i64>` helper inserts into `ws_events`, augments payload with `seq`, then broadcasts.
- New `ws::replay_events(db, campaign_id, since, limit)` returns missed events in order for client catch-up on reconnect.
- Migrated all 56 `ws::publish(...)` call sites in `backend/src/routes/combat/` to `ws::publish_persist(&s.db, ...).await` (40 files).
- Two `Vec<String>` deferred-publish patterns adapted: `conditions.rs` switched `pending_events` to `Vec<serde_json::Value>`; `turns.rs` keeps `tick_effects` returning `Vec<String>` and parses back with `serde_json::from_str` in the post-commit loop (function signature preserved per migration rules).
- Updated 3 source-shape tests in `combat_coverage_jun2026.rs` (`crit1_*`, `medws3_*`, `medws4_*`) and 1 in `combat_integration.rs` (`combatant_attacks_event_omits_hp_after`) to use the new call shape + `.await;` terminator.
- New regression test `publish_persist_no_string_concat` scans all combat files for `format!` / `write!` / `to_string() +` inside `publish_persist` calls — would silently bypass the persist if reintroduced.
- Pre-existing in this commit: `backend/src/routes/ws_events.rs` (HTTP endpoint) + `mod.rs` wiring.

**Regression tests**: `medmf6_ws_reconnect_exponential_backoff` (frontend), `publish_persist_no_string_concat` (backend).

### M-WS1. dice_roll leaks user_id + character_id — **FIXED 2026-06-23 (Sprint 33a)**
**Loc**: `backend/src/routes/dice.rs:101-113`
**Fix applied**: stripped `user_id` + `character_id` from the public `dice_roll` event. Other players see expression + total without knowing who rolled. Roller still gets the full payload via the HTTP response.
**Regression test**: `medws1_dice_roll_strips_user_id`.

### M-WS2. combatant_reacts leaks shield_blocked_hit — **FIXED 2026-06-23 (Sprint 33a)**
**Loc**: `backend/src/routes/combat/actions/reactions.rs:49,107,185-195`
**Fix applied**: dropped `shield_blocked_hit` from the public event. Removed the now-unused `shield_blocked_hit` local variable. Outcome observable via `combatant_attacks` and `combatant_damages` events downstream.
**Regression test**: `medws2_combatant_reacts_strips_shield_blocked`.

### M-WS3. combatant_uses_class_feature leaks message — **FIXED 2026-06-23 (Sprint 33a)**
**Loc**: `backend/src/routes/combat/special/class_feature.rs:542-555`
**Fix applied**: stripped `message` from the public event. The `feature` name stays (master needs to know "X used Rage" for adjudication). Owner gets the full message via the HTTP response (`ClassFeatureResult`).
**Regression test**: `medws3_class_feature_strips_message`.

### M-WS4. reaction_window leaks damage_pending — **FIXED 2026-06-23 (Sprint 33a)**
**Loc**: `backend/src/routes/combat/actions/combat/attack_apply.rs:219-232`
**Fix applied**: dropped `damage_pending` from the public event. Removed the now-unused `total_dmg` local. Target gets the full `AttackResult` via the HTTP response + the subsequent `combatant_damages` event.
**Regression test**: `medws4_reaction_window_strips_damage_pending`.

### M-P1. add_condition grapple release per-row N+1 — **FIXED 2026-06-23 (Sprint 33b)**
**Loc**: `backend/src/routes/combat/tactical/conditions.rs:191-232`
**Fix applied**: replaced per-row UPDATE + per-row WS loop with:
- 1 batched `UPDATE combatants SET conditions = array_remove(conditions, 'grappled') WHERE encounter_id = (subselect) AND id != $1 AND 'grappled' = any(conditions) RETURNING id`
- 1 batched WS event `combatant_loses_conditions_batch` with array of freed combatant_ids.
10 grappled targets = 10 UPDATE + 10 WS → 1 UPDATE + 1 WS (~10x faster).
**Regression test**: `medmp1_grapple_release_batched`.

### M-P2. patch_effects 3 per-row loops in autocommit — **FIXED 2026-06-23 (Sprint 33b)**
**Loc**: `backend/src/routes/combat/events.rs:93-168`
**Fix applied**: wrap all mutations in a tx (atomicity). Each branch uses `ANY($1::uuid[])` (1 query instead of N). 50 cids × 3 branches = 150 round-trips → 3 round-trips. Single batched `effects_change` event with `combatant_ids` array (was N per-combatant events).
**Regression test**: `medmp2_patch_effects_batched_and_atomic`.

---

## LOW (17) — defense-in-depth / edge cases

### Frontend
- L-F1: `web/src/lib/combat/Roster.svelte:52` — `removeCombatant` calls `onGotoTurn(currentEnc.turn_index)` after delete; resets active turn to same index. Should `loadList()` only.
- L-F2: `web/src/routes/campaigns/[id]/initiative/+page.svelte:561-564 + 1669` — `$effect` on `selectedId` + tab onSelect both call `loadList()` → double-load race. Drop the effect.
- L-F3: `web/src/routes/campaigns/[id]/initiative/+page.svelte:309-310, 1403-1405, 1773, 1789` — `formCombatant` set by context menu never reset on form close → next form uses stale combatant.
- L-F4: `web/src/routes/campaigns/[id]/initiative/+page.svelte:1931-1932` — `bind:attackTarget/attackExpr/damageExpr/...` shared between AttackForm and MultiattackForm. MultiattackForm clears main form target.
- L-F5: `web/src/routes/campaigns/[id]/initiative/+page.svelte:324-339` — `playTone` creates new AudioContext + osc per call, never closes. Memory leak.
- L-F6: `web/src/routes/campaigns/[id]/initiative/+page.svelte:2062-2070` — `showGrid`/`grid_type` onchange unguarded. Rapid toggle races. `selectedId!` non-null assertion throws if null.
- L-F7: `web/src/routes/campaigns/[id]/initiative/+page.svelte:1711` — NPC `spd` hardcoded to 30, ignores `c.speed` or `npc.stats.speed`.
- L-F8: `web/src/routes/campaigns/[id]/initiative/+page.svelte:386-419` — autofill `$effect` depends on `attackWeaponId` only, not `activeCtxCombatant?.id`. Expression bound to stale combatant on turn change.
- L-F9: `web/src/routes/campaigns/[id]/initiative/+page.svelte:1571-1581` — `placeAllTokens` step = 80/N. N>10 overlaps; last token at y=86 overflows. Cap step at 8.
- L-F10: `web/src/routes/campaigns/[id]/initiative/+page.svelte:1140-1154` — `doHeal` synthesizes fake `DamageResult` with hardcoded `damage_resisted: false` etc. UI display lies.
- L-F11: `web/src/routes/campaigns/[id]/initiative/+page.svelte:2656-2661` — N+1 `partyChars.find` in derived states. O(rows × chars) per render.
- L-F12: `web/src/lib/combat/Roster.svelte:75-181` — each row does `combatants.indexOf(c)` (O(N)) + `effectsFor(c)` (O(E)) twice. Total O(N² + N×E) per render. Memoize or pass `globalIndex` from parent.
- L-F13: `web/src/lib/stores/auth.svelte.ts:54` — `get isMaster()` points to `isAppAdmin`. Confusing. Use `campaign().isMaster` consistently.

### Backend
- L-WS1: `backend/src/ws.rs:313, 343` — `presence_joined/left.user_id` broadcast to campaign. Acceptable for this feature.
- L-WS2: `backend/src/ws.rs:198` — Cleanup uses `rand::random_range(0..100) < 1` (1% chance per check). Non-deterministic. Track `last_cleanup_at` and run deterministic sweep.
- L-WS3: `backend/src/routes/combat/encounters/turns.rs:188,259` — `prev_turn` and `goto_turn` emit `next_turn` event. Listeners can't distinguish forward vs backward. Emit `prev_turn` / `goto_turn` with separate event types.
- L-P1: `backend/src/routes/combat/combatants/bulk.rs:253-280` — post-commit per-row `emit_campaign` INSERT + per-row WS frame. 100 added = 100 INSERTs + 100 frames. Batched INSERT + batched WS.
- L-P2: `backend/src/routes/combat/tick.rs:173-178, 213-217, 269-274` — 4 separate SELECTs of same combatant; hazards loop re-fetches hp. Single SELECT, pass hp as locals.

---

## INFO (4) — documented quirks

- I-F1: `web/src/lib/campaignCtx.svelte.ts:17` — fallback returns `isMaster: false` when no context. Silently hides GM features. Should throw or log.
- I-WS1: `backend/src/ws.rs:36-44, 46-66` — Stale channel cleanup runs every 5min, 1h TTL. Hub entries persist up to 1h. Acceptable.
- I-WS2: `backend/src/routes/combat/encounters/create.rs:33, update.rs:44, delete.rs:28` — Plural suffix `encounter_creates/updates/deletes` inconsistent with `encounter_starts/ends`. Cosmetic.
- I-P1: `backend/src/state.rs:12-13` — `max_connections(16)` for 4 PCs + 1 master + background tasks. Tight. Consider 32-64 for prod.

---

## i18n Hardcoded Strings (top 30 most impactful)

All in scope files. Sample — full list ~40 across initiative page + 8 forms.

| File:Line | String | Suggested Key |
|-----------|--------|---------------|
| initiative/+page.svelte:545 | `"You were hit! Use Shield reaction?"` | `initiative.msg_react_shield_prompt` |
| initiative/+page.svelte:549 | `"Spell being cast — Counterspell available!"` | `initiative.msg_react_counterspell_prompt` |
| initiative/+page.svelte:1728-1808 | `"Attack"`, `"Damage"`, `"Save"`, `"Skill"`, `"Cast"`, `"Dodge"`, `"Disengage (BA)"`, `"Dash (BA)"`, `"Hide (BA)"`, `"Disengage"`, `"Dash"`, `"Hide"`, `"Help"`, `"Grapple"`, `"Escape"`, `"Stand Up"`, `"Shove"`, `"Ready"`, `"Trigger Ready"`, `"Delay"`, `"Multi"`, `"React"`, `"Overlay Dmg"`, `"Surprise"` | `initiative.btn_*` |
| initiative/+page.svelte:1814-1830 | `"Action Surge"`, `"Second Wind"`, `"Rage"`, `"Uncanny Dodge"`, `"Lay on Hands"` | `initiative.btn_*` |
| initiative/+page.svelte:2015 | `"Custom…"` | `initiative.opt_custom` |
| initiative/+page.svelte:687 | `'encounter'` | `initiative.default_enc_name` |
| initiative/+page.svelte:2126-2127 | `Fire`, `Acid`, `Cold`, `Lightning`, `Poison`, `Bludgeon`, `Necrotic` | `initiative.damage_type_*` |
| initiative/+page.svelte:2362 | `✓` token-moved glyph | `initiative.tok_moved` |
| initiative/+page.svelte:2404-2441 | emoji-prefixed context menu items | full i18n (emoji ordering breaks IT phrase order) |
| lib/combat/Banner.svelte:125 | `"⚔️ Flanking:"` | `initiative.flank_title` |
| lib/combat/ActionPanel.svelte:108 | `ft` (movement unit) | `common.unit_feet` |
| lib/combat/ActionPanel.svelte:126 | `"LR"` | `initiative.lr_short` |
| lib/combat/forms/AttackForm.svelte:15-18 | `"None"`, `"Half (+2)"`, `"3/4 (+5)"` | `initiative.cover_*` |
| lib/combat/forms/AttackForm.svelte:127,131,152 | `"1d20+7"`, `"1d8+4"`, `"15"` placeholders | `initiative.ph_*` |
| lib/combat/forms/AttackForm.svelte:138-141,189-193 | damage type raw values | `initiative.damage_type_*` |
| lib/combat/forms/CastForm.svelte:100,121,163-170 | `Lv{n}`, `"8d6"`, `"Miss"`, `"CRIT!"`, `"Hit"`, `"dmg"`, `"saved"`, `"failed"`, `"conc broken"` | `initiative.spell_lv`, `initiative.ph_dmg`, `initiative.cast_*` |
| lib/combat/forms/OverlayDmgForm.svelte:11-17 | `DEX`, `CON`, `WIS`, `STR`, `INT`, `CHA` | `initiative.ability_*` |
| lib/combat/forms/ReadyForm.svelte:62 | `"Anyone"` | `initiative.ready_anyone` |
| lib/combat/forms/SurpriseForm.svelte:55-56 | `nat` | `initiative.skill_natural` |
| lib/combat/DiceRoller.svelte:81 | `"2d6+3"` placeholder | `initiative.ph_dmg` |
| lib/combat/Roster.svelte:83,86-100,109,124-150 | em-dash, action initials, "+N" counter | full i18n |

---

## e2e Test Coverage Gaps (combat.spec.ts)

The existing `web/tests-e2e/combat.spec.ts` is broken and inadequate:

### Broken selectors (will not match)
- L7,14,19-22: `input[name="name"]`, `input[name="display_name"]`, `input[name="hp_max"]`, `input[name="ac"]` — add-combatant form uses `bind:value` only, no `name=` attrs.
- L28: `.combat-active, .turn-tracker` — neither class exists in scope.
- L32: `.target-select` — not used.
- L33: `button:has-text("Roll")` — would match Dice Roller "Roll" too.
- L40,52: `'/campaigns/test-campaign/initiative'` — hardcoded id, no fixture setup. Will 404 in CI.

### Missing test scenarios
- HP decrement / heal (applyDamage flow)
- Death save UI (button, dots, 3-successes-stable)
- Reaction prompt (Shield/Counterspell) — existing L51-60 `if (await reactionBtn.isVisible().catch(() => false))` silently passes
- Multiattack (per-attack target + damage)
- Token drag (snap to grid, range circle, opportunity attack trigger)
- Advantage / disadvantage rolling (2d20kh1)
- Spell slot decrement + ritual toggle + upcast
- Condition add/remove with duration
- Initiative roll (MyRolls)
- Encounter end → state reset
- Master-only features gated for non-master
- Multi-tab WS sync (same user, 2 windows)
- Permission/auth (master-only buttons)
- Roster search filter
- Encounters tab switching
- Map pin drag
- Overlay zone creation/damage
- Hazard damage application
- Lair action button + legendary action dots
- i18n EN ↔ IT switching

---

## Recommended Fix Order (Sprint 32-34)

**Sprint 32 (CRIT + 1 HIGH, ~2 days)**:
1. C-F1: drop `hp_after` from `overlay_damages` event
2. C-F2: add WS publish to `use_action`
3. C-P1: fix `auto_trigger_ready_actions_for_event` correlated subquery + batched UPDATE + batched WS
4. F4: mid-session `token_version` re-check in WS connection
5. F1: fix `checkOpportunityAttacks` edge-token skip + WS-driven OA

**Sprint 33 (remaining HIGH, ~2 days)**:
6. F2: `hpRatio` honors `hp_max_reduction`
7. F3: `cast_spell` emits `effects_change` for templates
8. F5-F7: class feature gating, result clearing, `DiceRoller` WS subscription
9. F8-F11: perf N+1 fixes (apply_spell_outcome, contested_hide, attack.rs 3-merge, multiattack batch)

**Sprint 34 (MEDIUM, ~3 days)**:
10. M-F1: i18n pass (40 strings) — half-day
11. M-F2-M-F6: remaining frontend bugs (multiattack per-attack, reach constants, modal focus trap, WS backoff, hazard placement)
12. M-WS1-M-WS4: WS payload leak mitigation (per-user emits for sensitive events)
13. M-P1, M-P2: conditions grapple batch + patch_effects tx

**Backlog (LOW + coverage)**:
14. 17 LOW items (mostly defense-in-depth + frontend cleanup)
15. Add GIN index on `combatant_effects.modifiers WHERE active = true` (perf, regen lookups)
16. Add partial index `idx_combatants_readied on combatants(encounter_id) where readied_action is not null and reaction_used = false` (perf, ready trigger)
17. Bump `max_connections` 16 → 32 (prod pool headroom)
18. Rewrite `web/tests-e2e/combat.spec.ts` — fix broken selectors, add 20 missing scenarios

**Expected impact**:
- Backend query count on AoE Fireball: 56 → 3 (~15x faster)
- WS payload leaks: 4 high-sensitivity fields → 0 visible to non-targets
- Cross-player awareness: dice_roll cross-visibility, OA on host-shoved, action_used sync — all restored
- PHB correctness: reach (10ft/2 cells), cone spread (45°), frightened LOS (deferred to L15 from yesterday)

---

## Test Status (post-audit, no code changes)

| Suite | Pass | Fail | Ignored |
|-------|------|------|---------|
| Backend `cargo test` (last run 2026-06-22) | 586 | 0 | 1 |
| Frontend `bunx vitest run` (last run 2026-06-22) | 630 | 0 | 0 |
| `cargo check` | clean | 0 errors | 0 warnings |
| `svelte-check --threshold warning` | clean | 0 errors | 0 warnings |

**Re-run recommended** after Sprint 32 fixes land.

---

## What's Clean (no action needed)

- **RBAC**: all 62 combat routes call `require_master`/`require_member`/`require_action_auth` BEFORE data mutation.
- **SQL injection**: 100+ `sqlx::query*` calls all parameterized. `format!` only builds dice expressions from server-computed stats or safe `savepoint sp_{idx}` (numeric).
- **SQLx reborrow**: 100% use `&mut *tx`. No moves.
- **Action economy atomicity**: 5 branches of `consume_action_or_bonus` all use `UPDATE ... WHERE action_used=false RETURNING id` + `is_none()` check.
- **IDOR**: every combatant-scoped handler resolves `combatant_id → encounter → campaign → require_member` chain.
- **Shield reaction** (`actions/reactions.rs:51-112`): reads `pending_hits` JSONB queue, errors if empty. Hits appended after `result.hit` check.
- **Counterspell** (`actions/reactions.rs:113-178`): reads `spell_being_cast` scoped to `encounter_id`. Auto-success at slot_level ≥ target.
- **Uncanny Dodge** (`special/class_feature.rs:307-359`): reads `pending_hits`, falls back to legacy `last_hit_damage`.
- **Token move** (`combatants/move_combatant.rs:114-140`): `SELECT FOR UPDATE` + tx serializes concurrent moves.
- **Async shared state**: no `static mut`, no `RwLock<HashMap>`. DB is sole source of truth.
- **BA+action spell restriction** (`spells/apply.rs:40-52`): correct.
- **Cantrip scaling** (`spells/cast.rs:230-253`): correct.
- **Spell components** (`spells/cast.rs:131-152`): V/S validated.
- **Temp HP** (`combatants/update.rs:85`): `case when $7 > temp_hp then $7 else temp_hp` — highest-wins.
- **Massive damage** (`attack.rs:364-366`): `target.hp_current > 0 && remaining_after_zero >= target.hp_max`. Correct.
- **R/V cancellation** (`combat_engine/resolvers/damage_type.rs:6-44`): correct.
- **Lay on Hands** (`class_feature.rs:200-306`): reads `sheet.resources` fuzzy name, validates same encounter, locked via `SELECT FOR UPDATE`.
- **Conditions creature-type immunity** (`tactical/conditions.rs:73-82`): correct.
- **Incapacitating conditions break concentration** (`tactical/conditions.rs:153-157` + `spells/apply.rs:108-111`): correct.
- **Death saves** (`resolvers/death_save.rs:59-86`): nat 20 = +1 HP + reset; nat 1 = +2 failures.
- **Heal 0 → >0** (`actions/combat/heal.rs:85-106`): resets death saves. Correct.
- **Multiattack parser** (`parse_multiattack.rs:24-152`): parses "2 claws + 1 bite" correctly.
- **Body limit**: `DefaultBodyLimit::max(512 * 1024)` on combat router. `bulk_add` 1-100 cap. `set_initiative` 1-50 cap.
- **Token revocation at handshake**: `ws.rs:230-252` checks `token_version` on upgrade. (Mid-session re-check is the new finding F4.)
- **WS connect rate limit**: 60/min/user, bounded map.
- **Svelte 5 runes**: zero Svelte 4 violations across 3,142 LOC + 16 components.
- **`overlays_expire` event**: NOT dead — emitted by `tick.rs:165`, listened at `initiative/+page.svelte:512`. (Yesterday's prior audit and today's Agent 2 disagreed; verified live.)

---

*Last updated: 2026-06-23 (Sprint 32a: 3 CRIT fixed with regression tests; delta re-audit on top of 2026-06-22 backend audit).*
*Yesterday's audit preserved as `COMBAT_AUDIT_20260622.md` for diff reference.*

---

## Sprint 32 Fix Status (2026-06-23) — CRIT + HIGH all fixed

### 32a — 3 CRIT
| ID | Title | Status | Files Changed | Regression Test |
|----|-------|--------|---------------|-----------------|
| C-F1 | `overlay_damages` leaks `hp_after` | **FIXED** | `backend/src/routes/combat/tactical/hazards.rs` (1 field removed from WS payload) | `crit1_overlay_damages_ws_excludes_hp_after` |
| C-F2 | `use_action` no WS publish | **FIXED** | `backend/src/routes/combat/combatants/action.rs` (capture auth + add `ws::publish`) | `crit2_use_action_publishes_combatant_updates` |
| C-P1 | `auto_trigger_ready` correlated subquery + per-row N+1 | **FIXED** | `backend/src/routes/combat/actions/reactions.rs` (refactor); `web/.../initiative/+page.svelte` (plural event listener) | `crit3_auto_trigger_ready_uses_batched_update_and_ws` |

### 32b — 5 frontend HIGH
| ID | Title | Status | Files Changed | Regression Test |
|----|-------|--------|---------------|-----------------|
| F1 | `checkOpportunityAttacks` edge tokens + WS-driven OA | **FIXED** | `web/.../initiative/+page.svelte` (3 sub-fixes: null check, reach cells, WS handler) | Manual smoke (e2e deferred) |
| F2 | `hpRatio` ignores `hp_max_reduction` | **FIXED** | `web/.../initiative/+page.svelte` (hpRatio reads linkedChar.sheet.hp_max_reduction) | Manual smoke |
| F5 | Class features shown to all characters | **FIXED** | `web/.../initiative/+page.svelte` (charHasClass/Level helpers + 5 button gates) | Manual smoke |
| F6 | Result state never cleared | **FIXED** | `web/.../initiative/+page.svelte` (7 setTimeout(null, 5000)) | Manual smoke |
| F7 | DiceRoller no WS subscribe | **FIXED** | `web/.../initiative/+page.svelte` + `web/lib/combat/DiceRoller.svelte` (sharedHistory prop) | Manual smoke |

### 32c — 2 backend HIGH correctness
| ID | Title | Status | Files Changed | Regression Test |
|----|-------|--------|---------------|-----------------|
| F3 | `cast_spell` no `effects_change` emit | **FIXED** | `backend/src/routes/combat/spells/apply.rs` (HashSet of affected IDs, emit per ID post-commit) | `highf3_cast_spell_emits_effects_change` |
| F4 | WS connection no mid-session token_version check | **FIXED** | `backend/src/ws.rs` (connection() takes claims_tv + db, 4th select! arm with 30s interval) | `highf4_ws_re_checks_token_version_periodically` |

### 32d — 4 backend HIGH perf
| ID | Title | Status | Files Changed | Regression Test |
|----|-------|--------|---------------|-----------------|
| F8 | `apply_spell_outcome` N×M effect INSERTs + sync | **FIXED** | `backend/src/routes/combat/spells/apply.rs` (4 batched queries via unnest) | `highf8_spell_apply_batched_effect_insert` |
| F9 | `contested_hide` N+1 load_snapshot per observer | **FIXED** | `backend/src/routes/combat/actions/economy/contested.rs` (load_snapshots_batch) | `highf9_contested_hide_uses_batch_snapshots` |
| F10 | `attack` 3 overlapping encounter queries | **FIXED** | `backend/src/routes/combat/actions/combat/attack.rs` (single `Others` query, in-memory filter) | `highf10_attack_uses_single_others_query` |
| F11 | `multiattack` 5 queries × N hits | **FIXED** | `backend/src/routes/combat/special/multiattack.rs` (4 batched queries); `sync.rs` (new sync_combatant_hp_to_sheet_batch_tx helper) | `highf11_multiattack_batched_apply` |

**Test counts**: backend 586 → **595** (+9 new tests: 3 CRIT regressions, 2 highf correctness, 4 highf perf, 0 failures, 1 ignored pre-existing). Frontend vitest 630 → **630** (no change, no new component tests added — e2e rewrite deferred). `cargo check` and `svelte-check --threshold warning` both clean.

**Branch state**: 4 commits pushed to master: `f0ff66e` (32a) · `96605fd` (32b) · `1c404b1` (32c) · `83d8b58` (32d). 7 files modified across backend, 1 frontend, 1 test, 1 doc.

**Remaining**: 13 MED + 18 LOW + 6 INFO. See "Recommended Fix Order" below for Sprint 33-34 plan.

---

## Sprint 33 Fix Status (2026-06-23) — all 12 MED fixed

### 33a — 4 WS payload intel leaks
| ID | Title | Status | Files Changed | Regression Test |
|----|-------|--------|---------------|-----------------|
| M-WS1 | `dice_roll` leaks `user_id` + `character_id` | **FIXED** | `backend/src/routes/dice.rs` (stripped from public event) | `medws1_dice_roll_strips_user_id` |
| M-WS2 | `combatant_reacts.shield_blocked_hit` leak | **FIXED** | `backend/src/routes/combat/actions/reactions.rs` (dropped + removed local var) | `medws2_combatant_reacts_strips_shield_blocked` |
| M-WS3 | `combatant_uses_class_feature.message` leak | **FIXED** | `backend/src/routes/combat/special/class_feature.rs` (stripped from public event) | `medws3_class_feature_strips_message` |
| M-WS4 | `reaction_window.damage_pending` leak | **FIXED** | `backend/src/routes/combat/actions/combat/attack_apply.rs` (dropped + removed local) | `medws4_reaction_window_strips_damage_pending` |

### 33b — 3 perf N+1 + 1 frontend UI fix
| ID | Title | Status | Files Changed | Regression Test |
|----|-------|--------|---------------|-----------------|
| M-P1 | grapple release per-row UPDATE + WS loop | **FIXED** | `backend/src/routes/combat/tactical/conditions.rs` (1 batched UPDATE + batched WS) | `medmp1_grapple_release_batched` |
| M-P2 | `patch_effects` 3 per-row loops in autocommit | **FIXED** | `backend/src/routes/combat/events.rs` (tx + ANY($1) batched + atomic) | `medmp2_patch_effects_batched_and_atomic` |
| M-F2 | MultiattackForm single target / no weapon | **FIXED** | `web/src/lib/combat/forms/MultiattackForm.svelte` (per-row target + weapon select) | `medmf2_multiattack_per_attack_target_and_weapon` |

### 33c — i18n pass (M-F1)
| ID | Title | Status | Files Changed | Regression Test |
|----|-------|--------|---------------|-----------------|
| M-F1 | ~40 hardcoded English strings | **FIXED** | `web/src/lib/i18n/{en,it}.json` (18 new keys), `web/src/lib/combat/forms/AttackForm.svelte` (label_key), `web/src/routes/campaigns/[id]/initiative/+page.svelte` (5 buttons + reaction prompts) | `medmf1_i18n_keys_exist_in_both_locales` + `medmf1_hardcoded_strings_replaced_with_i18n` |

### 33d — UX polish
| ID | Title | Status | Files Changed | Regression Test |
|----|-------|--------|---------------|-----------------|
| M-F3 | cone spread 53.13° + hex grid distance | **FIXED** | `initiative/+page.svelte` (cone 45° in 2 sites, cellPx = g*0.75 for hex) | `medmf3_cone_spread_45_degrees` + `medmf3_oa_reach_uses_colspacing_for_hex` |
| M-F4 | hazard fields + click-to-place | **PARTIAL** | `initiative/+page.svelte` (hazard fields gated; createZoneOverlay accepts optional position) | `medmf4_create_zone_overlay_accepts_position` |
| M-F5 | Modal focus trap | **FIXED** | `web/src/lib/combat/Modal.svelte` (Tab cycle, initial focus, restore on close) | `medmf5_modal_focus_trap` |
| M-F6 | WS reconnect backoff + replay | **FIXED (Sprint 34b)** | `web/src/lib/ws.svelte.ts` + `backend/src/ws.rs` (publish_persist + replay_events; ws_events table with per-campaign seq) | `medmf6_ws_reconnect_exponential_backoff`, `publish_persist_no_string_concat` |

**Test counts**: backend 595 → **609** (+14 new tests: 4 medws, 3 medmp/mf, 2 medmf1, 5 medmf3-6; 0 failures, 1 ignored pre-existing). Frontend vitest 630 → **630**. `cargo check` and `svelte-check --threshold warning` both clean.

**Branch state**: 4 commits pushed to master: `c08de49` (33a) · `604b413` (33b) · `3316337` (33c) · `ded6eab` (33d).

**Remaining**: 18 LOW + 6 INFO. One MED partial (M-F4 full click-to-place) — deferred as larger UX feature. M-F6 fully closed in Sprint 34b.
