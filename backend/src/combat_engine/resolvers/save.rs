use super::super::stats::ability_mod;
use super::super::types::{CombatantSnapshot, ComputedStats};
use super::types::{SaveReq, SaveResult};
use crate::dice::roll;
use rand::{SeedableRng, rngs::StdRng};

pub fn resolve_save(
    snap: &CombatantSnapshot,
    req: &SaveReq,
    stats: &ComputedStats,
) -> Result<SaveResult, String> {
    let mut rng = StdRng::from_os_rng();
    let ability = req.ability.to_lowercase();

    let mut adv = req.advantage || stats.save_advantage;
    // L14: dis applies if the global flag is set OR this specific ability
    // is in the ability-specific disadvantage set (e.g. restrained → dex).
    let ability_dis = stats
        .save_disadvantage_abilities
        .contains(&ability);
    let dis = req.disadvantage || stats.save_disadvantage || ability_dis;
    // Gnome Cunning: advantage on INT/WIS/CHA saves vs magic
    if stats.gnome_cunning
        && req.is_magical.unwrap_or(false)
        && matches!(ability.as_str(), "int" | "wis" | "cha")
    {
        adv = true;
    }
    // Magic Resistance: advantage on saves vs spells/magical effects (Yuan-Ti, Satyr)
    if snap
        .sheet_raw
        .get("magic_resistance")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
        && req.is_magical.unwrap_or(false)
    {
        adv = true;
    }
    // MED-2: PHB — paralyzed, petrified, stunned, AND unconscious creatures
    // automatically fail STR and DEX saves. Pre-fix only checked
    // paralyzed/petrified.
    if (stats.paralyzed || stats.petrified || stats.stunned || stats.unconscious)
        && (ability == "str" || ability == "dex")
    {
        let save_roll = roll("1d20", &mut rng).unwrap_or_else(|e| {
            tracing::error!("auto-fail 1d20 roll failed: {e}; using 0");
            crate::dice::RollResult {
                expression: "1d20".into(),
                terms: vec![],
                total: 0,
            }
        });
        return Ok(SaveResult {
            passed: false,
            natural_roll: 1,
            save_total: 1,
            dc: req.dc,
            save_roll,
            save_advantage: false,
            save_disadvantage: true,
        });
    }

    let effective_adv = adv && !dis;
    let effective_dis = dis && !adv;

    let save_mod = stats
        .save_mods
        .iter()
        .find(|(a, _)| a == &ability)
        .map(|(_, m)| *m)
        .unwrap_or(ability_mod(snap, &ability));

    let expr = if effective_adv {
        format!("2d20kh1+{}", save_mod)
    } else if effective_dis {
        format!("2d20kl1+{}", save_mod)
    } else {
        format!("1d20+{}", save_mod)
    };

    let roll_res = roll(&expr, &mut rng).map_err(|e| format!("save roll error: {}", e))?;

    let natural = roll_res
        .terms
        .first()
        .and_then(|t| t.kept.first().copied().or_else(|| t.rolls.first().copied()))
        .unwrap_or(0);

    let passed = roll_res.total >= req.dc;

    Ok(SaveResult {
        passed,
        natural_roll: natural,
        save_total: roll_res.total,
        dc: req.dc,
        save_roll: roll_res,
        save_advantage: effective_adv,
        save_disadvantage: effective_dis,
    })
}
