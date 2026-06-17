use super::super::types::CombatantSnapshot;
use super::damage_type::{apply_damage_type, apply_hp_damage, concentration_check};
use super::types::{DamageReq, DamageResult};
use rand::SeedableRng;
use rand::rngs::StdRng;

pub fn resolve_damage(
    target: &CombatantSnapshot,
    req: &DamageReq,
    target_stats: &super::super::types::ComputedStats,
) -> Result<DamageResult, String> {
    let mut rng = StdRng::from_os_rng();
    let dtype = req.damage_type.to_lowercase();
    let (effective_dmg, damage_resisted, damage_vulnerable, damage_immune) =
        apply_damage_type(req.amount, &dtype, target_stats, req.is_magical);

    let (new_hp, new_temp) = apply_hp_damage(target.hp_current, target.temp_hp, effective_dmg);

    let mut concentration_broken = false;
    let mut concentration_roll = None;
    if target.active_effects.iter().any(|e| e.concentration) {
        let (broken, roll_res) = concentration_check(target, effective_dmg, &mut rng);
        concentration_broken = broken;
        concentration_roll = Some(roll_res);
    }

    Ok(DamageResult {
        damage_raw: req.amount,
        damage_applied: effective_dmg,
        hp_before: target.hp_current,
        hp_after: new_hp,
        temp_hp_after: new_temp,
        concentration_broken,
        concentration_roll,
        combat_event_id: None,
        damage_resisted,
        damage_vulnerable,
        damage_immune,
        instant_death: target.hp_current > 0
            && (effective_dmg - target.hp_current - target.temp_hp).max(0) >= target.hp_max,
    })
}
