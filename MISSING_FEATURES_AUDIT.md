# CinghialApp — Missing Features Audit

> Generated: 2026-04-30 | Last updated: 2026-06-19 (Combat audit 2026-06-19 — 220 findings, see FEATURE_AUDIT.md "Round 7")
> Scope: Full codebase exploration — backend routes, DB schema, frontend pages, WS events, modules
> Method: 4 parallel explore agents across all domains

---

## Legend

| Icon | Meaning |
|------|---------|
| 🔴 | Critical gap — blocks core D&D 5e loop |
| 🟡 | High gap — expected feature in modern VTT/campaign manager |
| 🟢 | Medium gap — nice-to-have, quality of life |
| 🔵 | Low gap — polish/technical debt |

---

## 1. Core D&D 5e Mechanics — Character Sheet

### 🔴 1.1 Everything Character-Sheet Is JSONB Black Box
**Table:** `characters.sheet` (jsonb)

No queryable columns for:
- Ability scores (STR/DEX/CON/INT/WIS/CHA)
- Skill proficiencies
- Save proficiencies
- Hit dice pool
- Death save successes/failures
- Spell slots remaining / max
- Inspiration
- Experience points
- ~~Alignment~~ (now stored in `sheet.alignment`, story tab display + create form)
- Bonds, flaws, ideals
- Background

**Impact:** Cannot SQL-query "all chars with Perception proficiency" or "who has 3rd-level slots". No DB-level validation of JSON shape. All logic must parse JSONB in Rust.

**Fix direction:** Either normalize to columns (`str`, `dex`, `con`, `int`, `wis`, `cha`, `inspiration`, `xp`, `hit_dice_remaining`, `death_save_successes`, `death_save_failures`) or add generated columns + expression indexes. At minimum, `characters` needs `inspiration` and `xp` columns for queryability.

---

### 🔴 1.2 No Personal Inventory / Equipment System
**Tables missing:** `character_inventory`, `items`, `equipment`

- `loot_items` exists at **party** level only (`parties` → `loot_items`)
- `loot_items.claimed_by` FK → `characters(id)` exists but no UI/API for "equip", "unequip", "attune"
- No `items` catalog (spells have global `spells` table; items have nothing)
- Character sheet stores equipment opaquely in `sheet` JSONB

**Impact:** Fighter's +1 longsword has no dedicated home. Cannot build shop, loot generator, or equipment UI without hard-coding.

**Fix direction:**
1. Create `items` table (global SRD equipment catalog)
2. Create `character_inventory` table (FK `character_id`, `item_id`, `equipped`, `attuned`, `quantity`)
3. Add `POST /characters/{id}/inventory`, `PATCH /characters/{id}/inventory/{inv_id}`, `DELETE …`
4. Add "Shop" page or integrate into group/loot

---

### 🔴 1.3 No Spell Slot Tracking Table
**Table missing:** `spell_slots`

- `character_spells` tracks **which** spells known/prepared
- No table tracking **how many slots** of each level remain
- Slots live inside `sheet->slots` JSONB

**Impact:** Cannot query "who has 3rd-level slots left". Slot consumption in `cast_spell` must parse JSONB.

**Fix direction:** Add `spell_slots` table or generated columns for `slots_l1_max`, `slots_l1_current`, … `slots_l9_current`.

---

### 🟡 1.4 No Hit Dice / Rest Log Tracking
**Tables missing:** `rest_log`, `hit_dice_pool`

- Short rest / long rest endpoints exist but only modify `sheet` JSONB
- No log of when rests happened, how many hit dice spent, HP recovered
- No enforcement of "regain half hit dice on long rest" (must trust client JSON)

**Impact:** GM cannot audit rest usage. No temporal tracking of character state.

---

### 🟡 1.5 Character Currency Is Party-Only
**Table:** `parties` has `cp/sp/ep/gp/pp`

- No per-character purse
- Rogue pickpockets 50gp → nowhere to store individually
- No "split loot" or "transfer coin" mechanics

**Fix direction:** Add `cp`, `sp`, `ep`, `gp`, `pp` columns to `characters` or create `character_currency` table.

---

### 🟡 1.6 No Conditions Reference Table
**Column:** `combatants.conditions` is `text[]`

- No FK to canonical `conditions` table
- Condition names are free-text
- No auto-linking to rules
- No duration enforcement on conditions (only `combatant_effects` handles duration)

**Fix direction:** Create `conditions` table with SRD condition definitions, change `combatants.conditions` to reference it, or keep text[] but validate against known list.

---

### 🟡 1.7 No Character Class / Race / Background Tables
**All stored in `sheet` JSONB:**

- Multiclass progression
- Subclass features
- Racial traits
- Background features

**Impact:** Cannot query "all paladins" or "show me every character with Lucky feat". Cannot enforce class-level caps on features.

---

## 2. Core D&D 5e Mechanics — Combat

> **Note (2026-05-04):** Combat mechanics have been substantially improved. See `DND_AUTOMATION_GAPS.md` for current status. Key combat gaps that were present at audit time and are now ✅:
> Fighting styles, extra damage (sneak/smite/rage), two-weapon fighting, ritual casting, spell preparation enforcement, temp HP highest-wins, massive damage instant death, death save reset on heal, surprised enforcement, regeneration, condition immunity/durations, grapple auto-release, cantrip scaling, spell attack roll path, spell components/range validation, hazard zone damage, Shield/Counterspell reaction gating, ready action auto-execute.

### 🟡 2.1 Encounter / NPC Templates — ✅ (2026-08-04)
`encounter_templates` table (name + combatants JSONB: display_name, hp_max, ac, stats, count); `GET/POST /campaigns/{id}/encounter-templates` (master write) + `POST /encounters/{id}/spawn-from-template` (creates NPCs + combatants, grouped by name); FE: save current NPC combatants as a template + spawn picker in the initiative page.

### 🟡 2.2 No Monster Catalog / Bestiary
**Related to 2.1**

- NPCs use `stats` JSONB
- No global monster reference table
- No CR-based filtering
- No automatic XP calculation from encounter composition

---

### 🟢 2.3 Custom Spells / Homebrew — ✅ (2026-08-04)
`campaign_spells` table (campaign_id + slug PK); `GET/POST /campaigns/{id}/spells` + PATCH/DELETE per slug (master-only writes); `GET /spells?campaign_id=` merges campaign spells over SRD; `cast_spell` falls back to campaign spells; FE homebrew panel in the spells page (create/list/delete form).

---

### 🟢 2.4 Campaign Settings / House Rules — ✅ (2026-08-04)
`campaigns.settings` jsonb column (migration `20260804000010`), master-only PATCH, FE house-rules textarea in the settings page. Still missing: custom currencies, homebrew classes/races, campaign modifiers.

**Fix direction:** Add `settings` jsonb column to `campaigns` or create `campaign_settings` table.

---

## 3. World Building & Campaign Management

### 🟡 3.1 In-Game Calendar / Time Tracking — ✅ (2026-08-04)
`campaign_calendar` table; GET/PATCH + advance endpoints; FE calendar page. **Plus (round 7): moon phases (8-phase cycle by day), fixed-date holidays (add/list, "today" highlight), weather.** Still missing: session-date mapping.

### 🟡 3.2 Weather / Environment Tracking — ⚠️ (2026-08-04)
`campaign_calendar.weather` text field — master-editable, displayed on the calendar page. Still missing: weather history per session, seasonal modifiers, environmental hazards.

---

### 🟡 3.3 No Travel / Journey / Random Encounters
**Tables missing:** `journeys`, `travel_legs`, `random_encounter_tables`

- No hex-crawl support
- No random encounter generation
- No travel pace/speed calculations
- No foraging/survival tracking

---

### 🟡 3.4 No Shops / Merchants / Economy
**Tables missing:** `shops`, `shop_inventory`, `price_lists`

- Loot tracking exists but no buy/sell
- No item pricing UI
- No merchant haggling mechanics
- No regional price variations

---

### 🟢 3.5 Tagging / Labeling System — ⚠️ (2026-08-04)
`tags` + `taggings` tables; `GET/POST /campaigns/{id}/tags` (master write), apply/remove per resource, resource-scoped lookup; FE: tag chips + filter + create/color on the NPC page. Still missing: tags on quests/maps/lore/news, tag filtering on those lists.

---

### 🟢 3.6 Player Journal / Private Notes — ✅ (2026-08-04)
`journal_entries` table (campaign + author scoped, private); `GET/POST /campaigns/{id}/journal` + `PATCH/DELETE /journal/{id}` (author-only); FE journal page (create/edit/delete, only your entries).

---

### 🟢 3.7 Player Attendance / Session RSVP — ✅ (2026-08-04)
`session_attendance` table (session_id + user_id); `GET/POST /sessions/{id}/attendance` (master writes); FE attendance checkbox picker per session in the recap page (master).

### 🟢 3.8 No Campaign Handouts
**Could overlap with `news_entries` / `lore_entries`**

- No dedicated handout system
- No "reveal to players" mechanic for lore pieces
- No timed/drip-fed information

---

## 4. User Experience & Quality of Life

### 🟡 4.1 User Profile / Settings Page — ✅ (2026-08-04)
**Backend:** `GET/PATCH /users/me`, `POST /users/me/change-password`
**Frontend:** `/campaigns/[id]/profile` page exists (display name, language, password change); now with **avatar upload** (`ImageUpload kind="avatar"` → `users.avatar_url` via `SelfUpdate.avatar_url`) + nav entry for all roles. Still missing: dark/light toggle, notification prefs.

---

### 🟡 4.2 No Self-Service Password Reset
**Backend has:** Admin-only `POST /users/{id}/reset-password`
**Missing:** Forgot-password flow with email/token

---

### 🟡 4.3 Export / Import — ✅ (2026-08-04)
- **Character export/import ✅** — JSON download + sheet-replace import on the character page
- **Campaign export ✅** — `GET /campaigns/{id}/export` (campaign, members, calendar, factions, NPCs, lore, news, sessions + attendance, characters, campaign spells, maps + pins, party, loot, quests)
- **Campaign import ✅** — `POST /campaigns/import` recreates everything with fresh ids (owners/attendance re-mapped by email)
- Session recap PDF ❌

### 🟡 4.4 Bulk Operations — ⚠️ (2026-08-04)
- **Bulk invite ✅** — `POST /campaigns/{id}/invitations/bulk` + FE textarea
- Bulk add combatants ✅, bulk award XP ✅
- **Bulk delete NPCs/lore/news ✅** — `POST /campaigns/{id}/{npcs|lore|news}/bulk-delete` {ids}
- Bulk update character levels ❌

---

### 🟢 4.5 No Loading States
**Every page** fetches on mount with zero visual feedback:
- No skeleton screens
- No spinners
- Character, initiative, group, map, members, messages, news, NPCs, recap, settings all affected

---

### 🟢 4.6 Delete Without Confirmation
**Instant destructive actions:**
- Initiative: remove combatant, remove token, delete overlay
- Group: delete loot, unlink NPC from quest
- Character: remove equipment, weapon, spell, feat, attunement, class, resource
- Maps: delete pin
- World: delete NPC, faction, lore, news

---

### 🟢 4.7 Search / Filter — ⚠️ (2026-08-04)
- ✅ Loot items, quests, session recaps, members, NPCs, factions, lore
- ✅ **News articles (2026-08-04 — client-side title/body filter)**
- All remaining lists now searchable

---

### 🟢 4.8 State Doesn't Persist Across Navigation
- Tab selections reset
- Search queries reset
- Pagination indices reset
- Selected items reset
- Map zoom/pan resets

---

### 🟢 4.9 No 404 / Error Pages
- No `+error.svelte` anywhere
- Invalid campaign IDs show small red inline text
- Network errors show browser default or silent failure

---

### 🟢 4.10 No Admin Dashboard Beyond User List
**Backend has:** `GET /users`, `PATCH /users/{id}`, `DELETE /users/{id}`, `POST /users/{id}/reset-password`
**Frontend has:** `/master/users`, `/master/invite`
**Missing:**
- App-wide stats (total campaigns, active users, storage used)
- Server logs view
- Moderation tools
- Feature flags / toggles

---

## 5. API / Backend Gaps

### 🟡 5.1 No DELETE for Individual Combat Events
- `GET /encounters/{id}/events` exists
- `DELETE /combat-events/{id}` now implemented (GM-only, removes single event)

---

### 🟡 5.2 No PATCH for Effects at Encounter Scope
- `GET /encounters/{id}/effects` exists
- `PATCH /encounters/{id}/effects` now implemented: bulk remove by name, set active/inactive, add effect to multiple combatants

---

### 🟡 5.3 NPC Clone / Duplicate — ✅ (2026-08-04)
`POST /campaigns/{id}/npcs/{npc_id}/duplicate` — copies stats/image/visibility with a " (copy)" name suffix; FE duplicate button in the NPC list.

---

### 🟢 5.4 Campaign Archive / Restore — ✅ (2026-08-04)
`campaigns.archived_at` column; `POST /campaigns/{id}/archive` + `/restore` (master-only); list hides archived campaigns; FE toggle in the settings page.

---

### 🟢 5.5 No User Avatar Upload Endpoint
- `users` table has `avatar_url`
- Relies on generic `/uploads` with manual `campaign_id`
- No dedicated avatar upload

---

### 🟢 5.6 No Centralized File Attachments Table
- Image fields scattered: `image_key`, `portrait_url`, `icon_url`, `token_image`, `map_image`
- No `files` or `attachments` table with metadata (uploader, mime, size, campaign scope)
- No generic "attach file to NPC/quest/lore" feature

---

## 6. Architecture / Technical Debt

### ✅ 6.1 Combat Route Has Been Modularized
**Fixed (2026-05-04):** `combat.rs` was split into 8 submodules under `routes/combat/`:
- `mod.rs` (~442 lines) — shared helpers, fetch, tick
- `encounters.rs` (~479 lines) — encounter CRUD, initiative, turn order
- `combatants.rs` (~609 lines) — combatant CRUD, move, use_action
- `actions.rs` (~2,319 lines) — attack, damage, death-save, skill-check
- `spells.rs` (~519 lines) — cast-spell
- `special.rs` (~1,098 lines) — grapple, shove, class-feature, multiattack
- `tactical.rs` (~1,145 lines) — conditions, cover, lair, legendary
- `events.rs` — combat event log

---

### 🟡 6.2 No Centralized WS Event Schema
**File:** `backend/src/ws.rs`

- ~70 distinct event types emitted ad-hoc as JSON strings
- No enum, no contract, no validation
- Frontend parses generically (`Record<string, unknown>`)
- Typos in event names won't be caught at compile time

**Fix direction:** Define `WsEvent` enum in Rust, derive Serialize. Mirror in TypeScript frontend types.

---

### 🟢 6.3 `shared/` Contains No Runtime Shared Code
**Directory:** `shared/`

- Contains only spell-seeding scripts (`transform-spells.ts`, `seed_spell_effects.ts`)
- No shared types between backend and frontend
- Frontend types (`web/src/lib/types.ts`) manually mirror backend structs
- OpenAPI spec (`openapi.yaml`) exists but is likely stale

---

### 🟢 6.4 `docs/` Is Empty
- No feature specs
- No architecture docs
- No API usage guides
- No contributor onboarding

---

### 🟢 6.5 No Feature Flag System
- No runtime toggles
- No A/B testing framework
- No way to disable beta features
- All features always-on

---

## 7. WebSocket Event Completeness

### ✅ Well-Covered Domains
- Campaign lifecycle
- Messages (chat + whispers + edit/delete)
- Dice rolls
- Characters (CRUD + spells + rests)
- Sessions (CRUD)
- World (factions, NPCs, lore, news)
- Maps (CRUD + pins)
- Group (party, loot, quests)
- Combat (extensive — ~40 event types)
- Effects
- Notifications

### 🟢 Missing WS Events
- `presence_typing` — no "user is typing" indicator in chat
- `session_attendance_changed` — no attendance system
- `character_inventory_changed` — no inventory system
- `loot_claimed` / `loot_unclaimed` — no real-time loot updates beyond generic `loot_updated`
- `party_currency_changed` — no granular coin purse WS event
- `campaign_settings_changed` — no settings system
- `weather_changed` — no weather system

---

## 8. Summary by Category

| Category | 🔴 Critical | 🟡 High | 🟢 Medium | 🔵 Low |
|----------|------------|---------|----------|--------|
| Character Sheet | 3 | 3 | 1 | 0 |
| Combat | 0 | 2 | 2 | 0 |
| World Building | 0 | 4 | 5 | 0 |
| UX / QoL | 0 | 3 | 7 | 0 |
| API / Backend | 0 | 2 | 3 | 0 |
| Architecture | 0 | 2 | 3 | 0 |
| **Total** | **3** | **19** | **21** | **0** |

---

## 9. Recommended Priority Order

### Phase 1 — Core Character (🔴 Critical)
1. Normalize key character sheet fields out of JSONB (ability scores, inspiration, xp, hit dice, death saves)
2. Build `items` + `character_inventory` tables and API
3. Add `spell_slots` table or generated columns

### Phase 2 — GM Power Tools (🟡 High)
4. NPC / encounter templates + bestiary
5. In-game calendar + session attendance
6. Campaign settings / house rules
7. Bulk operations (invite, delete, award XP)

### Phase 3 — Player Experience (🟡 High)
8. User profile/settings page + password reset
9. Search/filter on all list pages
10. Loading states + delete confirmations
11. State persistence across navigation

### Phase 4 — World Depth (🟢 Medium)
12. Shops/merchants
13. Weather/environment
14. Travel/journey tracking
15. Tagging system
16. Player journal

### Phase 5 — Architecture (🟢 Medium)
17. Modularize `combat.rs`
18. Centralized WS event enum
19. `docs/` population
20. Feature flag system

---

*End of audit report. Use this as reference for feature planning and backlog prioritization.*
