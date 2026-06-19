use super::super::types::CombatantSnapshot;
use super::types::{HealReq, HealResult};

pub fn resolve_heal(target: &CombatantSnapshot, req: &HealReq) -> HealResult {
    let hp_before = target.hp_current;
    let hp_after = (target.hp_current + req.amount).min(target.hp_max);
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
