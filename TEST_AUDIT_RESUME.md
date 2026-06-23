# Test Audit Status — Resume Document

> **Purpose:** Resume point for the pre-existing test-failure cleanup that started in
> Sprint 38. The session is paused; everything below is needed to continue from where
> the work left off.
>
> **Last updated:** 2026-06-23 (end of session).

---

## TL;DR

- **Baseline:** 178 unique failing tests across 17 test binaries.
- **Current:** 149 unique failing tests across 17 test binaries (4 binaries now 100%).
- **Net fixed:** 29 tests across 5 commits. **Pushed to `master`.**
- **Status gates clean:** `cargo check` 0 warnings, `svelte-check` 0/0, frontend 630/630.

## Commits applied (newest first)

```
76d0bbf docs: log Sprint 38 follow-up batches 1-5 (29 tests fixed)
fb5b042 fix(test): world_content lore content→body rename
af068d3 fix(test): users tests full pass (12/12)
15feceb fix(test): characters_advanced full pass (18/18)
4a79cd4 fix(test+combat): field renames + npc-level fix + on-conflict
3f60245 fix(test+schema): cast-spell field renames + spells.damage_type col
d2a7abd docs: mark admin_restore + auth store localStorage fixes
a6731b8 fix(test): admin restore + auth store localStorage guards
```

The first two (a6731b8, d2a7abd) were the "fix all" session-1 work. The next five
(3f60245 → 76d0bbf) are the "fix in batch" session-2 work.

## Per-binary scoreboard

```
admin                  14/14  ✅ 100%
auth                    5/5   ✅ 100% (1 ignored)
characters_advanced    18/18  ✅ 100%
users                  12/12  ✅ 100%
dice                    1/1   ✅
combat_engine_unit     50/50  ✅
combat_engine_advanced 142/142 ✅
─────────────────────────────────────
campaigns_advanced      4/13   9 remaining
characters              1/14  13 remaining
combat_advanced         8/19  11 remaining
combat_coverage_jun2026 66/71  5 remaining
combat_full_integration 18/28 10 remaining
combat_integration      21/51 30 remaining  ← biggest
combat_movement         5/13   8 remaining
e2e                     2/12  10 remaining
edge_cases              6/14   8 remaining
effects                 7/8    1 remaining
messages                2/7    5 remaining
messages_advanced       2/12  10 remaining
more_gaps               2/7    5 remaining
notifications           0/5    5 remaining
quests_loot            11/12   1 remaining
uploads                 2/6    4 remaining
world                   5/13   8 remaining
world_content          10/19   9 remaining
ws_tests                7/9    2 remaining
─────────────────────────────────────
TOTAL:  149 failing, 514 passing of 663
```

## Files changed in this audit

**Source (production):**
- `backend/src/routes/admin.rs` — `restore_backup` uses `jsonb_populate_recordset`,
  `BackupTables` has `#[serde(default)]` on all 27 fields, column-name validation
  in pre-pass.
- `backend/src/routes/combat/spells/cast.rs` — `half_on_save: bool` has
  `#[serde(default)]`.
- `backend/src/lib.rs` — untouched (sqlx::migrate! embeds migrations at compile
  time, just rebuild when adding migrations).
- `web/src/lib/stores/auth.svelte.ts` — every `localStorage` access goes through
  `safeStorage()`.

**Migrations:**
- `migrations/20260623000003_spells_damage_type.sql` — adds nullable
  `spells.damage_type text` (cast_spell SELECT references it; SRD data has no
  damage_type so the column is always NULL → engine falls through to
  `detect_damage_type` template).

**Test helpers (used by fixed binaries + available for next batches):**
- `backend/tests/helpers.rs` — `add_member_via_invite(router, master_tok,
  user_tok, user_email, campaign_id, role)`. Replaces the removed
  `code`+`/join` flow. **Use this in every test that needs to add a member.**

**Test files modified:**
- `tests/combat_integration.rs` — 16 `slot_level`→`upcast_level`, 5
  `targets:[{target_id}]`→`target_ids:[id]`, NPC-level via `npcs.stats->pb`,
  multi-row spell insert `on conflict (slug) do nothing`, 32 `::uuid` casts.
- `tests/edge_cases.rs`, `tests/more_gaps.rs` — same spell renames as above.
- `tests/combat_full_integration.rs` — 32 `::uuid` casts.
- `tests/combat_coverage_jun2026.rs` — `on conflict` on spell inserts.
- `tests/characters_advanced.rs` — setup_campaign + 13 URL renames + 7 PATCH
  body wraps + `hit_dice: {current,max,die}` + `spell_id` (UUID) +
  204 status + death-save on characters was removed.
- `tests/users.rs` — 401 not 403 for wrong password; register response is
  `{user:{...}, token:...}`.
- `tests/world_content.rs` — 4 `content`→`body` in lore/session bodies.
- `tests/admin.rs` — 2 tests do inline `/auth/login` to avoid the 10/5min
  global login rate limit (tests/admin.rs:281, tests/admin.rs:386).

**Docs:**
- `COMBAT_AUDIT.md` — "Post-Sprint 38 mechanical fix batches" section.
- `TEST_GAPS.md` — auth.svelte.ts marked Tested.
- `AGENTS.md` §9.5 — `safeStorage()` mention.

## Error patterns (149 remaining)

Run `cargo test --no-fail-fast 2>&1 | grep -oE "left: [0-9]+|right: [0-9]+" | sort | uniq -c | sort -rn`
to reproduce these counts.

```
38 right: 200    # tests expect 200 but get something else
16 right: 201    # tests expect 201 (create) but get 200/204/etc.
14 left: 422     # validation: missing/wrong field name
14 left: 403     # permission: member/owner not granted
14 left: 400     # bad request body
 8 left: 500     # server error (engine bugs)
 7 left: 404     # wrong URL
 4 right: 0      # number assertion off (HP/movement)
 3 left: 204     # 200 vs 204 confusion
 3 right: 403    # tests expect 403 but get 401/200
 3 right: 400
 2 right: 401
 2 left: 201
```

Plus 38 `Option::unwrap() on None` (test setup races / 404s) and 19
`Result::unwrap() on Err` (most are SQL errors below).

### Recurring SQL errors (test side)

```
column "death_saves" of relation "combatants" does not exist       (×3)
column "movement_remaining_ft" of relation "combatants" does not exist (×2)
column "owner_id" does not exist (referenced from combatants — actually on characters) (×5)
column "modifiers" of relation "combatants" does not exist         (×1)
column "sheet" of relation "combatants" does not exist             (×1)
column "slug" of relation "npcs" does not exist                     (×1)  [wrong on conflict (slug)]
column "campaign_id" is of type uuid but expression is of type text (×1)  [missing ::uuid cast]
```

These are tests writing to columns that the engine reads from elsewhere:
- `death_saves` → on `characters.sheet->'death_saves'` (or `npcs.stats`)
- `movement_remaining_ft` → engine reads `sheet->'speed'`
- `modifiers` → on `combatant_effects.modifiers` (insert into effects table)
- `sheet` on combatants → on `characters.sheet`
- `npcs.slug` on conflict → npcs has no slug column; only spells does

### Other recurring patterns

1. **Field renames** (audit batch 2/3 only did combat + characters):
   - `content` → `body` (messages, news, lore, recap)
   - `visibility: "public"` → `scope: "campaign"` (messages)
   - `visibility: "private"` → `scope: "whisper"` (messages)
   - `npc_id` for npc combatants, `character_id` for character combatants
   - `amount` → `quantity` (loot, quests)
   - `is_equipped` → `equipped`
   - `temp_hp` (correct) vs `temporary_hp`

2. **Endpoint renames**:
   - `/messages/{id}` PATCH body: `body` (not `content`), `scope` (not `visibility`)
   - `POST /messages` with `scope: "campaign" | "whisper"`, `body`, `recipient_id?`
   - `/campaigns/{cid}/award-xp` body: `{character_id, amount, source}` (was different)

3. **Add-member pattern** (sprint 38 batch 3):
   - **Use** `add_member_via_invite(router, master_tok, user_tok, user_email, cid, role)`
     in `tests/helpers.rs` (already added). The `code`+`/join` flow is GONE.

4. **Status code drift**:
   - DELETE → 204 (not 200) — affects: characters delete, maps delete, npc delete,
     pins delete, combatant delete.
   - PATCH/PUT/POST creation → 201 + body OR 204 no body. Check endpoint.
   - Wrong password / bad token → 401 (not 403).

5. **Membership gating**:
   - `/campaigns/{id}/...` endpoints require `require_member` (player).
   - `/admin/...` endpoints require `require_admin` (admin role).
   - Master cannot create characters but bypasses character_limit.

6. **Spell/SRD seed**:
   - `tests/helpers.rs::seed_spells` runs in `make_app`. Tests should NOT
     re-insert spells. If a test needs a custom spell, use `on conflict (slug)
     do nothing` in the INSERT.

7. **UUID casts in test SQL**:
   - `where id = $1` → `where id = $1::uuid` whenever binding a string
     into a uuid column. Already done in combat_integration and
     combat_full_integration.
   - Same for `encounter_id`, `campaign_id`, `user_id`, `character_id`,
     `npc_id` lookups in test SQL.

8. **Combatant schema** (the migration is OUT OF SCOPE per the audit, but
   if a future batch adds the columns, all of these would Just Work):
   - `combatants.death_saves` — referenced by tests
   - `combatants.sheet` — referenced by tests
   - `combatants.movement_remaining_ft` — referenced by tests
   - `combatants.modifiers` — referenced by tests
   - `combatants.concentration_spell` — referenced by tests
   - These are engine-managed via `characters.sheet` or `combatant_effects`,
     but tests write directly to combatants. **Two options**:
     a) Add nullable columns + simple mirror logic in the engine
     b) Update tests to write to the actual source-of-truth location

## Per-binary fix strategy

When resuming, work the binaries in this order (highest impact first):

### Tier 1: easy wins, mostly mechanical
1. **`tests/messages_advanced.rs`** (10 failures) — all the unwrap-None
   errors are at lines 81, 104, 126, 222, 260, 301, 333, 368, 385, 420.
   Most likely the same `setup_campaign_with_members` helper issue +
   `content`→`body` and `visibility: "public"`→`scope: "campaign"`.
   See `tests/messages_advanced.rs:75-83` for the pattern.
2. **`tests/world_content.rs`** (9 remaining) — 4 already fixed
   (`content`→`body`). Remaining are maps/pins/npcs CRUD. Endpoint
   patterns + 200/204 likely.
3. **`tests/world.rs`** (8 failures) — same patterns. `factions_npcs_visibility`,
   `recap_*` are likely visibility renames. `maps_and_pins` is endpoint +
   method issues. `whisper_*` are message body shape.

### Tier 2: medium
4. **`tests/edge_cases.rs`** (8 failures) — combat mechanics: `silenced`,
   `no_somatic`, `saving_throw`, `concentration_check`, `cover_bonus`.
   These are likely engine state tests that need either schema additions
   or specific spell/move setup.
5. **`tests/e2e.rs`** (10 failures) — broad integration. Look for shared
   test setup issues first (lines 637, 771, 859 unwrap-None).
6. **`tests/campaigns_advanced.rs`** (9 failures) — `setup_campaign_with_members`
   at line 41 likely uses the old `code` field. Replace with
   `add_member_via_invite`.

### Tier 3: heavy lifting
7. **`tests/combat_integration.rs`** (30 failures) — biggest. Includes
   lay_on_hands, rage, dodge, dash, smite, counterspell, shield,
   uncanny_dodge, grapple, hazard, readied_action. The `death_saves` and
   `modifiers` column errors live here. These need either schema
   migrations or test-side column-targeting fixes.
8. **`tests/characters.rs`** (13 failures) — `award_xp`, `long_rest`,
   `short_rest`, character permissions. Many may share the
   `add_member_via_invite` fix.
9. **`tests/more_gaps.rs`** (5 failures) — mixed: 1 combat, 1 wizard
   spell prep, 3 uploads. The uploads ones are s3:None in test config
   per the audit (separate work).
10. **`tests/combat_advanced.rs`** (11 failures) — action economy tests
    (dash, dodge, disengage, help, hide, opportunity attack, etc.).
    These are action_used/bonus_action_used tracking on the combatants
    table — the columns exist but the test setup may be off.
11. **`tests/combat_full_integration.rs`** (10 failures) — `contested_hide`,
    `dash_doubles_movement`, `gm_npc_move_caps`, `patch_effects_add_bless`,
    `regen_*`. Engine mechanics tests.

### Tier 4: small
12. **`tests/messages.rs`** (5) — `list_campaign_messages_excludes_whispers`
    and `whisper_*`. Same `body`/`scope` patterns.
13. **`tests/combat_movement.rs`** (8) — `move_combatant_updates_position`,
    `move_consumes_movement`, overlays. May share patterns.
14. **`tests/uploads.rs`** (4) — `get_upload_url_for_*` + update portrait.
    Likely s3:None config issue per audit.
15. **`tests/notifications.rs`** (5) — list empty, mark read, unread filter.
    All unwrap-None. Test setup races.
16. **`tests/combat_coverage_jun2026.rs`** (5) — `high16_multiattack_*`,
    `highf8_spell_apply_batched_*`, `info4_action_surge_*`, `mech_trigger_ready_*`,
    `med6_cantrip_with_upcast_*`. Mostly the column ref pattern.
17. **`tests/effects.rs`** (1) — `apply_spell_effect_requires_active_encounter_combatant`.
18. **`tests/quests_loot.rs`** (1) — `loot_player_can_create`.
19. **`tests/ws_tests.rs`** (2) — `combat_event_triggers_notification`,
    `ws_campaign_without_auth_fails`.

## How to start the next session

1. Pull latest `master` (includes all batches 1-5 + docs).
2. Run `cargo test --no-fail-fast 2>&1 | grep "test result:"` to confirm the
   149 baseline.
3. Re-read `COMBAT_AUDIT.md` "Post-Sprint 38 mechanical fix batches" for
   context.
4. Pick the next binary from the Tier list. Read the failing test (use
   `cargo test --test <bin> <test_name> 2>&1 | grep -E "panicked|left|right|message|column"`)
   to see the actual error.
5. Apply the fix following the patterns above.
6. Commit per binary (or per 2-3 binaries) with a message like
   `fix(test): <bin> full pass (X/Y)` or `fix(test): <bin> progress (X→Y/Z)`.
7. Update the scoreboard in this doc.
8. Update `COMBAT_AUDIT.md` "Post-Sprint 38 mechanical fix batches" section
   with the new batch number.

## Commit message template

```
fix(test): <binary> full pass (X/X)

- tests/<file>: <pattern 1>
- tests/<file>: <pattern 2>
- tests/helpers.rs: <helper changes if any>

Tactical notes: <why this fix was the right call, any engine code touched,
any migrations added>.
```

## Things to AVOID (already tried, didn't work)

- **Don't add a blanket migration** with `combatants.death_saves`,
  `combatants.sheet`, `combatants.modifiers`, etc. The engine reads these
  from `characters.sheet` and `combatant_effects`. Adding redundant
  columns will create state-sync bugs and add 0 value.
- **Don't blanket-wrap PATCH bodies in `{"sheet": ...}`** in characters.rs
  (the simpler cases) — some PATCH bodies (e.g., `{"name": "X", "level_total": 2}`)
  use top-level fields, not sheet sub-keys. Look at the body shape first.
- **Don't blanket-cast `::uuid` on `combatant_effects` columns** — they're
  already typed. Only cast on cross-table lookups (combatant_id, etc.).
- **Don't add `on conflict (slug) do nothing` to characters/spells INSERTs**
  that are intentional (e.g., tests checking duplicate detection).
  The pattern is for `insert into spells` only, since helpers already
  seed the SRD.

## Useful one-liners

```bash
# Run one binary
cargo test --test <bin> 2>&1 | tail -3

# List failing tests in one binary
cargo test --test <bin> 2>&1 | grep -B 1 "FAILED\|^---- " | grep -E "^---- " | sed 's/^---- //' | sort -u

# Per-test error
cargo test --test <bin> <test_name> 2>&1 | grep -E "panicked|left|right|message|column|missing" | head -3

# Rebuild after migration (sqlx::migrate! embeds at compile time)
touch backend/src/lib.rs && cargo test

# Full run with timing
cargo test --no-fail-fast 2>&1 | grep "test result:" | tee /tmp/run.txt

# Count error patterns
cargo test --no-fail-fast 2>&1 | grep -oE "left: [0-9]+|right: [0-9]+" | sort | uniq -c | sort -rn
```

## Caveats

- **`$crate` errors and pre-fix assumptions:** the audit (COMBAT_AUDIT.md
  §"Honest verdict") found that the prior baseline of "619 passing" was a
  fiction — tests were silently being skipped because the per-test
  `search_path` was missing `public`, breaking `create extension ... citext`.
  This was fixed in Sprint 37 by appending `,public` to the URL search_path.
  The 149 current failures are the real test coverage gap, not test
  infra noise.
- **DB is in Docker** (`dungeonsandapps-postgres` container on port 5432).
  Test schema is per-test (each test gets a fresh schema, see
  `tests/helpers.rs::make_app`).
- **Login rate limit** is global per-process: 10 attempts per 5 minutes.
  Any test that does `auth/login` should be careful. The `setup_admin_and_user`
  pattern in `tests/admin.rs` was the only test that originally broke this.
  Other tests either use `register` (no login) or do inline login.
- **DB log shows checkpoints every 29s** because tests churn the WAL. This
  is expected; not a test failure.
</content>
</invoke>