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

## HIGH bugs uncovered by 2026-06-22 combat audit (test suite does not cover)

The 437-test backend suite passes with 0 errors / 0 warnings, but the 2026-06-22 deep-dive found 5 HIGH bugs in production code that the test suite does NOT exercise. **All 5 produce visibly wrong play in regular game sessions.** See `COMBAT_AUDIT.md` for full detail.

| ID | Bug | Path NOT covered | Test to add |
|----|-----|------------------|-------------|
| HIGH-16 | Multiattack target reorder: parsed attacks zip onto reordered `targets`; final loop iterates ORIGINAL `body.targets` by index → wrong damage to wrong target | `special/multiattack.rs:56-105,184-219` `try_parse_npc_multiattack` path with 2+ parsed attacks | Set up NPC with multiattack + 2+ targets; verify damage goes to correct `target_id`, not index-shifted |
| HIGH-17 | Within-5ft threshold uses 5% of map; PHB 5ft = 20% of map. Auto-crit (paralyzed/unconscious) + prone-advantage fire only at <1.25ft | `combat_engine/resolvers/attack.rs:42-58,198-213` | Place attacker 6ft (24%) from paralyzed target; verify auto-crit DOES fire (currently fails) |
| HIGH-18 | Auto-cover `cover="full"` (≥3 blockers) → 0 AC bonus (dead branch); total cover gives 0 instead of blocking attack | `actions/combat/attack.rs:216-220` + `combat_engine/resolvers/attack.rs:22-26` | Place 3+ blockers between attacker and target; verify attack rejected with BadRequest (currently succeeds with 0 bonus) |
| HIGH-19 | Spell range formula broken: `dist_ft = g_size * dist_pct`. 150ft Fireball targets things within 3% of caster. Same bug in attack/opportunity/twf | `spells/cast.rs:307-322` | Cast Fireball with targets at varying distances; verify targets within 150ft are hit, beyond 150ft are not (currently broken) |
| HIGH-20 | `apply_hp_damage` does NOT clamp HP to 0. 0-HP target taking damage → `hp_current = -X` in DB | `combat_engine/resolvers/damage_type.rs:51-61` + `damage.rs:17` | Deal damage to 0-HP combatant; verify `hp_current = 0` not negative (currently goes negative) |

**Action item:** add 5 regression tests in next sprint before fixing the underlying bugs. Tests should fail on current code, pass after fix.

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
2. **Combat initiative UI** - 3,021 lines, no tests (post-MED-12: was 4,103)
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
8. Combat engine (resolved mechanics) ✅ (132 + 49 = 181 unit tests after Sprint 9; +4 in Sprint 9: `compute_stats_paralyzed_with_fly_speed_still_zero`, `compute_stats_stunned_with_fly_speed_still_zero`, `compute_stats_fly_speed_uses_higher_of_walk_or_fly`, `compute_stats_fly_only_creature_uses_fly_speed`; +1 in Sprint 10: `set_initiative_endpoint_updates_combatant_initiative` (was pre-existing-failing on master, now passing after refactor)

### Combat-specific gaps remaining (Sprint 9 audit)
- 0 frontend combat component tests (24 Svelte files in `web/src/lib/combat/` + `initiative/+page.svelte`)
- 10 untested mechanics ranked high-risk: Rage end, Smite, Condition timer (`name:N` tick), Hidden reveal on attack, Grapple release on incapacitate, Regen modifier at turn start, Ritual casting, Spell range E2E, Fighting style Defense, Condition immunity by creature type
- C6 (kept-vs-unkept natural_roll) is review-grade fix; not covered by unit test (would need refactor of `resolve_*` to inject `Rng`)

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
cd backend && cargo test  # 489 tests (post-Sprint 9, was 437 in pre-Sprint 7 + 28 Sprint 1 + 7 Sprint 2 + 4 Sprint 3 + 3 Sprint 4 + 4 Sprint 9)

# Frontend unit
cd web && bunx vitest run        # 630 tests (20 files)

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
