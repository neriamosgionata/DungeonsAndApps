use super::super::types::CombatantSnapshot;
use super::types::{HealReq, HealResult};

pub fn resolve_heal(target: &CombatantSnapshot, req: &HealReq) -> HealResult {
    // L-2: honor exhaustion L4 (HP max halved, PHB p.291) even without an
    // explicit effective max — the raw-default version silently healed
    // exhausted characters above their capped max.
    let exhaustion = target
        .sheet_raw
        .get("exhaustion")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let effective_max = if exhaustion >= 4 {
        target.hp_max / 2
    } else {
        target.hp_max
    };
    resolve_heal_with_max(target, req, effective_max)
}

/// Sprint 38: allow callers to pass a pre-computed effective hp_max so
/// exhaustion L4 (HP max halved) is honored. Default [`resolve_heal`]
/// uses the snapshot's raw hp_max.
pub fn resolve_heal_with_max(
    target: &CombatantSnapshot,
    req: &HealReq,
    effective_hp_max: i32,
) -> HealResult {
    let hp_before = target.hp_current;
    // MED-7 (2nd pass): never REDUCE HP — a heal on someone already above
    // the effective max (mid-combat hp_max_reduction) must clamp, not drop.
    let hp_after = target
        .hp_current
        .max((target.hp_current + req.amount).min(effective_hp_max));
    let stabilized = target.hp_current <= 0 && hp_after > 0;
    let revived = stabilized;
    HealResult {
        amount: req.amount,
        hp_before,
        hp_after,
        temp_hp_after: target.temp_hp,
        stabilized,
        revived,
    }
}
