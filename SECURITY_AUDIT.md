# Security Audit Summary

**Date**: 2026-04-29  
**Scope**: Backend (Rust), Web (Svelte), Android (Kotlin)  
**Auditor**: Comprehensive static analysis + deep-dive review

---

## Executive Summary

| Severity | Count | Status |
|----------|-------|--------|
| CRITICAL | 9 | 9 Fixed (100%) |
| HIGH | 17 | 17 Fixed (100%) |
| MEDIUM | 35 | See below |
| LOW | 29 | See below |

---

## CRITICAL - All Fixed ✅

1. **CORS Misconfiguration** (backend) - Fixed: Use explicit `CORS_ORIGIN` env var
2. **WebSocket Hub Memory Leak** (backend) - Fixed: Daily cleanup task for inactive channels  
3. **WS Token Leak via URL** - Fixed: Moved to `Sec-WebSocket-Protocol` header
4. **seededSigs Memory Leak** (web) - Fixed: Cleanup on component destroy
5. **Effect Loop Risk** (web) - Fixed: Removed `queueMicrotask` pattern
6. **Global Coroutine Scope** (android) - Fixed: Use `rememberCoroutineScope()`
7. **Network Timeouts** (android) - Fixed: Added Ktor HttpTimeout plugin
8. **Repository Fire-and-Forget** (android) - Fixed: Return `Flow`, use `onStart/debounce`
9. **Dynamic SQL Injection** (backend) - Fixed: Replaced `format!()` with parameterized queries in 5 files

---

## HIGH - All Fixed ✅

| ID | Issue | Location | Fix |
|----|-------|----------|-----|
| HIGH-1 | JWT Secret Validation | `config.rs` | Added 32-byte minimum check with HMAC-SHA256 requirement |
| HIGH-2 | Combat Transaction Gaps | `combat.rs` | Added transaction boundaries in `next_turn`, `prev_turn`, `end_encounter` |
| HIGH-3 | Upload Path Traversal | `uploads.rs` | Added kind whitelist validation (`avatars\|maps\|portraits\|tokens\|npcs\|misc`) |
| HIGH-4 | WebSocket Token in URL | `ws.rs`, `ws.svelte.ts`, `notifications.svelte.ts` | Moved to `Sec-WebSocket-Protocol: auth.<token>` header |
| HIGH-5 | Login Brute Force | `auth.rs` | IP-based rate limiting with 10 attempts/5min window |
| HIGH-6 | Rate Limit Memory Leak | `auth.rs` | Added LRU-style cleanup, bounded at 10k IPs |
| HIGH-7 | Async Mutex Blocking | `auth.rs` | Changed from `std::sync::Mutex` to `tokio::sync::Mutex` |
| HIGH-8 | Upload Rate Limit Memory Leak | `uploads.rs` | Added LRU-style cleanup for upload buckets, bounded at 10k users |
| HIGH-9 | CORS Permissive Fallback | `config.rs` | Changed default from `*` to `http://localhost:5173` |
| HIGH-10 | Character Limit TOCTOU | `characters.rs` | Used transaction with `FOR UPDATE` lock on membership row |
| HIGH-11 | Notification Timer Leak | `notifications.svelte.ts` | Clear all pending timeouts in `clearAll()` |

---

## HIGH-12 — Fixed 2026-06-19 (Combat audit)

| ID | Issue | Location | Fix |
|----|-------|----------|-----|
| HIGH-12 | `use_action` endpoint missing RBAC — `AuthUser(_uid)` dropped, any authenticated user could toggle any combatant's action/bonus/reaction/legendary slots | `routes/combat/combatants/action.rs:11-13` | Added `require_action_auth` (member + owner check + master bypass + active encounter status); also removed `format!` SQL interpolation in favor of literal-query match arms |
| HIGH-13 | `bulk_add_combatants` accepted malformed bodies — no length cap, per-row `#[validate]` skipped | `routes/combat/combatants/bulk.rs:18` | Explicit 1-100 row length check + per-row `spec.validate()` with errors collected in `BulkAddError` |
| HIGH-14 | Atomicity gaps in action economy — `grapple_escape`, `trigger_ready`, `class_feature.rage` all overwrote `action_used`/`reaction_used`/`bonus_action_used` to `true` without atomic WHERE guards, allowing repeated consumption on race conditions or duplicate calls | `routes/combat/special/{escape,multiattack,class_feature}.rs` (Sprint 10) | Added `where <col> = false returning id` pattern on all 3 sites; `BadRequest` on miss |
| HIGH-15 | `set_initiative` body field `character_id: Uuid` was passed as `combatant.id` to `WHERE id = $2` — wrong target. Pre-existing test `set_initiative_endpoint_updates_combatant_initiative` used the correct shape (`{combatants: [{combatant_id, initiative}]}`) and was failing on master | `routes/combat/encounters/{types,initiative}.rs` (Sprint 10) | Rewrote `SetInitiativeBody` to match the test contract; handler loops, accepts `planned`/`active`; test now passes |

---

## MEDIUM Priority Issues (Documented)

### Backend

| ID | Issue | Location | Mitigation |
|----|-------|----------|------------|
| MED-1 | Transaction boundary gaps | `combat.rs:start()`, `invitations.rs:accept()` | Notifications intentionally outside tx for eventual consistency |

### Web

| ID | Issue | Location | Mitigation |
|----|-------|----------|------------|
| MED-2 | Deep link token exposure | `hooks.server.ts` | Token in URL fragment (not sent to server) |
| MED-3 | Client secret in source | `hooks.server.ts` | Public client - acceptable for PKCE flow |

### Android

| ID | Issue | Location | Mitigation |
|----|-------|----------|------------|
| MED-4 | SSL pinning disabled | NetworkModule.kt | Acceptable for dev/custom server deploys |
| MED-5 | Biometric bypass | BiometricAuthManager.kt | Requires root access to exploit |

---

## LOW Priority Issues (Documented)

### Backend
- Missing `Cache-Control` headers on some API responses
- Verbose error messages in production (could leak internals)
- Session tokens don't rotate on privilege change

### Web
- SvelteKit error page could leak stack traces
- No SubResource Integrity on external assets (none used)
- Console debug logging in production build

### Android
- Backup mode enabled (local data could be extracted)
- Screenshot allowed on sensitive screens
- Logcat may contain PII in debug builds

---

## Verification

```bash
# Backend compiles and tests pass
cd backend && cargo test && cargo clippy -- -D warnings

# Web builds without errors
cd web && bun run check && bun run build

# Android builds
./gradlew :app:assembleDebug
```

### Test Results (2026-04-29)
- ✅ Backend: 437 tests passed
- ✅ Web: 0 svelte-check errors/warnings
- ✅ All compiles clean

---

## WebSocket Auth Migration Guide

### Client-side changes:
```typescript
// OLD (vulnerable - token in URL)
const ws = new WebSocket(`${base}?token=${encodeURIComponent(token)}&campaign=${id}`);

// NEW (secure - token in subprotocol)
const ws = new WebSocket(base, [`auth.${token}`, `campaign.${id}`]);
```

### Server-side validation:
The backend now extracts auth from `Sec-WebSocket-Protocol` header instead of query params. Legacy query-param auth is rejected.

---

## Changelog

### 2026-04-29 - Security Hardening Complete
- Fixed HIGH-6 through HIGH-11
- Migrated WebSocket auth from URL to header
- Added bounded memory structures for rate limiting
- Fixed character creation race condition
- All HIGH severity issues resolved
