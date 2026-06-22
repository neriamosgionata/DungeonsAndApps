// AoE overlay auto-damage.
use super::*;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use super::super::actions::sync_combatant_hp_to_sheet;

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
    let campaign_id: Uuid = sqlx::query_scalar("select campaign_id from encounters where id = $1")
        .bind(encounter_id)
        .fetch_optional(&s.db)
        .await?
        .ok_or(AppError::NotFound)?;
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
        "select id, display_name, token_x::float8 as token_x, token_y::float8 as token_y from combatants where encounter_id = $1",
    )
    .bind(encounter_id)
    .fetch_all(&s.db)
    .await?;

    // Pre-compute in-area combatant ids (one pass; needs the coords from above).
    let mut in_area_ids: Vec<Uuid> = Vec::new();
    for (cid, _name, tx, ty) in &combatants {
        let in_area = if let (Some(x), Some(y)) = (tx, ty) {
            match shape.as_str() {
                "circle" => {
                    let r = radius.unwrap_or(20) as f64;
                    let dx = *x - origin.0;
                    let dy = *y - origin.1;
                    (dx * dx + dy * dy).sqrt() <= r
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
                    (dx * dx + dy * dy).sqrt() <= r
                }
            }
        } else {
            false
        };
        if in_area {
            in_area_ids.push(*cid);
        }
    }
    // Batch load snapshots for in-area combatants (1 query instead of N).
    let snaps = combat_engine::load_snapshots_batch(&s.db, &in_area_ids).await?;

    let mut rng = rand::rngs::StdRng::from_os_rng();
    let mut targets_affected = Vec::new();
    let mut sheet_syncs: Vec<(Uuid, i32, i32)> = Vec::new();

    let mut tx = s.db.begin().await?;

    for (cid, name, _tx_pos, _ty_pos) in combatants.into_iter().filter(|(cid, _, tx_pos, ty_pos)| {
        snaps.contains_key(cid) && tx_pos.is_some() && ty_pos.is_some()
    }) {
        let snap = match snaps.get(&cid) {
            Some(s) => s,
            None => continue,
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

        let (eff_dmg, _, _, _) =
            combat_engine::apply_damage_type(raw_dmg, &body.damage_type, &stats, body.is_magical);

        let mut damage_applied = eff_dmg;
        if body.half_on_save && save_passed == Some(true) {
            damage_applied = (eff_dmg as f32 / 2.0).floor() as i32;
        } else if save_passed == Some(false) || save_passed.is_none() {
            damage_applied = eff_dmg;
        }

        let (new_hp, new_temp) =
            combat_engine::apply_hp_damage(snap.hp_current, snap.temp_hp, damage_applied);

        sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
            .bind(new_hp)
            .bind(new_temp)
            .bind(cid)
            .execute(&mut *tx)
            .await?;

        sheet_syncs.push((cid, new_hp, new_temp));
        targets_affected.push(OverlayTargetResult {
            target_id: cid,
            target_name: name.clone(),
            in_area: true,
            save_passed,
            damage_applied,
            hp_after: new_hp,
        });
    }

    tx.commit().await?;

    for (cid, hp, temp) in &sheet_syncs {
        if let Err(e) = sync_combatant_hp_to_sheet(&s.db, *cid, *hp, *temp).await {
            tracing::error!(combatant_id = %cid, "sync sheet HP: {e}");
        }
    }

    ws::publish(
        campaign_id,
        json!({
            "type": "overlay_damages",
            "overlay_id": body.overlay_id,
            "targets": targets_affected.iter().map(|t| json!({
                "target_id": t.target_id,
                "damage": t.damage_applied,
                "hp_after": t.hp_after,
                "save_passed": t.save_passed,
            })).collect::<Vec<_>>(),
        })
        .to_string(),
    );

    Ok(Json(OverlayDamageResult {
        overlay_id: body.overlay_id,
        targets_affected,
    }))
}
