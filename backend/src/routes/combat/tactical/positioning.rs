// Flanking, cover, and geometry helpers.
use super::*;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    let campaign_id: Uuid = sqlx::query_scalar("select campaign_id from encounters where id = $1")
        .bind(encounter_id)
        .fetch_one(&s.db)
        .await?;
    rbac::require_member(&s.db, uid, campaign_id).await?;

    let tokens: Vec<(Uuid, String, f32, f32, String)> = sqlx::query_as(
        r#"select id, display_name, coalesce(token_x, 50) as x, coalesce(token_y, 50) as y,
           case when ref_type = 'character' then 'ally' else 'enemy' end as side
           from combatants
           where encounter_id = $1 and token_on_map = true and hp_current > 0"#,
    )
    .bind(encounter_id)
    .fetch_all(&s.db)
    .await?;

    let mut pairs = Vec::new();
    let grid_size: i32 = sqlx::query_scalar("select map_grid_size from encounters where id = $1")
        .bind(encounter_id)
        .fetch_one(&s.db)
        .await?;

    let allies: Vec<_> = tokens.iter().filter(|t| t.4 == "ally").collect();
    let enemies: Vec<_> = tokens.iter().filter(|t| t.4 == "enemy").collect();

    for target in &enemies {
        for i in 0..allies.len() {
            for j in (i + 1)..allies.len() {
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
            for j in (i + 1)..enemies.len() {
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

    Ok(Json(FlankResult {
        flanking_pairs: pairs,
    }))
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

    if dist_a > max_dist || dist_b > max_dist {
        return false;
    }
    if dist_a < 0.01 || dist_b < 0.01 {
        return false;
    }

    let na = (dx_a / dist_a, dy_a / dist_a);
    let nb = (dx_b / dist_b, dy_b / dist_b);
    let dot = na.0 * nb.0 + na.1 * nb.1;

    dot <= -0.5
}

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
    let campaign_id: Uuid = sqlx::query_scalar("select campaign_id from encounters where id = $1")
        .bind(encounter_id)
        .fetch_one(&s.db)
        .await?;
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

pub fn segments_intersect(
    ax: f32,
    ay: f32,
    bx: f32,
    by: f32,
    cx: f32,
    cy: f32,
    dx: f32,
    dy: f32,
) -> bool {
    let d1x = bx - ax;
    let d1y = by - ay;
    let d2x = dx - cx;
    let d2y = dy - cy;
    let denom = d1x * d2y - d1y * d2x;
    if denom.abs() < 0.0001 {
        return false;
    }

    let t = ((cx - ax) * d2y - (cy - ay) * d2x) / denom;
    let u = ((cx - ax) * d1y - (cy - ay) * d1x) / denom;
    t >= 0.0 && t <= 1.0 && u >= 0.0 && u <= 1.0
}

pub fn is_between(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> bool {
    let dx = bx - ax;
    let dy = by - ay;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 0.0001 {
        return false;
    }

    let t = ((px - ax) * dx + (py - ay) * dy) / len_sq;
    if t < 0.1 || t > 0.9 {
        return false;
    }

    let dist = ((px - ax) * dy - (py - ay) * dx).abs() / len_sq.sqrt();
    dist < 3.0
}
