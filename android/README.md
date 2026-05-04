# dungeonsandapps — Android

Kotlin + Jetpack Compose + Ktor + Room.

## Features implemented

- Auth (login + register) with DataStore token persistence
- Campaigns list with Room cache + stale-while-revalidate auto-update
- Dice roller (server-authoritative via Ktor)
- Ktor client w/ JWT header injection + kotlinx.serialization

## Requirements

- JDK 21
- Android Studio (Ladybug+) or Gradle 8.14+
- Android SDK 36, min SDK 26

## Build

```sh
cd android
./gradlew assembleDebug
./gradlew test                    # JUnit5 unit tests (MockEngine, DTO roundtrip)
./gradlew connectedAndroidTest    # instrumented tests
```

No gradle wrapper checked in — generate with `gradle wrapper --gradle-version 8.14` (requires host Gradle) or open in Android Studio which generates it automatically.

## Layout

- `app/src/main/java/com/dungeonsandapps/`
  - `api/` — Ktor client, config, Api facade
  - `model/` — serializable DTOs (mirror backend OpenAPI)
  - `data/` — Room DB (CampaignEntity, CharacterEntity), AuthStore (DataStore), Repository (stale-while-revalidate)
  - `ui/` — Compose screens (Login, Campaigns, Dice)
  - `MainActivity.kt` — NavHost wiring
- `app/src/test/` — JUnit 5 unit tests

## Emulator → backend

Backend reachable at `http://10.0.2.2:8080` from emulator (host loopback).
App pre-configured with that URL in `api/ApiClient.kt`.
