# cinghialapp — CLAUDE.md

Private, self-hosted D&D 5e campaign manager. Three apps share one Postgres:
`backend/` (Rust/Axum/SQLx), `web/` (SvelteKit + Svelte 5 runes + Tailwind),
`android/` (Kotlin companion). SRD spells live in `shared/`.

## Repo layout

```
backend/          Rust + Axum API, SQLx, WebSocket pub/sub, S3/MinIO uploads
web/              SvelteKit app (all user-facing UI; primary surface)
android/          Kotlin companion app
migrations/       Top-level SQL migrations (run by backend on startup)
shared/           SRD seed data (spells)
docker-compose.yml  Postgres + MinIO
```

## Stack rules (durable preferences)

- **Bun + latest deps**: always use Bun, always latest versions of every dep.
- **Tests always, run always**: every change ships with tests; run the full suite after every change (`cargo test` in `backend/`, `bun run test` + `bun run check` in `web/`).
- **Docker for DBs**: never use host-OS DB installs; Postgres + MinIO run via `docker compose` only.
- **Steampunk theme**: all UI aligns with walnut (`#2c1810`/`#3a2313`), parchment (`#f4e4c1`), brass/gold (`#c9a84c`/`#8b6914`/`#6d510f`). No black/grey/violet chrome. Page chrome is `.page-panel` (parchment card on dark body). Accent red `#8b1a1a` for danger only.
- **Always use i18n**: every user-facing string routes through `svelte-i18n` (`$_('ns.key')`) with parity between `web/src/lib/i18n/en.json` and `web/src/lib/i18n/it.json`. No hardcoded text anywhere users read it — labels, placeholders, `title`, `aria-label`, `confirm()`, toast bodies, empty states, option labels. `{{name}}` interpolation + `.replace('{{name}}', value)` at the call site.

## Dev environment

- Services: `docker compose up -d postgres minio` (MinIO web UI :9001, Postgres :5432).
- Backend: `cd backend && cargo run` (migrations auto-apply on startup).
- Web: `cd web && bun run dev`.
- Dev master account (seeded in local Postgres) — creds stored in user's auto-memory.

## i18n parity check

Bun one-liner that must report balanced keys:

```bash
cd web && bun -e "
const en = JSON.parse(require('fs').readFileSync('./src/lib/i18n/en.json','utf8'));
const it = JSON.parse(require('fs').readFileSync('./src/lib/i18n/it.json','utf8'));
function walk(o, p='', a=[]) { for (const k in o) { const np=p?p+'.'+k:k; if (o[k]&&typeof o[k]==='object') walk(o[k], np, a); else a.push(np); } return a; }
const a = new Set(walk(en)), b = new Set(walk(it));
const miA = [...a].filter(x => !b.has(x)), miB = [...b].filter(x => !a.has(x));
console.log(miA.length || miB.length ? {miA, miB} : 'OK ' + a.size);
"
```

### i18n conventions

- `common.*` — shared verbs/nouns (`add/create/cancel/save/delete/edit/close/search/all/none/loading/description/name/title/body/category/remove/end`).
- `visibility.*` — `master | players | label`. The app is master/players only; there is no `private`/`public` anywhere (DB, backend, UI).
- `character.*` — sheet copy: ability keys (`ability_str/dex/con/int/wis/cha`), skills (`skill_*`), tabs (`tab_vitals/combat/magic/loot/features/story`), rest confirms (`short_rest_confirm/long_rest_confirm`), death-save labels/aria, `concentration_since` with `{{time}}`, etc.
- `delete_*` prompts are imperatives in Italian → **"Eliminare"** ("Eliminare X?" / "Eliminare definitivamente…"). The bare `common.delete` button stays **"Elimina"** (action label). Don't conflate the two.
- Italian translations for standalone concepts:
  - `character.tab_vitals` / `character.vitals` → **"Condizione"** (user explicitly rejected "Vitali" and "Salute").
  - `character.background` → "Trascorsi" (story tab section); `tab_story` → "Storia".
  - `presence.online/offline` → "in linea" / "non in linea".
  - `spells.cantrip` → "Trucchetto".

## Design vocabulary per section

Each major section has a themed title (EN / IT) and a shared header pattern: centered title in `IM Fell English SC`, italic subtitle framed by `❦` fleurons, horizontal rule with centered fleuron, parchment-body card below.

| Section     | EN title        | IT title              |
|-------------|-----------------|-----------------------|
| Characters  | Characters      | Personaggi            |
| Recap       | Session History | Cronologia sessioni (Chronicle timeline) |
| Map         | Atlas           | Atlante               |
| NPCs        | Dramatis Personae | —                  |
| Factions    | Hall of Banners | —                     |
| Lore        | Codex           | —                     |
| News        | Herald          | —                     |
| Spells      | Spells (SRD 5.1) | —                    |
| Messages    | Guild Hall      | Sala del Gilda        |
| Initiative  | War Council     | Consiglio di Guerra   |

### Page-panel widths

Default `.page-panel` caps at `max-width: 80rem`. `/map` and `/initiative` opt in to `.page-panel-wide` (`max-width: calc(100vw - 3rem)`) via the campaign layout so battle/world maps have room. Add new wide routes by extending the class-binding in `web/src/routes/campaigns/[id]/+layout.svelte`.

## Backend notes

- `backend/src/routes/combat.rs` has the richest route surface; it also handles token/battle-map state.
- RBAC: `rbac::require_member` / `rbac::require_master`; `Role::Master` is the escalation check. Campaign owner is master; admins inherit.
- Character ↔ combatant HP/AC sync: updating a combatant linked to a character writes back into `sheet.hp.{current,max,temp}` and `sheet.ac`, and emits `character_updated` WS. Keep this bidirectional when adding fields.
- Time crate: we depend on `time` with `serde-human-readable`; serde attribute for `OffsetDateTime` uses `#[serde(with = "time::serde::rfc3339")]`.
- SQL pattern for partial updates: `coalesce($N, col)` with `Option<T>` binds. Null fields preserve existing. For "set to null" flags, use a parallel `bool` (`clear_map_image`) and a `case when $N then null else coalesce($M, col) end`.
- `sqlx::query_as` for structs; always select explicit column lists and cast enums via `::text as status`.
- WS publish pattern: `ws::publish(campaign_id, json!({"type":"...","id":id}).to_string())`.
- Notifications: `emit_campaign` for broadcasts, `emit` for per-user pings. `ref_kind/ref_id` power "click notification → jump to resource" (chat whispers pass `ref_id = sender_id` and the Messages page reacts to `?whisper=<uid>`).

## Frontend notes (web)

- Svelte 5 runes: `$state`, `$derived`, `$derived.by(() => {...})`, `$effect`, `$props`. No stores except `svelte-i18n` (`$_`) and the hand-rolled `auth.svelte.ts` / `ws.svelte.ts`.
- WebSocket client: `campaignSocket.connect(cid)` / `.on((ev) => {...})` / `.disconnect()`. Subscribe per-section in `onMount`, unsubscribe in `onDestroy`.
- `campaignCtx.svelte.ts` provides `isMaster/campaignId/leveling` to nested pages via `provideCampaign` / `useCampaign`.
- `CollapsibleAdd` component renders the "+ New X" buttons as modals. All create flows across sections use it for consistency.
- `Paragraphs.svelte` parses multi-paragraph text: `# Title` / `## Title` starts a titled block; blank lines break paragraphs. Used in recap/lore/news readers.
- Character sheet: spell-slot seeding is auto per-class/level (full/half/third/warlock/multiclass). `canLearn(c, spell)` enforces class-list + caster-level access — custom classes get full-caster treatment. Spell description modal is available from the sheet *and* from the spell book.
- NPC list paginates; empty search still shows the paginated list.
- Character sheet ↔ combatant sync is realtime (WS), so combat HP changes reflect instantly on the sheet.
- Concentration: one active at a time; `character.concentration_since` uses `{{time}}` interpolation.

## Initiative — War Council (battle map)

The Initiative page hosts both the initiative roster **and** a full combat map.

### Schema (`migrations/20260429000004_combat_map.sql`)

```sql
alter table encounters
    add column map_image text,
    add column map_grid_size int not null default 50;

alter table combatants
    add column token_x real,
    add column token_y real,
    add column token_color text,
    add column token_on_map boolean not null default false;
```

Coords are percent (0–100) over the uploaded map image — resolution-independent.

### Backend routes (`routes/combat.rs`)

- `PATCH /encounters/:id` — master. Accepts `map_image`, `clear_map_image: true`, `map_grid_size` (20–200). Emits `encounter_updated`.
- `PATCH /combatants/:id` — master. Accepts `token_x/y/color/on_map`.
- `POST /combatants/:id/move` — master OR the character's owner. Clamps coords to `0..100`; sets `token_on_map = true`; emits `combatant_moved`.

### Frontend (`web/src/routes/campaigns/[id]/initiative/+page.svelte`)

- `view` state toggles **Roster** / **Battle Map** via a brass tab strip.
- Map view (master): ImageUpload in a toolbar, grid-size input, "Place all tokens" auto-layout (players left column 20%, NPCs right column 80%, evenly spaced), "Take off map" per-token, off-map tray chips to return.
- Map view (player): drag your own token only. Drag others → 403 (both backend and UI-gated). `canMoveToken(c)` centralizes the permission check.
- Active turn's token glows gold on the map.
- `combatant_moved` WS events patch local state in place (no reload flicker during someone else's drag). During your own drag, incoming events for your token are skipped.
- Token visuals: initial-letter disc, color from `token_color` override or hash of character/id, HP mini-bar, player tokens outlined gold, NPC tokens outlined red.

### E2E test (`backend/tests/e2e.rs::combat_battle_map_tokens`)

Covers: master uploads map + sets grid; Alice moves own token; Alice blocked from Bob's (403); master moves Bob's; coord clamping; NPC unreachable by player (403); token_color patch by master; non-member 403.

## File upload (MinIO/S3)

- `ImageUpload.svelte` handles uploads. `kind` options: `map`, `npc`, `pin`, `campaign`, `character` — keep these in sync with backend allowlist.
- Upload returns a full URL (via presigned or public bucket) stored directly in DB columns (`image_key`, `icon_url`, etc.).

## Android

Kotlin companion app exists in `android/` — ask before changing; most work happens in `web/` + `backend/`.

## Gotchas

- Running a migration requires restarting the backend process (`sqlx::migrate!` runs on startup).
- When adding a column to `combatants` or `encounters`, update the explicit column lists in `combat.rs` (each `select`/`returning` lists columns; there's no shared const anymore — the early attempt to extract one was unused and removed).
- Svelte-check and the full vitest suite are the bar for "done" on the web side; `cargo test` for the backend. If the user says "finish" or "full pass", run all three plus the i18n parity one-liner.
- `confirm()` prompts and `alert()` strings count as user-facing text; route them through i18n (`character.short_rest_confirm`, `character.long_rest_confirm`, `character.delete_character_confirm` with `{{name}}`, etc.).
- The `/map` world map is a single document per campaign — don't reintroduce the name/tabs UI. Upload-only, auto-creates on first image.
- Character portraits are not yet modeled; tokens fall back to initial + colored disc. If portraits land, extend `tokenBg`/`tokenInitial` to prefer the image.
