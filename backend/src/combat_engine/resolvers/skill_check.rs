use super::super::stats::{ability_mod, compute_stats, proficiency_from_level};
use super::super::types::{CombatantSnapshot, ComputedStats};
use super::types::{SkillCheckReq, SkillCheckResult};
use crate::dice::{RollResult, roll};
use rand::{Rng, SeedableRng, rngs::StdRng};

pub fn resolve_skill_check(
    snap: &CombatantSnapshot,
    req: &SkillCheckReq,
    stats: &ComputedStats,
) -> Result<SkillCheckResult, String> {
    let mut rng = StdRng::from_os_rng();
    let skill = req.skill.to_lowercase().replace(' ', "_");

    let pb = if snap.proficiency_bonus > 0 {
        snap.proficiency_bonus
    } else {
        proficiency_from_level(snap.level_total)
    };

    let skill_prof_for_jack = snap.skills.get(&skill)
        .or_else(|| snap.skills.get(&skill.replace('_', " ")))
        .and_then(|v| v.as_str());
    let is_proficient_for_jack = matches!(skill_prof_for_jack, Some("prof") | Some("proficient") | Some("expert"));

    let modv = stats.skill_mods.iter()
        .find(|(s, _)| s == &skill)
        .map(|(_, m)| if !is_proficient_for_jack && stats.jack_of_all_trades {
            *m + (pb / 2)
        } else {
            *m
        })
        .unwrap_or_else(|| {
            // fallback: try ability mod based on skill name
            let ability = skill_ability(&skill);
            let base = ability_mod(snap, ability);
            if stats.jack_of_all_trades { base + pb / 2 } else { base }
        });

    let adv = req.advantage;
    let dis = req.disadvantage;
    let effective_adv = adv && !dis;
    let effective_dis = dis && !adv;

    let expr = if effective_adv {
        format!("2d20kh1+{}", modv)
    } else if effective_dis {
        format!("2d20kl1+{}", modv)
    } else {
        format!("1d20+{}", modv)
    };

    let roll_res = roll(&expr, &mut rng)
        .map_err(|e| format!("skill check roll error: {}", e))?;

    let natural = roll_res.terms.first()
        .and_then(|t| t.rolls.first().copied())
        .unwrap_or(0);

    // Reliable Talent (Rogue 11+): treat any d20 ≤9 as 10 for proficient/expert skills
    let has_reliable_talent = snap.classes.as_array().map(|arr| {
        arr.iter().any(|c| {
            let name = c.get("name").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
            let level = c.get("level").and_then(|v| v.as_i64()).map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32).unwrap_or(0);
            name == "rogue" && level >= 11
        })
    }).unwrap_or(false);
    let skill_prof = snap.skills.get(&skill)
        .or_else(|| snap.skills.get(&skill.replace('_', " ")))
        .and_then(|v| v.as_str());
    let is_proficient = matches!(skill_prof, Some("prof") | Some("proficient") | Some("expert"));
    let total = if has_reliable_talent && is_proficient && natural < 10 {
        roll_res.total - natural + 10
    } else {
        roll_res.total
    };

    let passed = req.dc.map(|dc| total >= dc);

    Ok(SkillCheckResult {
        skill: req.skill.clone(),
        natural_roll: natural,
        total,
        dc: req.dc,
        passed,
        advantage: effective_adv,
        disadvantage: effective_dis,
    })
}

fn skill_ability(skill: &str) -> &str {
    match skill {
        "athletics" => "str",
        "acrobatics" | "sleight_of_hand" | "stealth" => "dex",
        "arcana" | "history" | "investigation" | "nature" | "religion" => "int",
        "animal_handling" | "insight" | "medicine" | "perception" | "survival" => "wis",
        "deception" | "intimidation" | "performance" | "persuasion" => "cha",
        _ => "wis",
    }
}

