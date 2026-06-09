# Test Coverage Gap Analysis

## Current State

### Backend (Rust)
- **Source:** ~10,800 lines
- **Tests:** ~6,200 lines (57% ratio)
- **Test Count:** 437 tests across 29 test files
- **Status:** Good coverage on combat, auth, API endpoints

### Frontend (TypeScript/Svelte)
- **TS Source:** ~2,900 lines → **Tests:** ~2,800 lines (97% ratio) ✅
- **Svelte Source:** ~19,700 lines → **Tests:** 0 lines (0% ratio) ❌
- **Test Count:** 626 tests across 19 files (all TS utilities + pure function modules)
- **Status:** Business logic covered, UI untested

---

## Backend Gaps

| Module | Priority | Gap |
|--------|----------|-----|
| `ws.rs` (WebSocket) | Medium | No integration tests for actual WS connections |
| `uploads.rs` | Medium | No S3/MinIO upload flow tests |
| `rbac.rs` | Low | Direct middleware tests (covered indirectly) |
| `rate_limit.rs` | Low | Direct rate limit tests (covered indirectly) |
| `config.rs` | Low | No config validation tests |

**Recommendation:** Backend coverage is good. Add WS integration tests if needed.

---

## Frontend Gaps (Major)

### 1. Svelte Components (19,429 lines) - CRITICAL
| Component | Lines | Risk |
|-----------|-------|------|
| `character/+page.svelte` | 4,701 | HIGH - Core feature |
| `initiative/+page.svelte` | 4,103 | HIGH - Combat system |
| `npcs/+page.svelte` | 786 | MEDIUM |
| `map/+page.svelte` | 610 | MEDIUM |
| `SlotTrack.svelte` | 141 | LOW - Reusable |
| `EffectBadge.svelte` | ~80 | LOW - Reusable |
| 12 other components | ~2,500 | LOW |

**Blocked by:** Svelte 5 SSR incompatibility with @testing-library/svelte

**Workaround:** Use Playwright E2E tests

### 2. Store/State Management
| Store | Status |
|-------|--------|
| `auth.svelte.ts` | Partial (logic tested) |
| `campaignCtx.svelte.ts` | Partial (logic tested) |
| `notifications.svelte.ts` | ❌ Untested |
| `ws.svelte.ts` | ❌ Untested |

### 3. Route Logic
| Area | Status |
|------|--------|
| Form validation | ✅ Tested (validation.test.ts) |
| Form submission | ❌ Untested |
| Route guards | ❌ Untested |
| Data fetching | ❌ Untested |

### 4. E2E Tests
| Flow | Status |
|------|--------|
| Auth (login/register) | 🟡 Basic structure |
| Campaign CRUD | 🟡 Basic structure |
| Character creation | 🟡 Basic structure |
| Combat | ❌ Not started |

---

## Risk Assessment

### HIGH Risk (Untested Core Features)
1. **Character sheet UI** - 4,701 lines, no tests
2. **Combat initiative UI** - 4,103 lines, no tests
3. **Form submissions** - Could break with refactoring

### MEDIUM Risk
1. WebSocket reconnection logic
2. Image upload flow
3. Real-time combat updates

### LOW Risk (Well Tested)
1. D&D calculations ✅
2. Spell slot math ✅
3. API client ✅
4. i18n translations ✅
5. Onboarding steps logic ✅ (25 tests)
6. Class/subclass data ✅ (30 tests)
7. Item catalog ✅ (13 tests)

---

## Recommendations

### Immediate (High ROI)
1. **Add Playwright E2E tests** for critical flows:
   - Login → Create campaign → Create character
   - Combat: Start encounter → Attack → End turn

2. **Test store methods** that are pure functions

### Short Term
1. Wait for Svelte 5 testing library fixes
2. Add component visual regression tests (Chromatic/Storybook)

### Long Term
1. Extract more logic from components to testable TS files
2. Integration tests for WebSocket flows

---

## Test Command Reference

```bash
# Backend
cd backend && cargo test  # 437 tests

# Frontend unit
cd web && bunx vitest run        # 626 tests (19 files)

# Frontend E2E
cd web && bunx playwright test  # 3 basic tests
```

## Summary

| Category | Coverage | Risk |
|----------|----------|------|
| Backend API | 85% | LOW |
| Frontend Utils | 90% | LOW |
| Frontend Components | 0% | **HIGH** |
| E2E Flows | 5% | MEDIUM |

**Biggest Risk:** 19,429 lines of Svelte UI with zero automated tests. Any refactoring risks breaking core user flows.
