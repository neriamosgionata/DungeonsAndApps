use super::*;
use super::{
    fetch, has_condition, remove_condition,
    sync_combatant_hp_to_sheet,
    Combatant, Encounter,
};

use crate::{
    combat_engine,
    error::{AppError, AppResult},
    extract::AuthUser,
    rbac::{self, Role},
    ws,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;
use validator::Validate;

// =====================================================================
// Structs: Overlay + OverlayCreate
// =====================================================================

#[derive(Debug, Serialize, FromRow)]
pub struct Overlay {
    pub id: Uuid,
    pub encounter_id: Uuid,
    pub kind: String,
    pub shape: String,
    pub origin_x: f64,
    pub origin_y: f64,
    pub end_x: Option<f64>,
    pub end_y: Option<f64>,
    pub radius_ft: Option<i32>,
    pub length_ft: Option<i32>,
    pub width_ft: Option<i32>,
    pub angle_deg: Option<f64>,
    pub points: Option<serde_json::Value>,
    pub color: String,
    pub label: Option<String>,
    pub zone_type: Option<String>,
    pub active: bool,
    pub expires_at_round: Option<i32>,
    pub expires_at_turn: Option<i32>,
    pub source_spell_slug: Option<String>,
    pub created_by_combatant_id: Option<Uuid>,
    pub created_at: OffsetDateTime,
    pub hazard_damage_expression: Option<String>,
    pub hazard_damage_type: Option<String>,
    pub hazard_save_ability: Option<String>,
    pub hazard_save_dc: Option<i32>,
    pub hazard_half_on_save: bool,
}

#[derive(Debug, Deserialize, Validate)]
pub struct OverlayCreate {
    pub kind: String,
    pub shape: String,
    pub origin_x: f64,
    pub origin_y: f64,
    pub end_x: Option<f64>,
    pub end_y: Option<f64>,
    pub radius_ft: Option<i32>,
    pub length_ft: Option<i32>,
    pub width_ft: Option<i32>,
    pub angle_deg: Option<f64>,
    pub points: Option<serde_json::Value>,
    pub color: Option<String>,
    pub label: Option<String>,
    pub zone_type: Option<String>,
    pub expires_at_round: Option<i32>,
    pub expires_at_turn: Option<i32>,
    pub source_spell_slug: Option<String>,
    pub created_by_combatant_id: Option<Uuid>,
    pub hazard_damage_expression: Option<String>,
    pub hazard_damage_type: Option<String>,
    pub hazard_save_ability: Option<String>,
    pub hazard_save_dc: Option<i32>,
    pub hazard_half_on_save: Option<bool>,
}

// =====================================================================
// Overlay endpoints
// =====================================================================

pub async fn list_overlays(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
) -> AppResult<Json<Vec<Overlay>>> {
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(encounter_id).fetch_one(&s.db).await?;
    rbac::require_member(&s.db, uid, campaign_id).await?;

    let rows: Vec<Overlay> = sqlx::query_as::<_, Overlay>(
        r#"select id, encounter_id, kind, shape, origin_x, origin_y, end_x, end_y,
                  radius_ft, length_ft, width_ft, angle_deg, points, color, label, zone_type,
                  active, expires_at_round, expires_at_turn, source_spell_slug, created_by_combatant_id, created_at,
                  hazard_damage_expression, hazard_damage_type, hazard_save_ability, hazard_save_dc, coalesce(hazard_half_on_save, false) as hazard_half_on_save
           from encounter_overlays
           where encounter_id = $1 and active = true
           order by created_at desc"#,
    )
    .bind(encounter_id)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

pub async fn create_overlay(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
    Json(body): Json<OverlayCreate>,
) -> AppResult<(StatusCode, Json<Overlay>)> {
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(encounter_id).fetch_one(&s.db).await?;
    rbac::require_master(&s.db, uid, campaign_id).await?;

    let o: Overlay = sqlx::query_as::<_, Overlay>(
        r#"insert into encounter_overlays
           (encounter_id, kind, shape, origin_x, origin_y, end_x, end_y, radius_ft, length_ft, width_ft, angle_deg, points,
            color, label, zone_type, expires_at_round, expires_at_turn, source_spell_slug, created_by_combatant_id,
            hazard_damage_expression, hazard_damage_type, hazard_save_ability, hazard_save_dc, hazard_half_on_save)
           values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24)
           returning id, encounter_id, kind, shape, origin_x, origin_y, end_x, end_y,
                     radius_ft, length_ft, width_ft, angle_deg, points, color, label, zone_type,
                     active, expires_at_round, expires_at_turn, source_spell_slug, created_by_combatant_id, created_at,
                     hazard_damage_expression, hazard_damage_type, hazard_save_ability, hazard_save_dc, coalesce(hazard_half_on_save, false) as hazard_half_on_save"#,
    )
    .bind(encounter_id)
    .bind(&body.kind)
    .bind(&body.shape)
    .bind(body.origin_x)
    .bind(body.origin_y)
    .bind(body.end_x)
    .bind(body.end_y)
    .bind(body.radius_ft)
    .bind(body.length_ft)
    .bind(body.width_ft)
    .bind(body.angle_deg)
    .bind(body.points)
    .bind(body.color.as_deref().unwrap_or("rgba(255,0,0,0.25)"))
    .bind(body.label.as_deref())
    .bind(body.zone_type.as_deref())
    .bind(body.expires_at_round)
    .bind(body.expires_at_turn)
    .bind(body.source_spell_slug.as_deref())
    .bind(body.created_by_combatant_id)
    .bind(body.hazard_damage_expression.as_deref())
    .bind(body.hazard_damage_type.as_deref())
    .bind(body.hazard_save_ability.as_deref())
    .bind(body.hazard_save_dc)
    .bind(body.hazard_half_on_save.unwrap_or(false))
    .fetch_one(&s.db)
    .await?;

    ws::publish(campaign_id, json!({"type":"overlay_added","encounter_id":encounter_id,"id":o.id}).to_string());
    Ok((StatusCode::CREATED, Json(o)))
}

pub async fn delete_overlay(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path((encounter_id, overlay_id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(encounter_id).fetch_one(&s.db).await?;
    rbac::require_master(&s.db, uid, campaign_id).await?;

    sqlx::query("update encounter_overlays set active = false where id = $1 and encounter_id = $2")
        .bind(overlay_id).bind(encounter_id).execute(&s.db).await?;

    ws::publish(campaign_id, json!({"type":"overlay_removed","encounter_id":encounter_id,"id":overlay_id}).to_string());
    Ok(StatusCode::NO_CONTENT)
}

// =====================================================================
// Condition Immunity
// =====================================================================

/// Returns true if this combatant is immune to the given condition (lowercase name).
pub async fn check_condition_immunity(db: &sqlx::PgPool, combatant_id: Uuid, condition: &str) -> Result<bool, crate::error::AppError> {
    let row: Option<(Option<serde_json::Value>, Option<serde_json::Value>)> = sqlx::query_as(
        r#"select n.stats, ch.sheet
           from combatants c
           left join npcs n on n.id = c.npc_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#)
        .bind(combatant_id).fetch_optional(db).await?;

    let (npc_stats_raw, char_sheet) = row.unwrap_or((None, None));

    if let Some(ref raw) = npc_stats_raw {
        if let Some(npc) = combat_engine::NpcStats::from_value(raw) {
            if npc.condition_immunities.iter().any(|c| c.to_lowercase() == condition) {
                return Ok(true);
            }
            let creature_type = raw.get("creature_type").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
            if is_immune_by_type(&creature_type, condition) {
                return Ok(true);
            }
        }
    }

    if let Some(ref sheet) = char_sheet {
        if let Some(arr) = sheet.get("condition_immunities").and_then(|v| v.as_array()) {
            if arr.iter().any(|c| c.as_str().map(|s| s.to_lowercase() == condition).unwrap_or(false)) {
                return Ok(true);
            }
        }
        let creature_type = sheet.get("creature_type").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
        if !creature_type.is_empty() && is_immune_by_type(&creature_type, condition) {
            return Ok(true);
        }
    }

    Ok(false)
}

fn is_immune_by_type(creature_type: &str, condition: &str) -> bool {
    match condition {
        "poisoned" | "exhaustion" | "frightened" | "charmed" =>
            matches!(creature_type, "undead" | "construct" | "plant"),
        "paralyzed" | "petrified" =>
            creature_type == "construct",
        "blinded" | "deafened" =>
            creature_type == "plant",
        _ => false,
    }
}

// =====================================================================
// Encounter Difficulty Calculator
// =====================================================================

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
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(encounter_id).fetch_one(&s.db).await?;
    rbac::require_member(&s.db, uid, campaign_id).await?;

    let party_levels: Vec<i32> = sqlx::query_scalar(
        r#"select ch.level_total
           from characters ch
           where ch.campaign_id = $1
             and coalesce((ch.sheet->>'alive')::boolean, true) = true"#,
    )
    .bind(campaign_id)
    .fetch_all(&s.db).await?;

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
    .fetch_all(&s.db).await?;

    let mut total_xp = 0i32;
    let mut monster_entries = Vec::new();

    for (name, stats) in &combatants {
        let xp = if let Some(s) = stats {
            s.get("xp").and_then(|v| v.as_i64()).map(|v| v as i32)
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
        thresholds: DifficultyThresholds { easy, medium, hard, deadly },
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

// =====================================================================
// AoE Overlay Auto-Damage
// =====================================================================

#[derive(Debug, Deserialize)]
pub struct OverlayDamageBody {
    pub overlay_id: Uuid,
    pub damage_expression: String,
    pub damage_type: String,
    pub save_ability: Option<String>,
    pub save_dc: Option<i32>,
    pub half_on_save: bool,
    pub is_magical: bool,
    pub label: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OverlayDamageResult {
    pub overlay_id: Uuid,
    pub targets_affected: Vec<OverlayTargetResult>,
}

#[derive(Debug, Serialize)]
pub struct OverlayTargetResult {
    pub target_id: Uuid,
    pub target_name: String,
    pub in_area: bool,
    pub save_passed: Option<bool>,
    pub damage_applied: i32,
    pub hp_after: i32,
}

pub async fn overlay_damage(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
    Json(body): Json<OverlayDamageBody>,
) -> AppResult<Json<OverlayDamageResult>> {
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(encounter_id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master {
        return Err(AppError::Forbidden);
    }

    let overlay: (String, Option<f64>, Option<f64>, Option<i32>, Option<i32>, Option<i32>) = sqlx::query_as(
        "select shape, origin_x, origin_y, radius_ft, length_ft, width_ft from encounter_overlays where id = $1 and encounter_id = $2 and active = true")
        .bind(body.overlay_id).bind(encounter_id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let (shape, ox, oy, radius, _length, _width) = overlay;
    let origin = (ox.unwrap_or(50.0), oy.unwrap_or(50.0));

    let combatants: Vec<(Uuid, String, Option<f64>, Option<f64>)> = sqlx::query_as(
        "select id, display_name, token_x, token_y from combatants where encounter_id = $1")
        .bind(encounter_id).fetch_all(&s.db).await?;

    let mut rng = rand::rngs::StdRng::from_os_rng();
    let mut targets_affected = Vec::new();

    for (cid, name, tx, ty) in &combatants {
        let in_area = if let (Some(x), Some(y)) = (tx, ty) {
            match shape.as_str() {
                "circle" => {
                    let r = radius.unwrap_or(20) as f64;
                    let dx = *x - origin.0;
                    let dy = *y - origin.1;
                    (dx*dx + dy*dy).sqrt() <= r
                }
                "cube" | "square" => {
                    let r = radius.unwrap_or(20) as f64;
                    let dx = (*x - origin.0).abs();
                    let dy = (*y - origin.1).abs();
                    dx <= r && dy <= r
                }
                _ => {
                    let dx = *x - origin.0;
                    let dy = *y - origin.1;
                    let r = radius.unwrap_or(20) as f64;
                    (dx*dx + dy*dy).sqrt() <= r
                }
            }
        } else { false };

        if !in_area { continue; }

        let snap = match combat_engine::load_snapshot(&s.db, *cid).await {
            Ok(s) => s,
            Err(_) => continue,
        };
        let stats = combat_engine::compute_stats(&snap);

        let mut save_passed = None;

        if let Some(ref ability) = body.save_ability {
            let dc = body.save_dc.unwrap_or(15);
            let save_req = combat_engine::SaveReq {
                ability: ability.clone(),
                dc,
                advantage: false,
                disadvantage: false,
                label: body.label.clone(),
                is_magical: Some(true),
            };
            if let Ok(res) = combat_engine::resolve_save(&snap, &save_req, &stats) {
                save_passed = Some(res.passed);
            }
        }

        let dmg_roll = crate::dice::roll(&body.damage_expression, &mut rng)
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        let raw_dmg = dmg_roll.total;

        let (eff_dmg, _, _, _) = combat_engine::apply_damage_type(raw_dmg, &body.damage_type, &stats, body.is_magical);

        let mut damage_applied = eff_dmg;
        if body.half_on_save && save_passed == Some(true) {
            damage_applied = (eff_dmg as f32 / 2.0).floor() as i32;
        } else if save_passed == Some(false) || save_passed.is_none() {
            damage_applied = eff_dmg;
        }

        let (new_hp, new_temp) = combat_engine::apply_hp_damage(snap.hp_current, snap.temp_hp, damage_applied);

        sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
            .bind(new_hp).bind(new_temp).bind(cid).execute(&s.db).await?;

        let _ = sync_combatant_hp_to_sheet(&s.db, *cid, new_hp, new_temp).await;

        targets_affected.push(OverlayTargetResult {
            target_id: *cid,
            target_name: name.clone(),
            in_area: true,
            save_passed,
            damage_applied,
            hp_after: new_hp,
        });
    }

    ws::publish(campaign_id, json!({
        "type": "overlay_damage",
        "overlay_id": body.overlay_id,
        "targets": targets_affected.iter().map(|t| json!({
            "target_id": t.target_id,
            "damage": t.damage_applied,
            "hp_after": t.hp_after,
            "save_passed": t.save_passed,
        })).collect::<Vec<_>>(),
    }).to_string());

    Ok(Json(OverlayDamageResult { overlay_id: body.overlay_id, targets_affected }))
}

// =====================================================================
// Surprise Round
// =====================================================================

#[derive(Debug, Deserialize)]
pub struct SurpriseBody {
    pub surprised_combatant_ids: Vec<Uuid>,
}

pub async fn surprise_round(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SurpriseBody>,
) -> AppResult<Json<Encounter>> {
    let e = fetch(&s, id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;
    if e.status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }
    if e.round != 1 {
        return Err(AppError::BadRequest("surprise can only be set on round 1".into()));
    }

    for cid in &body.surprised_combatant_ids {
        let conditions: Vec<String> = sqlx::query_scalar("select conditions from combatants where id = $1")
            .bind(cid).fetch_one(&s.db).await?;
        let mut new_conditions = conditions.clone();
        if !new_conditions.iter().any(|c| c.to_lowercase() == "surprised") {
            new_conditions.push("surprised".to_string());
        }
        sqlx::query("update combatants set conditions = $1 where id = $2")
            .bind(&new_conditions).bind(cid).execute(&s.db).await?;
    }

    ws::publish(e.campaign_id, json!({
        "type": "surprise_round",
        "encounter_id": id,
        "surprised_ids": body.surprised_combatant_ids,
    }).to_string());

    Ok(Json(e))
}

// =====================================================================
// Surprise Round — Auto Stealth vs Passive Perception
// =====================================================================

#[derive(Debug, Deserialize)]
pub struct SurpriseAutoBody {
    pub ambusher_ids: Vec<Uuid>,
    pub defender_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Serialize)]
pub struct SurpriseAutoResult {
    pub surprised_ids: Vec<Uuid>,
    pub stealth_rolls: Vec<SurpriseStealthRoll>,
    pub perceptions: Vec<SurprisePerception>,
}

#[derive(Debug, Serialize)]
pub struct SurpriseStealthRoll {
    pub combatant_id: Uuid,
    pub name: String,
    pub stealth_total: i32,
    pub natural: i32,
}

#[derive(Debug, Serialize)]
pub struct SurprisePerception {
    pub combatant_id: Uuid,
    pub name: String,
    pub passive_perception: i32,
    pub surprised: bool,
}

pub async fn surprise_auto(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SurpriseAutoBody>,
) -> AppResult<Json<SurpriseAutoResult>> {
    let e = fetch(&s, id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;
    if e.status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }
    if e.round != 1 {
        return Err(AppError::BadRequest("surprise can only be set on round 1".into()));
    }

    let ambusher_set: std::collections::HashSet<Uuid> = body.ambusher_ids.iter().copied().collect();
    let defender_ids: Vec<Uuid> = if let Some(ref ids) = body.defender_ids {
        ids.clone()
    } else {
        sqlx::query_scalar("select id from combatants where encounter_id = $1 and initiative_rolled = true")
            .bind(id).fetch_all(&s.db).await?
            .into_iter().filter(|cid: &Uuid| !ambusher_set.contains(cid)).collect()
    };

    let mut rng = rand::rngs::StdRng::from_os_rng();
    let mut stealth_rolls = Vec::new();
    let mut max_stealth = 0i32;

    for cid in &body.ambusher_ids {
        let snap = combat_engine::load_snapshot(&s.db, *cid).await?;
        let stats = combat_engine::compute_stats(&snap);
        let stealth_mod = stats.skill_mods.iter()
            .find(|(s, _)| s == "stealth")
            .map(|(_, m)| *m)
            .unwrap_or(0);
        let expr = format!("1d20+{}", stealth_mod);
        let roll_res = crate::dice::roll(&expr, &mut rng)
            .map_err(|e| AppError::BadRequest(format!("stealth roll: {}", e)))?;
        let nat = roll_res.terms.first().and_then(|t| t.rolls.first().copied()).unwrap_or(0);
        let total = roll_res.total.max(1);
        if total > max_stealth { max_stealth = total; }
        stealth_rolls.push(SurpriseStealthRoll {
            combatant_id: *cid,
            name: snap.display_name.clone(),
            stealth_total: total,
            natural: nat,
        });
    }

    let mut perceptions = Vec::new();
    let mut surprised_ids = Vec::new();

    for cid in &defender_ids {
        let snap = combat_engine::load_snapshot(&s.db, *cid).await?;
        let stats = combat_engine::compute_stats(&snap);
        let pp = stats.passive_scores.iter()
            .find(|(s, _)| s == "perception")
            .map(|(_, m)| *m)
            .unwrap_or(10);
        let is_surprised = pp < max_stealth;
        perceptions.push(SurprisePerception {
            combatant_id: *cid,
            name: snap.display_name.clone(),
            passive_perception: pp,
            surprised: is_surprised,
        });
        if is_surprised {
            surprised_ids.push(*cid);
        }
    }

    for cid in &surprised_ids {
        let conditions: Vec<String> = sqlx::query_scalar("select conditions from combatants where id = $1")
            .bind(cid).fetch_one(&s.db).await?;
        let mut new_conditions = conditions;
        if !new_conditions.iter().any(|c| c.to_lowercase() == "surprised") {
            new_conditions.push("surprised".to_string());
        }
        sqlx::query("update combatants set conditions = $1 where id = $2")
            .bind(&new_conditions).bind(cid).execute(&s.db).await?;
    }

    ws::publish(e.campaign_id, json!({
        "type": "surprise_auto",
        "encounter_id": id,
        "surprised_ids": surprised_ids,
        "stealth_rolls": stealth_rolls.iter().map(|r| json!({"id": r.combatant_id, "total": r.stealth_total})).collect::<Vec<_>>(),
        "max_stealth": max_stealth,
    }).to_string());

    Ok(Json(SurpriseAutoResult {
        surprised_ids,
        stealth_rolls,
        perceptions,
    }))
}

// =====================================================================
// Flanking Detection
// =====================================================================

#[derive(Debug, Serialize)]
pub struct FlankResult {
    pub flanking_pairs: Vec<FlankPair>,
}

#[derive(Debug, Serialize)]
pub struct FlankPair {
    pub attacker_a_id: Uuid,
    pub attacker_a_name: String,
    pub attacker_b_id: Uuid,
    pub attacker_b_name: String,
    pub target_id: Uuid,
    pub target_name: String,
}

pub async fn check_flanking(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
) -> AppResult<Json<FlankResult>> {
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(encounter_id).fetch_one(&s.db).await?;
    rbac::require_member(&s.db, uid, campaign_id).await?;

    let tokens: Vec<(Uuid, String, f32, f32, String)> = sqlx::query_as(
        r#"select id, display_name, coalesce(token_x, 50) as x, coalesce(token_y, 50) as y,
           case when ref_type = 'character' then 'ally' else 'enemy' end as side
           from combatants
           where encounter_id = $1 and token_on_map = true and hp_current > 0"#,
    )
    .bind(encounter_id)
    .fetch_all(&s.db).await?;

    let mut pairs = Vec::new();
    let grid_size: i32 = sqlx::query_scalar("select map_grid_size from encounters where id = $1")
        .bind(encounter_id).fetch_one(&s.db).await?;

    let allies: Vec<_> = tokens.iter().filter(|t| t.4 == "ally").collect();
    let enemies: Vec<_> = tokens.iter().filter(|t| t.4 == "enemy").collect();

    for target in &enemies {
        for i in 0..allies.len() {
            for j in (i+1)..allies.len() {
                let a = allies[i];
                let b = allies[j];
                if is_flanking(a.2, a.3, b.2, b.3, target.2, target.3, grid_size) {
                    pairs.push(FlankPair {
                        attacker_a_id: a.0,
                        attacker_a_name: a.1.clone(),
                        attacker_b_id: b.0,
                        attacker_b_name: b.1.clone(),
                        target_id: target.0,
                        target_name: target.1.clone(),
                    });
                }
            }
        }
    }

    for target in &allies {
        for i in 0..enemies.len() {
            for j in (i+1)..enemies.len() {
                let a = enemies[i];
                let b = enemies[j];
                if is_flanking(a.2, a.3, b.2, b.3, target.2, target.3, grid_size) {
                    pairs.push(FlankPair {
                        attacker_a_id: a.0,
                        attacker_a_name: a.1.clone(),
                        attacker_b_id: b.0,
                        attacker_b_name: b.1.clone(),
                        target_id: target.0,
                        target_name: target.1.clone(),
                    });
                }
            }
        }
    }

    Ok(Json(FlankResult { flanking_pairs: pairs }))
}

pub fn is_flanking(ax: f32, ay: f32, bx: f32, by: f32, tx: f32, ty: f32, grid_size: i32) -> bool {
    let px_per_pct = 6.0_f32;
    let cell_pct = (grid_size as f32) / px_per_pct;
    let max_dist = cell_pct * 2.0;

    let dx_a = ax - tx;
    let dy_a = ay - ty;
    let dx_b = bx - tx;
    let dy_b = by - ty;

    let dist_a = (dx_a * dx_a + dy_a * dy_a).sqrt();
    let dist_b = (dx_b * dx_b + dy_b * dy_b).sqrt();

    if dist_a > max_dist || dist_b > max_dist { return false; }
    if dist_a < 0.01 || dist_b < 0.01 { return false; }

    let na = (dx_a / dist_a, dy_a / dist_a);
    let nb = (dx_b / dist_b, dy_b / dist_b);
    let dot = na.0 * nb.0 + na.1 * nb.1;

    dot <= -0.5
}

// =====================================================================
// Cover Calculation
// =====================================================================

#[derive(Debug, Serialize)]
pub struct CoverResult {
    pub attacker_id: Uuid,
    pub target_id: Uuid,
    pub cover_type: String,
    pub cover_bonus: i32,
    pub blockers: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CoverQuery {
    pub attacker_id: Uuid,
    pub target_id: Uuid,
}

pub async fn calculate_cover(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
    Query(q): Query<CoverQuery>,
) -> AppResult<Json<CoverResult>> {
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(encounter_id).fetch_one(&s.db).await?;
    rbac::require_member(&s.db, uid, campaign_id).await?;

    let attacker: (f32, f32) = sqlx::query_as(
        "select coalesce(token_x, 50), coalesce(token_y, 50) from combatants where id = $1 and encounter_id = $2")
        .bind(q.attacker_id).bind(encounter_id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let target: (f32, f32) = sqlx::query_as(
        "select coalesce(token_x, 50), coalesce(token_y, 50) from combatants where id = $1 and encounter_id = $2")
        .bind(q.target_id).bind(encounter_id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let others: Vec<(String, f32, f32, String)> = sqlx::query_as(
        r#"select display_name, coalesce(token_x, 50) as x, coalesce(token_y, 50) as y, ref_type::text
           from combatants
           where encounter_id = $1 and id not in ($2, $3) and token_on_map = true and hp_current > 0"#,
    )
    .bind(encounter_id)
    .bind(q.attacker_id)
    .bind(q.target_id)
    .fetch_all(&s.db).await?;

    let mut blockers = Vec::new();
    let mut max_cover = 0i32;

    for (name, ox, oy, _ref_type) in &others {
        if is_between(*ox, *oy, attacker.0, attacker.1, target.0, target.1) {
            blockers.push(name.clone());
            max_cover = (max_cover + 1).min(3);
        }
    }

    let (cover_type, cover_bonus) = match max_cover {
        1 => ("half".to_string(), 2),
        2 => ("three_quarters".to_string(), 5),
        3 => ("full".to_string(), 999),
        _ => ("none".to_string(), 0),
    };

    Ok(Json(CoverResult {
        attacker_id: q.attacker_id,
        target_id: q.target_id,
        cover_type,
        cover_bonus,
        blockers,
    }))
}

// =====================================================================
// Condition Management
// =====================================================================

#[derive(Debug, Deserialize)]
pub struct ConditionBody {
    pub condition: String,
    pub remove: Option<bool>,
    pub duration_rounds: Option<i32>,
}

pub async fn add_condition(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ConditionBody>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Uuid, String, Option<Uuid>, Vec<String>) = sqlx::query_as(
        r#"select e.campaign_id, e.id, e.status::text, ch.owner_id, c.conditions
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let (campaign_id, _encounter_id, _status, owner, conditions) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }

    let condition = body.condition.to_lowercase();
    let removing = body.remove == Some(true);

    if !removing {
        let immune = check_condition_immunity(&s.db, id, &condition).await?;
        if immune {
            return Err(AppError::BadRequest(format!(
                "immune to {}", condition
            )));
        }
    }
    let new_conditions: Vec<String> = if removing {
        conditions.into_iter().filter(|c| {
            let name = c.split(':').next().unwrap_or(c).to_lowercase();
            name != condition
        }).collect()
    } else {
        let mut c = conditions;
        let already = c.iter().any(|existing| {
            existing.split(':').next().unwrap_or(existing).to_lowercase() == condition
        });
        if !already {
            let entry = if let Some(dur) = body.duration_rounds.filter(|&d| d > 0) {
                format!("{}:{}", condition, dur)
            } else {
                condition.clone()
            };
            c.push(entry);
        }
        c
    };

    let breaks_concentration = !removing && matches!(
        condition.as_str(),
        "incapacitated" | "paralyzed" | "stunned" | "unconscious"
    );
    let releases_grapple = !removing && matches!(
        condition.as_str(),
        "incapacitated" | "paralyzed" | "stunned" | "unconscious" | "dead"
    );

    let mut tx = s.db.begin().await?;

    let c: Combatant = sqlx::query_as::<_, Combatant>(
        r#"update combatants set conditions = $1 where id = $2
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                     readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast"#,
    )
    .bind(&new_conditions)
    .bind(id)
    .fetch_one(&mut *tx).await?;

    if breaks_concentration {
        sqlx::query(
            "update combatant_effects set active = false where combatant_id = $1 and concentration = true and active = true"
        )
        .bind(id)
        .execute(&mut *tx).await?;
    }

    if releases_grapple {
        let grappler_conds: Vec<String> = sqlx::query_scalar(
            "select conditions from combatants where id = $1")
            .bind(id).fetch_optional(&mut *tx).await?.unwrap_or_default();
        if has_condition(&grappler_conds, "grappling") {
            let freed = remove_condition(grappler_conds, "grappling");
            sqlx::query("update combatants set conditions = $1 where id = $2")
                .bind(&freed).bind(id).execute(&mut *tx).await?;
            let enc_combatants: Vec<(Uuid, Vec<String>)> = sqlx::query_as(
                "select id, conditions from combatants
                 where encounter_id = (select encounter_id from combatants where id = $1)
                   and id != $1")
                .bind(id).fetch_all(&mut *tx).await?;
            for (gid, gconds) in enc_combatants {
                if has_condition(&gconds, "grappled") {
                    let new_gconds = remove_condition(gconds, "grappled");
                    sqlx::query("update combatants set conditions = $1 where id = $2")
                        .bind(&new_gconds).bind(gid).execute(&mut *tx).await?;
                    ws::publish(campaign_id, json!({
                        "type": "combatant_condition_removed",
                        "combatant_id": gid,
                        "condition": "grappled",
                        "reason": "grappler incapacitated",
                    }).to_string());
                }
            }
        }
    }

    tx.commit().await?;

    ws::publish(campaign_id, json!({
        "type": if removing { "combatant_condition_removed" } else { "combatant_condition_added" },
        "combatant_id": id,
        "condition": body.condition,
    }).to_string());

    if breaks_concentration {
        ws::publish(campaign_id, json!({
            "type": "concentration_broken",
            "combatant_id": id,
            "reason": condition,
        }).to_string());
    }

    Ok(Json(c))
}

// =====================================================================
// Bulk Effects PATCH — types only
// =====================================================================

#[derive(Debug, Deserialize)]
pub struct PatchEffectsBody {
    pub combatant_ids: Vec<Uuid>,
    pub remove_by_name: Option<String>,
    pub set_active: Option<bool>,
    pub add_effect: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct PatchEffectsResult {
    pub affected: usize,
}

// =====================================================================
// Geometry Helpers
// =====================================================================

/// Check if two line segments (A-B) and (C-D) intersect.
pub fn segments_intersect(ax: f32, ay: f32, bx: f32, by: f32, cx: f32, cy: f32, dx: f32, dy: f32) -> bool {
    let d1x = bx - ax; let d1y = by - ay;
    let d2x = dx - cx; let d2y = dy - cy;
    let denom = d1x * d2y - d1y * d2x;
    if denom.abs() < 0.0001 { return false; }

    let t = ((cx - ax) * d2y - (cy - ay) * d2x) / denom;
    let u = ((cx - ax) * d1y - (cy - ay) * d1x) / denom;
    t >= 0.0 && t <= 1.0 && u >= 0.0 && u <= 1.0
}

pub fn is_between(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> bool {
    let dx = bx - ax;
    let dy = by - ay;
    let len_sq = dx*dx + dy*dy;
    if len_sq < 0.0001 { return false; }

    let t = ((px - ax) * dx + (py - ay) * dy) / len_sq;
    if t < 0.1 || t > 0.9 { return false; }

    let dist = ((px - ax) * dy - (py - ay) * dx).abs() / len_sq.sqrt();
    dist < 3.0
}
