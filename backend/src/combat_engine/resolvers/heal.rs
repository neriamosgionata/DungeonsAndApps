use super::super::types::CombatantSnapshot;
use super::types::{HealReq, HealResult};

pub fn resolve_heal(target: &CombatantSnapshot, req: &HealReq) -> HealResult {
    resolve_heal_with_max(target, req, target.hp_max)
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
    let hp_after = (target.hp_current + req.amount).min(effective_hp_max);
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
