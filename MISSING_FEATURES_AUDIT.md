# CinghialApp — Missing Features Audit

> Generated: 2026-04-30 | Last updated: 2026-06-09 (NPC search verified, hit die defaults, onboarding)
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

### 🟡 2.1 No Encounter / NPC Templates
**Tables missing:** `encounter_templates`, `npc_templates`, `bestiary_entries`

- Every encounter built from scratch
- Every NPC created manually
- No "spawn 5 goblins" or "add adult red dragon" quick-actions

**Impact:** GM prep is slow. Cannot save favorite encounters.

**Fix direction:**
1. `npc_templates` table (global SRD bestiary + campaign-specific)
2. `encounter_templates` table (pre-built encounter compositions)
3. `POST /encounters/{id}/spawn-from-template` endpoint

---

### 🟡 2.2 No Monster Catalog / Bestiary
**Related to 2.1**

- NPCs use `stats` JSONB
- No global monster reference table
- No CR-based filtering
- No automatic XP calculation from encounter composition

---

### 🟢 2.3 No Custom Spells / Homebrew
**Table:** `spells` is global SRD only

- No `campaign_spells` table
- GM cannot create homebrew spells
- Spell effects are hard-coded in `seed_spell_effects.ts`

**Fix direction:** Add `campaign_id` nullable FK to `spells` (NULL = global SRD), or create `campaign_spells` table.

---

### 🟢 2.4 No Campaign Settings / House Rules Table
**Column:** `campaigns.leveling` only config option

- No house rules storage
- No custom currencies
- No homebrew classes/races
- No campaign-specific modifiers

**Fix direction:** Add `settings` jsonb column to `campaigns` or create `campaign_settings` table.

---

## 3. World Building & Campaign Management

### 🟡 3.1 No In-Game Calendar / Time Tracking
**Tables missing:** `campaign_calendar`, `calendar_events`

- No world date tracking
- No moon phases
- No holidays/festivals
- No "session took place on Harvestide 15, 1492 DR"

---

### 🟡 3.2 No Weather / Environment Tracking
**Table missing:** `campaign_weather`

- No weather conditions per session
- No seasonal modifiers
- No environmental hazards tracking

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

### 🟢 3.5 No Tagging / Labeling System
**Tables missing:** `tags`, `taggable_tags`

- Cannot tag NPC as "villain"
- Cannot tag quest as "main-plot"
- Cannot tag map as "dungeon-level-2"
- Cannot filter by tags anywhere

---

### 🟢 3.6 No Player Journal / Private Notes
**Only `parties.shared_notes` exists**

- No per-player private notes
- No session journal entries
- No character backstory storage beyond `sheet` JSONB

---

### 🟢 3.7 No Player Attendance / Session RSVP
**Tables missing:** `session_attendance`

- No link between `users` and `campaign_sessions`
- Cannot track who attended which session
- No "who was present when this happened" for recap accuracy

---

### 🟢 3.8 No Campaign Handouts
**Could overlap with `news_entries` / `lore_entries`**

- No dedicated handout system
- No "reveal to players" mechanic for lore pieces
- No timed/drip-fed information

---

## 4. User Experience & Quality of Life

### 🟡 4.1 No User Profile / Settings Page
**Backend has:** `GET/PATCH /users/me`, `POST /users/me/change-password`
**Frontend missing:** `/profile` or `/settings` route

- Users cannot change display name, language preference, avatar
- Password change exists in API but no UI
- No dark/light mode toggle
- No notification preferences

---

### 🟡 4.2 No Self-Service Password Reset
**Backend has:** Admin-only `POST /users/{id}/reset-password`
**Missing:** Forgot-password flow with email/token

---

### 🟡 4.3 No Export / Import Anywhere
**Missing:**
- Character export (JSON/CSV/PDF)
- Character import
- Campaign export (full backup)
- Campaign import (restore)
- Session recap export (PDF)

---

### 🟡 4.4 No Bulk Operations
**Missing:**
- Bulk delete NPCs, lore, news
- Bulk invite (list of emails)
- Bulk add combatants to encounter
- Bulk award XP
- Bulk update character levels

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

### 🟢 4.7 No Search / Filter on Most Lists
**Missing search:**
- Loot items
- Quests
- Session recaps
- News articles (has pagination, no search)
- Members / invitations
- NPCs (has pagination + search + faction filter; pagination disabled when filters active)
- Factions
- Lore entries

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

### 🟡 5.3 No NPC Clone / Duplicate
- NPCs have full CRUD but no "duplicate this NPC" endpoint

---

### 🟢 5.4 No Campaign Archive / Restore
- Campaigns can be `DELETE`d permanently
- No `archived_at` soft-delete or archive/restore endpoints

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
