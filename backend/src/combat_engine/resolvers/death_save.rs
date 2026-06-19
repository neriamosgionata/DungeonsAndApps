use super::super::types::CombatantSnapshot;
use super::types::{DeathSaveReq, DeathSaveResult};
use crate::dice::roll;
use rand::{SeedableRng, rngs::StdRng};

pub fn resolve_death_save(
    snap: &CombatantSnapshot,
    req: &DeathSaveReq,
) -> Result<DeathSaveResult, String> {
    let mut rng = StdRng::from_os_rng();

    let adv = req.advantage;
    let dis = req.disadvantage;
    let effective_adv = adv && !dis;
    let effective_dis = dis && !adv;

    let expr = if effective_adv {
        "2d20kh1".to_string()
    } else if effective_dis {
        "2d20kl1".to_string()
    } else {
        "1d20".to_string()
    };

    let roll_res = roll(&expr, &mut rng).map_err(|e| format!("death save roll error: {}", e))?;

    // "Natural roll" = the d20 face used for the check. For 1d20 it's the die.
    // For 2d20kh1 (advantage) / 2d20kl1 (disadvantage) it's the kept die.
    // Use the unkept first die as a fallback for completeness.
    let natural = roll_res
        .terms
        .first()
        .and_then(|t| t.kept.first().copied().or_else(|| t.rolls.first().copied()))
        .unwrap_or(0);

    let nat20 = natural == 20;
    let nat1 = natural == 1;

    // Read current death saves from sheet
    let ds = snap.sheet_raw.get("death_saves");
    let successes_before = ds
        .and_then(|d| d.get("successes"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
        .clamp(0, 3) as i32;
    let failures_before = ds
        .and_then(|d| d.get("failures"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
        .clamp(0, 3) as i32;

    let mut successes_after = successes_before;
    let mut failures_after = failures_before;
    let mut hp_after = snap.hp_current;
    let mut alive = true;
    let mut stabilized = false;
    let mut died = false;

    if nat20 {
        // Regain 1 HP, stable
        hp_after = 1;
        alive = true;
        stabilized = true;
        successes_after = 0;
        failures_after = 0;
    } else if nat1 {
        failures_after = (failures_before + 2).min(3);
        if failures_after >= 3 {
            died = true;
            alive = false;
        }
    } else if natural >= 10 {
        successes_after = (successes_before + 1).min(3);
        if successes_after >= 3 {
            stabilized = true;
            alive = true;
            successes_after = 0;
            failures_after = 0;
        }
    } else {
        failures_after = (failures_before + 1).min(3);
        if failures_after >= 3 {
            died = true;
            alive = false;
        }
    }

    Ok(DeathSaveResult {
        natural_roll: natural,
        passed: natural >= 10 && !nat1,
        successes_before,
        failures_before,
        successes_after,
        failures_after,
        stabilized,
        died,
        nat20,
        nat1,
        hp_after,
        alive,
    })
}
