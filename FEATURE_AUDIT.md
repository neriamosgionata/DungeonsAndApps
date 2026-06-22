# CinghialApp — Feature Audit Report

> Generated: 2026-05-04 (Round 6 — Full re-audit post-fixes) | Last updated: 2026-06-19 (Combat audit — 220 findings; Sprints 9 + 10 + 11 closed 14/14 critical + 12/19 high backend + 12/18 high frontend + 4 RMW races)
> Scope: Security, DB schema, API completeness, frontend UX, WS events, i18n, tests, architecture

---

## Combat Audit 2026-06-19 (Round 7)

Full audit of combat system: backend (8,755 LOC routes + 2,941 LOC engine) + frontend (3,021 LOC page + 2,611 LOC extracted) + 47 migrations.

**Findings: 220 (🔴 14, 🟠 74, 🟡 100, 🔵 32) + 1 frontend type-drift risk.**

### Closed in Sprint 9 (top-5 critical, see `DND_AUTOMATION_GAPS.md`):
- C1/C2: `use_action` no RBAC + format!-SQL — any user toggled any combatant
- C3/C4: paralyzed/stunned flyers + fly-replaces-walk (PHB p.292)
- C6: `natural_roll` reads unkept die on adv/dis (death save, skill check, TWF)
- C10: `bulk_add_combatants` no validation
- C11/C12: `save_dc=0` auto-pass + `cantripLevel` reads wrong field

### Closed in Sprint 10 (atomicity + state-corruption + dead code + drift + 4 frontend critical paths, see `DND_AUTOMATION_GAPS.md`):
- Atomicity: `grapple_escape`, `trigger_ready`, `class_feature.rage` — added `where <col> = false returning id` pattern
- Semantic: `set_initiative` — refactored to list of `{combatant_id, initiative}` (test was failing on master, now passes)
- Stale: `combatant_leaves` String→Uuid; `combatant_joins` bulk first-id-only→per-combatant events; dead `delete`/`list` duplicates removed
- Drift: `prev_turn` added `tick_effects` + per-turn reset + `notify_turn`; early-return guard for round 0/turn 0 (was 500)
- Visibility: `list_combatants` non-master path shows own hidden combatants
- Error: `attack` `map_grid_size` fetch_one→fetch_optional + NotFound
- Frontend: lay_on_hands self-default, applyDamage hp_max_reduction clamp, autofill dead-branch, reactionWindowNotice timer tracking

### Closed in Sprint 11 (4 RMW races + 4 frontend high paths, see `DND_AUTOMATION_GAPS.md`):
- RMW-1: `move_combatant` `SELECT FOR UPDATE` + tx wrap
- RMW-2: `class_feature` (second_wind, lay_on_hands, uncanny_dodge) — `SELECT FOR UPDATE` on relevant rows in tx
- RMW-3: `apply_spell_outcome` slot decrement — `SELECT FOR UPDATE` on character row
- RMW-4: `start_encounter` — wrap 5 UPDATEs in tx
- FE-5: `checkOpportunityAttacks` dedupe by `(attacker_id, target_id)`
- FE-6: `Roster.svelte` + parent double search — removed parent input + dead `rosterSearch`/`rosterCombs`
- FE-7: `Banner.svelte` chained `.replace` → svelte-i18n `values: {n, total}` (order-safe)
- FE-8: `2× loadList()` per action — `lastLocalLoadAt` + 500ms dedupe window for WS-triggered loads

### Still open (deferred to future sprints):
- 52 backend + 27 frontend UX smells
- 10 untested mechanics (Rage end, Smite, Condition timer, Hidden reveal, Grapple release, Regen at turn start, Ritual casting, Spell range E2E, Fighting style Defense, Condition immunity by creature type)
- 110+ hardcoded EN strings in combat UI
- Stale `last_hit_attacker` ref in `web/src/lib/types.ts:307` (column dropped 2026-06-17)
- ~40 stale line refs in `DND_AUTOMATION_GAPS.md` (pre-Sprint 7-8 split)

---

## 1. Security

### ✅ 1.1 `combat.rs:667` — `.unwrap()` on `body.npc_id`
**Fixed (2026-05-04):** Replaced `.unwrap()` with `.ok_or(AppError::BadRequest(...))?`.

### ✅ 1.2 XP addition not overflow-checked
**Fixed (2026-05-04):** Changed to `xp_before.saturating_add(body.xp_each)`.

### ✅ 1.3 `characters.rs:160` — `.unwrap_or(1)` silently defaults char limit
**Fixed (2026-05-04):** Changed to `.ok_or(AppError::Forbidden)?` — non-members get 403.

### 🟢 1.4 Upload streams to disk before auth check
**File:** `backend/src/routes/uploads.rs:206-236`  
Auth token is validated at function entry (JWT via `AuthUser` extractor), but `campaign_id` membership check happens after file is fully streamed to a temp file on disk. An authenticated-but-non-member user can waste server disk I/O.  
**Fix:** Parse `campaign_id` from multipart BEFORE streaming file chunks; return 403 early.  
*(Note: no S3 write happens before auth — this is disk only, not a data leak.)*

---

## 2. DB Schema

### 🟢 2.1 Missing `updated_at` auto-triggers
No `BEFORE UPDATE` trigger sets `updated_at = now()` automatically. All mutations must set it manually via `updated_at = now()` in the SQL. Several routes already do this, but a missed UPDATE silently leaves stale timestamps.  
**Fix:** Migration: `CREATE OR REPLACE FUNCTION set_updated_at() RETURNS trigger AS $$ BEGIN NEW.updated_at = now(); RETURN NEW; END $$ LANGUAGE plpgsql;` + attach to all tables with `updated_at`.

### ✅ 2.2 No data backfill for old bad visibility defaults
**Fixed (2026-05-04):** Added UPDATE backfill statements to `20260501000004_fix_defaults.sql` for `campaign_sessions`, `news_entries`, `maps`, `map_pins`.

### 🔵 2.3 CHECK constraints added late
**Migration:** `20260501000003_indexes_and_constraints.sql`  
HP/AC/movement bounds checks were added after schema init. Already in place; no runtime risk now.  
*(No action needed — noted for posterity.)*

---

## 3. Frontend API Client

### ✅ All combatant endpoints present
`dash`, `hide`, `search`, `use-object`, `conditions`, `multiattack`, `two-weapon-fight`, `class-feature` — all in `resources.ts`.

### ✅ `Auth.updateMe` + `Auth.changePassword` added
`PATCH /users/me` and `POST /users/me/change-password` wired.

### 🟢 3.1 No frontend UI for `Auth.updateMe` / `Auth.changePassword`
API methods exist in `resources.ts` but no user-facing settings page calls them. Users cannot change their own display name or password.  
**Fix:** Add profile/settings page at `/profile` or expose in campaign settings sidebar.

---

## 4. Frontend UX

### ✅ 4.1 Loading states
All `onMount(load)` pages have `let loading = $state(true)` + `finally { loading = false }`:  
character, group, map, npcs, factions, lore, news, recap, dice, spells.

### ✅ 4.2 Loading states — messages, members, initiative, settings
**Fixed (2026-05-04):** Added `loading = $state(true)` + `finally { loading = false }` + `{#if loading}` display to all four pages.

### ✅ 4.3 Delete confirmations
All destructive actions confirmed: character sheet (spell/feat/equipment/weapon/attunement/class/resource/feature), initiative (combatant remove, overlay remove), group (loot), npcs, sessions, maps, factions, lore, news, settings.

### ✅ 4.4 Search/filter
Group loot, group quests, recap sessions, members list all have live filter.  
npcs, factions, lore already had search.  
news has pagination (no text search — acceptable).

### 🟢 4.5 State doesn't persist across navigation
Tab selections, search queries, pagination indices all reset when navigating away and back.  
**Fix:** Encode state in URL search params (`?tab=magic&q=fire`) via SvelteKit `$page.url.searchParams`.

### ✅ 4.6 Error pages
`src/routes/+error.svelte` and `src/routes/campaigns/[id]/+error.svelte` both exist.

---

## 5. WebSocket Events

### ✅ 5.1 Core combat events handled
`next_turn`, `encounter_started`, `encounter_ended`, `encounter_updated`, `encounter_deleted`, `encounter_created`, `combatant_*`, `lair_action`, `surprise_round`, `overlay_damage` — all handled in `initiative/+page.svelte:284-300`.

### ✅ 5.2 `party_updated` handled
`group/+page.svelte:61` listens for `party_updated`.

### 🟢 5.3 No WS event enum — ~70 ad-hoc strings
All events are emitted as `json!({"type":"snake_case_name",...})` strings with no shared enum/constant. A typo in either backend or frontend silently breaks realtime.  
**Fix:** Define `WsEvent` enum in Rust with `#[serde(tag = "type")]`; mirror in `web/src/lib/types.ts`.

### 🔵 5.4 No `presence_typing` event
No "user is typing" indicator in chat.  
*(Low priority — chat works without it.)*

---

## 6. i18n

### ✅ 6.1 Skill/ability names use i18n
`{$_('character.skill_${sk.key}')}` + `{$_('character.ability_${sk.ability}')}` — fully i18n'd. Keys exist in both `en.json` and `it.json`.

### ✅ 6.2 Hardcoded Visibility label in recap form
**Fixed (2026-05-04):** Replaced with `{$_('common.visibility')}`.

### 🔵 6.3 D&D class/subclass names in `dnd/classes.ts` and `dnd/subclasses.ts`
Class names (Fighter, Wizard, etc.) are English-only strings. These are proper nouns in the SRD — acceptable to leave untranslated, but should be documented as intentional.

---

## 7. Test Coverage

### Backend — 437 tests pass (29 test files)
| Suite | Coverage |
|---|---|
| auth.rs | JWT, CORS, rate limiting, password strength |
| dice.rs | Dice roll integration |
| combat*.rs | Combat engine, encounters, combatants, effects |
| e2e.rs | Auth flow, campaign/character RBAC, combat full flow, battle map tokens, dice, world |
| world.rs | Maps, factions, NPCs, group, recap, messages, whispers |
| ws_advanced.rs | WebSocket auth, events, broadcast, presence |
| characters*.rs | Character CRUD, rests, hit dice |

### Frontend — 626 tests pass (19 test files)
| Suite | Coverage |
|---|---|
| dnd/*.test.ts | Calculations, spell slots, resources, feats, classes, subclasses, items, dice, time |
| api/client.test.ts | API client basics |
| validation.test.ts | Email, password, HTML sanitize |
| utils.test.ts | Slugify, format, capitalize, truncate, debounce |
| i18n/*.test.ts | i18n init, key parity, locale validation |
| stores/stores.test.ts | AuthStore (token, user, cross-tab sync) |
| uuid.test.ts | UUID generation |
| wsUrl.test.ts | WebSocket URL construction |
| onboardingSteps.test.ts | 25 tests covering fresh-character step generation |

---

## 8. Architecture

### ✅ 8.1 `combat.rs` — 4,913 lines (threshold: 4,800)
Split into 8 submodules under `routes/combat/` as recommended below. Total ~6,762 lines across files.

### 🟡 8.2 `combat_engine.rs` — 2,461 lines (up from 1,936)
Pure logic + snapshot loading. Consider extracting `load_snapshot`/`load_snapshots_batch` to `combat/query.rs`.

### 🟢 8.3 `characters.rs` — 943 lines (up from 779)
Contains sheet CRUD, rest mechanics, spell CRUD, XP award. Consider splitting:
- `characters/sheet.rs` — CRUD + combatant sync
- `characters/rest.rs` — short/long rest
- `characters/spells.rs` — spell list management

### 🟢 8.4 `world.rs` — 600 lines
Factions, NPCs, lore, news all in one file. Split when adding more world features.

---

## 9. Test Results (Current)

| Suite | Tests | Status |
|---|---|--------|
| Backend | 437 | ✅ All pass |
| Frontend | 626 | ✅ 19 test files pass |
| Frontend type check | 0 errors | ✅ |
| Backend compile | 0 errors, 0 warnings | ✅ |

---

## Summary

| Severity | Count | Open | Fixed This Session |
|---|---|---|---|
| 🔴 Critical | 0 | — | `combat.rs` split ✅, `npc_id.unwrap()` ✅ |
| 🟡 High | 0 | — | XP saturating_add ✅, char limit Forbidden ✅, 4 loading states ✅ |
| 🟢 Medium | 6 | updated_at triggers, no profile UI, state persistence, WS enum, test gaps, file split candidates | visibility backfill ✅, vis label ✅ |
| 🔵 Low | 2 | upload auth order (disk only, no data leak), D&D class names | — |

---

*End of audit report.*
