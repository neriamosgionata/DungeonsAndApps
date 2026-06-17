// Encounter difficulty calculator.
use super::*;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct DifficultyResult {
    pub total_xp: i32,
    pub adjusted_xp: i32,
    pub difficulty: String,
    pub thresholds: DifficultyThresholds,
    pub party_levels: Vec<i32>,
    pub monster_xp: Vec<(String, i32, i32)>,
}

#[derive(Debug, Serialize)]
pub struct DifficultyThresholds {
    pub easy: i32,
    pub medium: i32,
    pub hard: i32,
    pub deadly: i32,
}

pub async fn encounter_difficulty(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
) -> AppResult<Json<DifficultyResult>> {
    let campaign_id: Uuid = sqlx::query_scalar("select campaign_id from encounters where id = $1")
        .bind(encounter_id)
        .fetch_one(&s.db)
        .await?;
    rbac::require_member(&s.db, uid, campaign_id).await?;

    let party_levels: Vec<i32> = sqlx::query_scalar(
        r#"select ch.level_total
           from characters ch
           where ch.campaign_id = $1
             and coalesce((ch.sheet->>'alive')::boolean, true) = true"#,
    )
    .bind(campaign_id)
    .fetch_all(&s.db)
    .await?;

    let mut easy = 0i32;
    let mut medium = 0i32;
    let mut hard = 0i32;
    let mut deadly = 0i32;

    for level in &party_levels {
        let (e, m, h, d) = xp_thresholds(*level);
        easy += e;
        medium += m;
        hard += h;
        deadly += d;
    }

    let combatants: Vec<(String, Option<serde_json::Value>)> = sqlx::query_as(
        r#"select c.display_name, n.stats as npc_stats
           from combatants c
           left join npcs n on n.id = c.npc_id
           where c.encounter_id = $1"#,
    )
    .bind(encounter_id)
    .fetch_all(&s.db)
    .await?;

    let mut total_xp = 0i32;
    let mut monster_entries = Vec::new();

    for (name, stats) in &combatants {
        let xp = if let Some(s) = stats {
            s.get("xp")
                .and_then(|v| v.as_i64())
                .map(|v| v as i32)
                .or_else(|| s.get("cr_xp").and_then(|v| v.as_i64()).map(|v| v as i32))
                .or_else(|| s.get("cr").and_then(|v| v.as_str()).and_then(cr_to_xp))
                .unwrap_or(0)
        } else {
            0
        };
        total_xp += xp;
        if xp > 0 {
            monster_entries.push((name.clone(), xp, 1));
        }
    }

    let multiplier = if party_levels.is_empty() {
        1.0
    } else {
        let n_monsters = combatants.len().max(1);
        let n_party = party_levels.len();
        encounter_multiplier(n_monsters, n_party)
    };

    let adjusted_xp = (total_xp as f32 * multiplier).ceil() as i32;

    let difficulty = if adjusted_xp >= deadly {
        "deadly"
    } else if adjusted_xp >= hard {
        "hard"
    } else if adjusted_xp >= medium {
        "medium"
    } else {
        "easy"
    };

    Ok(Json(DifficultyResult {
        total_xp,
        adjusted_xp,
        difficulty: difficulty.to_string(),
        thresholds: DifficultyThresholds {
            easy,
            medium,
            hard,
            deadly,
        },
        party_levels,
        monster_xp: monster_entries,
    }))
}

fn xp_thresholds(level: i32) -> (i32, i32, i32, i32) {
    match level {
        1 => (25, 50, 75, 100),
        2 => (50, 100, 150, 200),
        3 => (75, 150, 225, 400),
        4 => (125, 250, 375, 500),
        5 => (250, 500, 750, 1100),
        6 => (300, 600, 900, 1400),
        7 => (350, 750, 1100, 1700),
        8 => (450, 900, 1400, 2100),
        9 => (550, 1100, 1600, 2400),
        10 => (600, 1200, 1900, 2800),
        11 => (800, 1600, 2400, 3600),
        12 => (1000, 2000, 3000, 4500),
        13 => (1100, 2200, 3400, 5100),
        14 => (1250, 2500, 3800, 5700),
        15 => (1400, 2800, 4300, 6400),
        16 => (1600, 3200, 4800, 7200),
        17 => (2000, 3900, 5900, 8800),
        18 => (2100, 4200, 6300, 9500),
        19 => (2400, 4900, 7300, 10900),
        20 => (2800, 5700, 8500, 12700),
        _ => (25, 50, 75, 100),
    }
}

fn cr_to_xp(cr: &str) -> Option<i32> {
    let c = cr.trim().to_lowercase();
    match c.as_str() {
        "0" | "1/8" => Some(10),
        "1/4" => Some(50),
        "1/2" => Some(100),
        "1" => Some(200),
        "2" => Some(450),
        "3" => Some(700),
        "4" => Some(1100),
        "5" => Some(1800),
        "6" => Some(2300),
        "7" => Some(2900),
        "8" => Some(3900),
        "9" => Some(5000),
        "10" => Some(5900),
        "11" => Some(7200),
        "12" => Some(8400),
        "13" => Some(10000),
        "14" => Some(11500),
        "15" => Some(13000),
        "16" => Some(15000),
        "17" => Some(18000),
        "18" => Some(20000),
        "19" => Some(22000),
        "20" => Some(25000),
        "21" => Some(33000),
        "22" => Some(41000),
        "23" => Some(50000),
        "24" => Some(62000),
        "25" => Some(75000),
        "26" => Some(90000),
        "27" => Some(105000),
        "28" => Some(120000),
        "29" => Some(135000),
        "30" => Some(155000),
        _ => None,
    }
}

fn encounter_multiplier(n_monsters: usize, n_party: usize) -> f32 {
    let base = match n_monsters {
        1 => 0.5,
        2 => 1.0,
        3..=6 => 1.5,
        7..=10 => 2.0,
        11..=14 => 2.5,
        _ => 3.0,
    };
    if n_party <= 2 {
        base + 0.5
    } else if n_party >= 6 {
        (base - 0.5).max(0.5)
    } else {
        base
    }
}
