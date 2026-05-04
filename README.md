# DungeonsAndApps

Private D&D 5e campaign management app.

## Stack

- **Backend:** Rust + Axum + SQLx + PostgreSQL, JWT auth, WebSocket realtime
- **Web:** Svelte 5 (runes) + SvelteKit + TailwindCSS v4 + Bun
- **Android:** Kotlin + Jetpack Compose + Ktor + Room
- **Storage:** PostgreSQL + S3/MinIO (local) / AWS S3 (prod)
- **Infra:** AWS EC2 t4g.small + Terraform + GitHub Actions CI/CD
- **Shared:** OpenAPI contract, SRD 5.1 spells JSON

## Layout

```
DungeonsAndApps/
├─ backend/        Rust Axum server (port 8080)
├─ web/            SvelteKit static frontend (port 5173)
├─ android/        Kotlin Compose app
├─ shared/         openapi.yaml, spells-srd.json, transform scripts
├─ migrations/     SQLx migrations
├─ infra/          Terraform (EC2 + S3 + SSM + Route53 + GitHub secrets)
├─ scripts/        deploy.sh
├─ nginx/          nginx.prod.conf
├─ docker-compose.yml       local dev
└─ docker-compose.prod.yml  production
```

## Features

Character sheet (5e full), Recap (session history), Atlas (maps + pins), Dramatis Personae (NPCs), Hall of Banners (factions), Codex of Lore, The Herald (news), Spells (SRD 5.1), Guild Hall (chat + whispers), War Council (initiative + combat engine).

## Roles

- **Player** — own character, campaign data, master-visible world info
- **Master** — full CRUD, private rolls, combat control, visibility toggles

## Dev

```sh
docker compose up -d              # postgres + minio
cd backend && cargo watch -x run  # API on :8080
cd web && bun install && bun dev  # web on :5173
```

Seed master account:

```sh
DATABASE_URL=postgres://cinghiale:cinghiale@localhost:5432/dungeonsandapps \
  cargo run --manifest-path backend/Cargo.toml --bin seed_master \
  -- master@dungeonsandapps.local <password> 'Game Master'
```

## Deploy

First time:

```sh
bash infra/bootstrap.sh
```

Subsequent deploys: push to `master` — CI builds, tests, and deploys automatically.

See `infra/README.md` for full infra docs.

## Rules

- Always Bun, always latest deps.
- Always tests, run suite on every change.
- SRD content only — no proprietary WotC material.
