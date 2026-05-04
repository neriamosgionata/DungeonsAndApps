# dungeonsandapps

Private D&D 5e campaign management app.

## Stack

- **Backend:** Rust + Axum + SQLx + PostgreSQL (latest), JWT auth, WebSocket realtime
- **Web:** Svelte 5 (runes) + SvelteKit + Tailwind + Bun, PWA
- **Android:** Kotlin + Jetpack Compose + Ktor client, Room cache
- **Storage:** PostgreSQL + S3/MinIO
- **Shared:** OpenAPI contract, SRD 5.1 spells JSON

## Layout

```
dungeonsandapps/
├─ backend/        rust axum server
├─ web/            sveltekit 5 runes
├─ android/        kotlin compose
├─ shared/         openapi.yaml, spells-srd.json
├─ migrations/     sqlx migrations
└─ docker-compose.yml
```

## Features

Character sheet (5e), Recap, Map (world + pins), NPCs, Factions, Lore, News, Enchantments (SRD), Group (coin/loot/notes/quests), Messages (+ master whispers), Dice (server-auth), Initiative/Combat (encounters).

## Roles

- **Player** — own character + campaign data + master-visible world info
- **Master** — full CRUD, private rolls, combat control, visibility toggles

## Dev

```sh
docker compose up -d            # postgres + minio
cd backend && cargo run         # api on :8080
cd web && bun install && bun dev  # web on :5173
```

## Rules

- Always Bun, always latest deps.
- Always tests, always run tests on every change.
- SRD content only — no proprietary WotC material in repo.
