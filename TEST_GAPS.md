# Test Coverage Gap Analysis

## Current State

### Backend (Rust)
- **Source:** ~10,800 lines
- **Tests:** ~7,200 lines (67% ratio)
- **Test Count:** 586 tests across 26 test files
- **Status:** Good coverage on combat, auth, API endpoints. 30 new tests added 2026-06-22 (5 HIGH-no-test regressions + 6 HIGH-already-fixed regressions + 12 mechanics coverage + 3 MED regressions M6/M11/M12 + 4 LOW/INFO regressions L18/L15/L11/I4).

### Frontend (TypeScript/Svelte)
- **TS Source:** ~2,900 lines → **Tests:** ~2,800 lines (97% ratio) ✅
- **Svelte Source:** ~19,700 lines → **Tests:** 0 lines (0% ratio) ❌
- **Test Count:** 630 tests across 20 files (all TS utilities + pure function modules)
- **Status:** Business logic covered, UI untested

---

## HIGH bugs uncovered by 2026-06-22 combat audit — **ALL CLOSED 2026-06-22**

The 2026-06-22 deep-dive found 5 HIGH bugs in production code that the test suite did NOT exercise. **All 5 are now fixed in code AND have regression tests** in `backend/tests/combat_coverage_jun2026.rs`. See `COMBAT_AUDIT.md` §"Previously HIGH — Now Fixed" for closure log.

| ID | Bug | Status | Regression Test |
|----|-----|--------|-----------------|
| HIGH-16 | Multiattack target reorder index swap | **FIXED 2026-06-22** | `high16_multiattack_damage_lands_on_correct_target_id` |
| HIGH-17 | Within-5ft threshold (5% instead of 20%) | **FIXED 2026-06-22** | `high17_auto_crit_at_4ft_from_paralyzed_target` |
| HIGH-18 | `cover="full"` dead branch (0 AC instead of block) | **FIXED 2026-06-22** | `high18_total_cover_blocks_attack` |
| HIGH-19 | Spell range formula `g_size * dist_pct` (gave 0ft) | **FIXED 2026-06-22** | `high19_spell_range_filters_by_distance` |
| HIGH-20 | `apply_hp_damage` no HP clamp (0 → negative) | **FIXED 2026-06-22** | `high20_hp_clamps_at_zero_on_overkill` |

---

## Mechanics coverage tests added 2026-06-22

12 new tests in `backend/tests/combat_coverage_jun2026.rs` cover gaps identified in the 41-mechanism D&D audit:

| # | Mechanic | Test | Status |
|---|----------|------|--------|
| 1 | GWF damage reroll 1-2 | `mech_gwf_reroll_low_dice_takes_better` | Covered (avg damage > 11) |
| 2 | Sneak Attack extra-damage engine contract | `mech_sneak_attack_extra_damage_applied_per_attack_engine_level` | Documents handler-level gate |
| 3 | Spell prep: Cleric/Druid/Paladin/Artificer | `mech_spell_prep_required_for_divine_casters` | Source-level assertion |
| 4 | Spell prep skip: Sorc/Bard/Warlock/Ranger/Rogue | `mech_known_casters_skip_prep_check` | Source-level assertion |
| 5 | Spell range enforcement | `mech_spell_range_silent_drop_out_of_range_target` | Formula verified (silent-drop contract) |
| 6 | Prone ranged disadvantage | `mech_prone_ranged_disadvantage_uses_2d20kl1` | Engine contract |
| 7 | Prone target melee advantage | `mech_prone_target_melee_advantage_via_attack_advantage_against` | Engine contract |
| 8 | Surprised action economy enforcement | `mech_surprised_action_economy_enforced_at_turn_start` | Integration |
| 9 | Temp HP "only if higher" PATCH | `mech_temp_hp_patch_keeps_higher_value` | Integration |
| 10 | Grapple release chain on grappler incapacitated | `mech_grapple_release_chain_on_grappler_incapacitated` | Integration |
| 11 | Ready action trigger (`trigger_ready`) | `mech_trigger_ready_consumes_reaction_and_clears_readied` | Integration |
| 12 | Rage effects (damage_bonus + BPS resistance + attack_advantage) | `mech_rage_effect_writes_all_three_modifiers` | Unit (engine contract) |

---

## HIGH regression tests added 2026-06-22

11 new tests in `backend/tests/combat_coverage_jun2026.rs` guard the 12 HIGH bugs from the 2026-06-22 combat audit:

| ID | Bug | Test | Status |
|----|-----|------|--------|
| H1 | Multiattack target reorder index swap | `high16_multiattack_damage_lands_on_correct_target_id` | Integration |
| H2 | Within-5ft threshold (5% → 20%) | `high17_auto_crit_at_4ft_from_paralyzed_target` | Unit |
| H3 | `cover="full"` dead branch | `high18_total_cover_blocks_attack` | Unit |
| H4 | Spell range formula | `high19_spell_range_filters_by_distance` | Unit |
| H5 | HP clamp at 0 | `high20_hp_clamps_at_zero_on_overkill` | Unit |
| H6 | TWF main-hand `light` check | `high6_twf_requires_main_hand_light_property` | Source-level |
| H7+H10 | `set_initiative` tx + ROW_NUMBER | `high7_set_initiative_assigns_contiguous_turn_order` | Integration |
| H8 | Delete turn_order renumber | `high8_delete_renumbers_turn_order_contiguously` | Integration |
| H9 | conditions.rs events after commit | `high9_conditions_events_published_after_commit` | Source-level |
| H11 | delay `SELECT FOR UPDATE` | `high11_delay_locks_encounter_with_for_update` | Integration |
| H12 | bulk_add tx + savepoints | `high12_bulk_add_uses_tx_with_savepoints` | Integration |

---

## Coverage gaps identified by 2026-06-22 re-audit

| # | Gap | Location | Effort |
|---|-----|----------|--------|
| 1 | `grapple_escape` handler — 0 test refs | `special/escape.rs:24` | Add integration test (contested roll + action consume + condition remove + WS emit) |
| 2 | `delete_event` handler — 0 test refs | `events.rs:71` | Add unit test for master-only DELETE of combat_events row |
| 3 | `try_parse_npc_multiattack` — 0 test refs | `special/parse_multiattack.rs:172` | Add unit test for "2 claws + 1 bite" parsing |
| 4 | L15 (frightened LOS) — no test | `attack.rs:91-93` | Deferred until source-of-fear tracking refactor |
| 5 | L18 (OA reach mismatch) — no integration test | `opportunity.rs:103-109` vs `web/.../initiative/+page.svelte:1511` | Add integration test pinning backend/frontend consistency |
| 6 | L11 (start.rs stale flags) — no regression test | `start.rs:97-104` | Add test: start encounter, check all combatants' `action_used` reset |

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
|-------|-------|
| `auth.svelte.ts` | ✅ Tested (3 tests in `resources.test.ts` via Auth.updateMe / Auth.changePassword) — also: `safeStorage()` guard added 2026-06-23 so module load works under jsdom-opaque-origin |
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
cd backend && cargo test  # 586 tests (post 2026-06-22 full re-audit + MED + LOW/INFO fixes, was 437 in pre-Sprint 7 + 28 Sprint 1 + 7 Sprint 2 + 4 Sprint 3 + 3 Sprint 4 + 4 Sprint 9 + 5 HIGH-no-test + 6 HIGH-already-fixed + 12 mechanics + 3 MED + 4 LOW/INFO = 437 + 63 = 500, then audit fixes added 86 more = 586)

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
