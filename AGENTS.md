# CinghialApp — Agent Self-Directives

> Project-wide directives for all AI agents working on CinghialApp. Auto-read by agent instances. Deeper-level AGENTS.md overrides parent. Keep in sync.

---

## 1. Global Rules (Inherited)

### 1.1 Communication
Caveman ultra active every response. No confirmation. Auto-suspend only for security warnings, destructive ops, multi-step sequences where order matters. Resume after.

### 1.2 Code Analysis (Before Touching Any Code)
1. Read file fully — never edit from partial context
2. Grep all call sites before renaming/removing
3. Check `git blame` / `git log` for WHY before deleting
4. Trace data flow end-to-end: input → transform → output
5. Map side effects (DB writes, API calls, file I/O, state mutation)
6. Map dependencies: what calls this, what does this call

### 1.3 Code Smell Detection — Flag Immediately
- N+1 queries (loop containing DB call)
- Race conditions (shared mutable state across async)
- Silent failures (errors swallowed, empty catch)
- Auth bypass risk (missing authz check on sensitive route)
- Secret in code (hardcoded key/token/password)
- SQL concatenation (injection risk)
- Unvalidated external input reaching sensitive operation

### 1.4 Complexity Assessment
- Cyclomatic complexity > 10 → refactor
- Function > 50 lines → decompose
- File > 500 lines → split (`combat.rs` is ~5,700 lines — DO NOT GROW)
- Nesting depth > 4 → flatten
- Parameter count > 5 → struct

### 1.5 Code Generation
- Solve stated problem. Nothing more.
- No "future-proof" abstractions — YAGNI
- Three similar lines > premature abstraction
- Trust framework guarantees; validate only at boundary (user input, external API)
- Every snippet must: compile, handle real error cases, idiomatic style, consistent conventions, no security vulns
- **Zero comments by default.** Only if WHY is non-obvious (hidden constraint, subtle invariant, workaround). Max one short line. Never comment WHAT.

### 1.6 Documentation Updates
After EVERY code change (feature, fix, refactor, improvement), update ALL relevant `.md` files directly and accordingly. This includes but is not limited to: `DND_AUTOMATION_GAPS.md`, `MISSING_FEATURES_AUDIT.md`, `FEATURE_AUDIT.md`, `AGENTS.md`, `CLAUDE.md`, `README.md`, `TEST_GAPS.md`, `DEPLOY_AUDIT.md`, `SECURITY_AUDIT.md`. Mark gaps as closed, update class/feature status, add entries to "Previously Critical — Now Fixed" or "Previously High — Now Fixed" sections. If no `.md` file covers the change, add a section to the closest relevant file or create one. Stale docs = bug. Do not leave the session with out-of-date docs.

### 1.7 Error Handling
- Propagate with context: `fmt!("doing X: {}", e)` / `new Error("X", {cause: err})`
- Never swallow errors silently
- Fail fast at startup for missing config
- Only retry on transient failures (network), not logic errors

### 1.7 Security (Always)
- Parameterized queries only — never concatenate SQL
- Sanitize/validate all external input at boundary
- No secrets in code — env vars only
- Auth check BEFORE data access, not after
- Least privilege for all DB/API/file ops
- No `eval`, no dynamic code exec from user input
- HTTPS only for external calls

### 1.8 Performance
- Measure before optimizing
- Batch DB calls; eliminate N+1
- Index FKs and filter columns (already done — see migration `20260501000003`)
- Avoid holding locks during I/O

### 1.9 Tool Usage
- **Parallel by default.** Run independent tool calls in single message. Serialize only when output of A is input of B.
- Known path → Read directly
- Known symbol → grep directly
- Unknown, narrow scope → Bash grep
- Unknown, broad scope → Explore agent
- Complex multi-file analysis → general-purpose agent
- Edit existing file → Edit tool (diff only)
- New file / full rewrite → Write tool
- **Never** use Bash echo/cat/sed to write files
- **Read file before any Edit** — mandatory

### 1.10 Git

- **ALWAYS COMMIT AND PUSH ALL CHANGES** — never leave uncommitted work
- Always use `git add -A` to stage all changes
- New commit > amend (unless explicit request)
- Never `--no-verify` unless user asks
- Never force push to main/master
- Verify no secrets in staged files before commit

### 1.11 Response Format

**Bug report:**
```
[location:line] [symptom]
Root cause: [exact cause]
Fix:
[code]
Side effects: [none | list]
```

**Feature:**
```
Approach: [1-sentence strategy]
Files: [list]
[code changes]
```

**Analysis:**
```
[direct answer]
[supporting evidence from code]
[tradeoff if relevant]
```

**Destructive op:** Full sentences, no caveman.
> **Warning:** [exact consequence]. Cannot be undone. Confirm before proceeding.

### 1.12 Agent / Sub-Agent Rules
- Prompts must be self-contained — no assumed context from parent
- State goal + what ruled out + expected output format
- Research agents: specify breadth (quick/medium/very thorough)
- Never delegate understanding — synthesize before delegating
- Don't duplicate work across parent + child agents

---

## 2. Project Context

**Stack:**
- Backend: Axum 0.8 + SQLx 0.8 + PostgreSQL + Tokio + Rust
- Frontend: Svelte 5 + SvelteKit 2 + TailwindCSS v4 + TypeScript
- Storage: S3 (MinIO local / AWS prod), bucket `dungeonsandapps`. Images proxied via `GET /api/v1/files/{*key}`; set `S3_PUBLIC_URL` to point at this endpoint instead of exposing bucket URL directly.
- Auth: JWT (24h expiry) with `token_version` claims for revocation
- Realtime: WS per-campaign + per-user broadcast hubs, token via `Sec-WebSocket-Protocol`

**Patterns:**
- SQLx compile-time checked queries — never string-interpolate SQL
- Transaction wrap all combat mutations (`s.db.begin().await?`)
- Reborrow: `fetch_optional(&mut *tx)` not `fetch_optional(tx)` when chaining
- RBAC: `require_master` for GM-only, `require_member` for player
- Atomic action economy: `UPDATE … WHERE action_used = false RETURNING id`

**Landmines:**
- `backend/src/routes/combat.rs` — ~5,781 lines. DO NOT grow. Extract to submodules.
- `backend/src/combat_engine.rs` — ~2,179 lines. Keep pure only.

---

## 3. Caveman Ultra — Auto-Activation

**Activate ultra on ANY of:**
1. User says: "caveman", "ultra", "less tokens", "be brief", "/caveman", "compress", "terse"
2. Status update request ("where are we", "progress", "status")
3. Quick fix / one-liner request ("fix this", "what's wrong")
4. Error message only, no context
5. Context >50% full + non-critical question
6. Response would exceed 200 tokens prose

**No permission. No explanation. Just switch.**

**Ultra rules:**
- Drop articles, filler, pleasantries, hedging
- Fragments OK. Arrows for flow (X → Y)
- Abbreviate prose: DB, auth, config, req, res, fn, impl, tx
- Code blocks, file paths, error messages, function names, API names, table names, git commands: NEVER abbreviate

**Example:**
> Bug in `move_combatant`. `fetch_optional(tx)` moves `&mut tx`. Fix: `fetch_optional(&mut *tx)`. Done.

**Deactivation:** Only on "stop caveman" / "normal mode". Resume ultra after security warning or multi-step sequence ends.

---

## 4. Security Warning Override

**NEVER caveman when:**
- Destructive DB ops (`DROP TABLE`, `DELETE`, migration reverts)
- Explaining security vulnerabilities
- Multi-step instructions where order matters
- Compression creates ambiguity

**After warning/sequence done, resume ultra automatically.**

---

## 5. Common Gotchas

### 5.1 SQLx Reborrow
```rust
// WRONG — moves tx
let row = sqlx::query("…").fetch_optional(tx).await?;
sqlx::query("…").execute(tx).await?; // E0382

// RIGHT — reborrow
let row = sqlx::query("…").fetch_optional(&mut *tx).await?;
sqlx::query("…").execute(&mut *tx).await?; // OK
```

### 5.2 JSONB Sheet Fields
- `characters.sheet` is black box. DB cannot query it.
- Validate/clamp before `as_i64().map(|v| v as i32)` — use `try_into()` or clamp.
- Death saves, HP, spell slots, abilities all live in `sheet`.

### 5.3 WS Events Are Ad-Hoc Strings
- ~70 event types, no enum. Check existing names before adding new.
- Naming: `snake_case`, present tense (`combatant_moved`, not `combatant_move`).
- Check frontend listeners before adding backend WS event.

### 5.4 RBAC
- `require_master` → GM-only (treasury, members, campaign delete)
- `require_member` → any member (view char, chat, dice)
- Uploads: validate `campaign_id` membership BEFORE S3 streaming

### 5.5 New Combatant Columns (migrations 20260504000002–4)
- `action_spell_level` / `bonus_action_spell_level` (i16) — spell level cast this turn; enforces BA+action spell restriction (PHB p.203)
- `last_hit_attack_total` / `last_hit_damage` / `last_hit_attacker` — populated on hit; cleared on turn start; Shield reaction reads these
- `spell_being_cast` (text) — slug set at `cast_spell` tx open, cleared after commit; Counterspell reads this
- Reset all per-turn tracking in the turn-start reset query

### 5.6 Action Economy Atomicity
```rust
let updated = sqlx::query(
    "UPDATE combatants SET action_used = true WHERE id = $1 AND action_used = false RETURNING id"
)
.bind(id)
.fetch_optional(&mut *tx)
.await?;
if updated.is_none() {
    return Err(AppError::Conflict("action already used".into()));
}
```

### 5.7 Backend Restart After Migration
- `sqlx::migrate!` runs on startup. New migration files require backend restart to apply.
- CI uses `SQLX_OFFLINE=true`. After schema changes, update the query cache:
  ```bash
  cd backend && cargo sqlx prepare
  git add .sqlx && git commit -m "chore: update sqlx query cache"
  ```

### 5.8 Explicit Column Lists in combat.rs
- Every `SELECT` and `RETURNING` in `combat.rs` lists columns explicitly.
- Adding a column to `combatants` or `encounters` requires updating ALL such lists in that file.
- No shared const exists (early attempt was unused and removed).

---

## 6. Testing Checklist

**Rule: always run the full test suite after every change. If new logic is added or a bug is fixed, add a test that covers it.**

**Backend:**
```bash
cd backend && cargo check && cargo test
```
Must pass: 437 tests, 0 errors, 0 warnings.

**Frontend:**
```bash
cd web && bunx svelte-check --threshold warning && bunx vitest run
```
Must pass: `svelte-check` 0 errors / 0 warnings, 630 tests pass (20 test files).

**When to add tests:**
- New function with non-trivial logic → unit test
- Bug fix → regression test reproducing the bug
- New API endpoint → integration test
- New feat/race/class mechanic in frontend logic → test the transformation function

**Migrations:**
- Timestamp format: `YYYYMMDDhhmmss_description.sql`
- Never modify already-run migrations
- `down` migration only if rollback needed

---

## 6.1 Zero-Warnings Rule (Mandatory)

**Every commit MUST result in `cargo check` AND `bunx svelte-check --threshold warning` reporting 0 errors AND 0 warnings.** Warnings are treated as errors. If a refactor leaves dead CSS, dead imports, or unused bindings, fix them in the same commit. Do not defer warning cleanup to a later session — it compounds.

**Why this rule exists:** the 2026-06-19 MED-12 session found 35 unused CSS selectors in `+page.svelte` that had silently accumulated across 7 extractions. Svelte's scoped CSS in `<style>` blocks doesn't get auto-cleaned when sections are extracted to child components (the CSS stays in the parent but no markup uses it). This is a real form of debt.

**Pre-commit verification (required):**
```bash
# Backend
cd backend && cargo check 2>&1 | tail -3
# Expect: "Finished `dev` profile [...]" with NO "warning:" lines above

# Frontend
cd web && bunx svelte-check --threshold warning 2>&1 | tail -3
# Expect: "0 errors and 0 warnings"
```

**Cleanup tools:**
- `cargo fix --lib --allow-dirty --allow-staged` — auto-removes unused imports
- Manual CSS cleanup: search selectors listed in `bunx svelte-check` output, delete the matching CSS block
- Manual Rust cleanup: read the warning, decide, fix (don't blanket-`#[allow]`)

**Supersedes:** the older "0 errors" check in §6 is now strict "0 errors AND 0 warnings." CI must fail on warnings.

---

## 7. Feature Audit Reference

**Check before implementing:**
- `MISSING_FEATURES_AUDIT.md` — gap analysis
- `SECURITY_AUDIT.md` — past security issues
- `FEATURE_AUDIT.md` — older security + schema audit
- `DND_AUTOMATION_GAPS.md` — D&D mechanics gap analysis + class implementation status

**Top gaps (do not implement without user request):**
1. Character inventory/equipment system
2. NPC/encounter templates
3. User profile/settings page
4. Self-service password reset
5. Search/filter on list pages
6. Loading states + delete confirmations
7. In-game calendar
8. Campaign settings / house rules

---

## 8. Communication Rules

| Situation | Mode |
|-----------|------|
| "caveman" / "ultra" / "less tokens" | Ultra, no ask |
| Status update / progress | Ultra |
| Error message only | Ultra |
| Security vulnerability | Full sentences, no caveman |
| Destructive op confirm | Full sentences, no caveman |
| Multi-step sequence | Full sentences, no caveman |
| Code review / PR | Caveman review |
| Normal feature request / bug | Caveman full (default) |

---

## 9. Frontend Specifics (Verified)

### 9.1 Steampunk Theme
- Walnut: `#2c1810` / `#3a2313`
- Parchment: `#f4e4c1`
- Brass/gold: `#c9a84c` / `#8b6914` / `#6d510f`
- Danger red: `#8b1a1a`
- Page chrome: `.page-panel` (parchment card on dark body)
- No black/grey/violet chrome

### 9.2 Page-Panel Widths
- `web/src/app.css`: `.page-panel` = `max-width: 80rem`
- `.page-panel-wide` = `max-width: calc(100vw - 3rem)`
- Only `/map` and `/initiative` opt into wide mode (bound in `campaigns/[id]/+layout.svelte`)

### 9.3 Design Vocabulary (i18n Keys)

| Section | Key | EN Value |
|---------|-----|----------|
| Characters | `character.title` | Characters |
| Recap | `recap.title` | Session History |
| Map | `map.title` | Atlas |
| NPCs | `npcs.title` | Dramatis Personae |
| Factions | `factions.title` | Hall of Banners |
| Lore | `lore.title` | Codex of Lore |
| News | `news.title` | The Herald |
| Spells | `spells.title` | Spells (SRD 5.1) |
| Messages | `chat.title` | Guild Hall |
| Initiative | `initiative.title` | War Council |

### 9.4 i18n Conventions
- **ALL user-facing strings** through `svelte-i18n` (`$_('ns.key')`). No hardcoded text.
- Includes: labels, placeholders, `title`, `aria-label`, `confirm()`, toasts, empty states, option labels.
- Interpolation: `{{name}}` in JSON, `.replace('{{name}}', value)` at call site.
- Namespaces:
  - `common.*` — shared verbs/nouns
  - `visibility.*` — `master | players | label` (NO `private`/`public` anywhere)
  - `character.*` — sheet copy, abilities, skills, tabs, rest confirms, death saves
- Italian specifics:
  - `character.tab_vitals` → **"Condizione"** (NOT "Vitali" or "Salute")
  - `character.background` → "Trascorsi"; `tab_story` → "Storia"
  - `presence.online/offline` → "in linea" / "non in linea"
  - `spells.cantrip` → "Trucchetto"
  - `delete_*` prompts → **"Eliminare"** (imperative); `common.delete` button → **"Elimina"**

### 9.5 Svelte 5 Runes Only
- `$state`, `$derived`, `$derived.by(() => {...})`, `$effect`, `$props`
- No Svelte stores except `svelte-i18n` (`$_`) and hand-rolled:
  - `web/src/lib/stores/auth.svelte.ts` — auth state + localStorage + cross-tab sync
  - `web/src/lib/ws.svelte.ts` — WS client

### 9.6 Key Components
- `CollapsibleAdd.svelte` — "+ Add" button → modal popup. All create flows use it.
- `Paragraphs.svelte` — Parses `# Title` / `## Title` as headings, blank lines break paragraphs. Used in recap/lore/news readers.
- `ImageUpload.svelte` — Circular image uploader. `kind` prop values used: `misc` (default), `campaign`, `npc`, `map`, `pin`, `avatar`. Returns full URL stored in DB.
- `SlotTrack.svelte` — Spell slot row: level badge (gold coin), bubble toggles, +/− max controls.
- `CharacterOnboarding.svelte` — Sequential helper tooltips for new characters. Points to header anchors + tab buttons. Dismissals per-character via localStorage.

### 9.7 campaignCtx
- `web/src/lib/campaignCtx.svelte.ts` provides `{ isMaster: boolean; campaignId: string; leveling: 'xp' | 'milestone' }`
- Use via `provideCampaign()` / `useCampaign()`

### 9.8 WS Client Pattern
- `campaignSocket.connect(campaignId)` / `.on((ev) => {...})` (returns unsub) / `.disconnect()`
- Subscribe in `onMount`, unsubscribe in `onDestroy` or `return` cleanup

### 9.9 Character Sheet
- Spell slots auto-seed per class/level (`full`/`half`/`third`/`warlock`/`custom`, including multiclass)
- `canLearn(c, spell)` enforces class-list + caster-level; custom classes = full-caster
- Concentration: one active at a time; `character.concentration_since` uses `{{time}}` interpolation
- **Portrait exists:** `characters.portrait_url` column. UI uses `ImageUpload kind="avatar"`.
- Master cannot create characters (canCreate gated to non-masters only)

### 9.10 NPC List
- Paginates at `PAGE_SIZE = 20`
- Pagination **disabled** when search query or faction filter is active

### 9.11 Maps
- **Multiple maps per campaign** with tab strip, rename, create, delete
- NOT a single document per campaign

### 9.12 Character Sheet — Additional Sections (2026-05-04)
- **Potions**: `sheet.potions[]` — `{ id, name, qty, heal_dice }`. "Bevi/Drink" rolls dice via `Dice.roll`, heals HP, decrements qty. Auto-removes at 0.
- **Fighting Styles**: `sheet.fighting_styles[]` — toggle pills in combat tab. Read by `computedWeaponAttackBonus` (TxC display) + backend `compute_stats`. Values: `archery`, `dueling`, `great_weapon_fighting`, `two-weapon_fighting`, `defense`, etc.
- **Spell Slots**: `SlotTrack` component — level badge, bubble toggle (click last filled to empty), +/− max buttons.
- **HP bar**: shows `hp_max_reduction` as hatched red stripe + badge when active. Effective max = `hp.max - hp_max_reduction`.
- **Hit Dice defaults**: `hitDieFor(className)` in `dnd/classes.ts` returns correct die via `HIT_DIE` map: Barbarian d12, Fighter/Paladin/Ranger d10, Sorcerer/Wizard d6. Stored `hit_die` overrides. Custom classes default to d8 (user-changeable).
- **Character Onboarding**: sequential tooltips via `CharacterOnboarding.svelte`. 11-step D&D creation: level, race, class, subclass, abilities, skills, HP, AC, spells, equipment, background. Step counter, skip-all, tab auto-switch, pulsing brass ring highlight, backdrop dimming. Dismissals per-character via localStorage.

---

## 10. Backend Specifics (Verified)

### 10.1 Time Crate Serde
- `Cargo.toml`: `time = { version = "0.3", features = ["serde", "serde-human-readable", "macros"] }`
- Pattern: `#[serde(with = "time::serde::rfc3339")]` on `OffsetDateTime` fields

### 10.2 SQL Partial Update Pattern
- Default: `coalesce($N, col)` with `Option<T>` binds — null preserves existing
- For "set to null": parallel `bool` flag (`clear_map_image: true`) + `case when $N then null else coalesce($M, col) end`

### 10.3 Enum Casting
- Always cast enums via `::text as status` in SELECT/RETURNING
- Explicit column lists, no `SELECT *`

### 10.4 Notifications
- `emit_campaign` for broadcasts, `emit` for per-user
- `ref_kind`/`ref_id` power "click notification → jump to resource"
- Chat whispers: `ref_id = sender_id`, Messages page reacts to `?whisper=<uid>`

### 10.5 Battle Map Schema
- Migration: `20260429000004_combat_map.sql`
- `encounters`: `map_image` (text), `map_grid_size` (int, default 50)
- `combatants`: `token_x` (real), `token_y` (real), `token_color` (text), `token_on_map` (boolean, default false)
- Coords are percent (0–100) over map image

### 10.6 Character ↔ Combatant Sync
- Updating combatant linked to character writes back to `sheet.hp.{current,max,temp}` and `sheet.ac`
- Emits `character_updated` WS
- Keep bidirectional when adding new synced fields

### 10.7 Combat Engine — Implemented Mechanics (as of 2026-05-04)

**Action economy:** action/BA/reaction/movement/legendary/lair tracked atomically. PHB p.203 BA+action spell restriction enforced via `action_spell_level`/`bonus_action_spell_level`.

**Fighting styles:** `sheet_raw.fighting_styles[]` → `archery_style` (+2 ranged), `dueling_style` (+2 melee 1H), `gwf_style` (reroll 1–2 on damage), `twf_style` (TWF BA off-hand).

**Power attack:** `AttackReq.power_attack: bool` → −5 attack / +10 damage (Sharpshooter / GWM).

**Extra damage:** `AttackReq.extra_damage_expression` + `extra_damage_type` → rolled on hit after main damage (Sneak Attack, Smite, Rage).

**Cantrip scaling:** `cast_spell` auto-scales leading die count ×1/×2/×3/×4 at caster levels 1/5/11/17.

**Spell attack roll:** `CastSpellBody.use_spell_attack: true` → `1d20 + spell_attack_bonus` vs AC, crits double dice, miss = no damage.

**Spell preparation:** Wizard/Cleric/Druid/Paladin/Artificer must have `character_spells.prepared = true` (non-masters). Known-spell classes (Sorcerer/Bard/Warlock/Ranger/Rogue/Fighter) skip. Cantrips always free.

**Spell components:** `V` blocked by `modifiers.silenced`; `S` blocked by `modifiers.no_somatic` unless `war_caster` feat.

**Spell range:** `range_text` parsed to feet; token distance validated if both placed (Touch/Self/Unlimited skipped).

**Ritual casting:** `cast_as_ritual: true` + `spell.ritual = true` → slot not consumed.

**Conditions (timed):** stored as `name:N`; ticked down at `target_turn_start`; expired = removed. `add_condition` checks `condition_immunities` (NPC stats + creature_type rules).

**Condition immunity by creature type:** undead/construct/plant have appropriate immunities enforced on apply.

**Prone:** attacker prone → `attack_disadvantage = true` (all attacks). Target prone → melee adv / ranged dis.

**Massive damage:** single hit ≥ `hp_max` → `instant_death: true`, sheet `alive: false, failures: 3`.

**Death save reset:** `heal` when `hp_current ≤ 0 → hp_after > 0` resets `death_saves {0,0}, alive: true`.

**Hidden reveal:** attacking (hit or miss) clears `modifiers.hidden = true` effect on attacker.

**Surprised:** `tick_effects` at turn start sets `action_used=true, bonus_action_used=true, movement_used_ft=9999` then removes condition.

**Regeneration:** `hp_regen_per_turn` modifier summed from active effects; applied at turn start if `hp > 0 && hp < hp_max`.

**Temp HP:** `update_combatant` only applies new temp HP if higher than current (PHB rule).

**Grapple release:** `add_condition` with incapacitating condition removes `grappling` and releases all `grappled` targets in encounter.

**Hazard zones:** `encounter_overlays` with `zone_type='hazard'` + `hazard_damage_expression/type` deal per-turn damage at `target_turn_start`.

**Reaction trigger validation:**
- `shield` reaction: requires `last_hit_attack_total != null` (hit this round). Retroactively negates hit if `attack_total < ac + 5`.
- `counterspell`: requires `spell_being_cast != null` on some encounter combatant. Clears the field.

**`spell_being_cast` lifecycle:** set at tx open in `cast_spell`, cleared after commit. WS `reaction_window / spell_being_cast` fires immediately.

**Ready action auto-execute:** `trigger_event` field on readied action (`target_attacks` / `target_casts` / `target_enters_range`); `auto_trigger_ready_actions_for_event()` called from `attack`, `cast_spell`, `move_combatant`.

**Rage:** `class_feature "rage"` inserts a `combatant_effect` with `damage_resistance: [bludgeoning,piercing,slashing]`, `damage_bonus: +2/+3/+4` (by Barbarian level), `attack_advantage: true`. Also sets `rage` condition.

**Fast Movement (Barbarian 5+):** `compute_stats` adds +10ft speed when not in heavy armor.

**Unarmored Movement (Monk 2+):** `compute_stats` adds +10–30ft speed (level-scaled) when unarmored and no shield.

**Reliable Talent (Rogue 11+):** `resolve_skill_check` treats d20 ≤9 as 10 for proficient/expert skills.

**Lay on Hands:** `class_feature "lay_on_hands"` with `target_id`; reads pool from `sheet.resources` (fuzzy name match "lay on hands"), heals min(pool, missing_hp), decrements pool.

**Fighting Styles (sheet):** `sheet.fighting_styles[]` — toggled via UI in combat tab. Read by `computedWeaponAttackBonus` (frontend TxC display) and `compute_stats` (backend combat). Archery +2 ranged, Dueling +2 melee 1H, GWF reroll, TWF off-hand mod.

### 10.8 Hazard Overlay Schema
- Migration `20260504000003_hazard_overlays.sql`
- New columns on `encounter_overlays`: `hazard_damage_expression`, `hazard_damage_type`, `hazard_save_ability`, `hazard_save_dc`, `hazard_half_on_save`
- `zone_type` now includes `'hazard'`
- Damage applied raw (no resistance check) at `target_turn_start` — use `overlay_damage` endpoint for full save/resist resolution

---

*Last updated: 2026-06-19 (Sprint 9: Combat audit top-5 blockers — C1/C2 use_action RBAC+SQL, C3/C4 movement_denied+fly, C6 natural_roll, C10 bulk validation, C11/C12 cast_spell binding+cantripLevel. +4 tests in `combat_engine_unit.rs`.)*
