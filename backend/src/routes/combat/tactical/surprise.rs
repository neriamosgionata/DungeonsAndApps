// Surprise round handler and auto-stealth vs perception handler.
use super::*;
use super::super::fetch;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

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
        return Err(AppError::Conflict("encounter not active".into()));
    }
    if e.round != 1 {
        return Err(AppError::BadRequest(
            "surprise can only be set on round 1".into(),
        ));
    }

    if !body.surprised_combatant_ids.is_empty() {
        sqlx::query(
            "update combatants set conditions = array_append(conditions, 'surprised')
             where id = ANY($1) and not ('surprised' = any(conditions))",
        )
        .bind(&body.surprised_combatant_ids)
        .execute(&s.db)
        .await?;
    }

    ws::publish_persist(
        &s.db,
        e.campaign_id,
        json!({
            "type": "surprise_rounds",
            "encounter_id": id,
            "surprised_ids": body.surprised_combatant_ids,
        }),
    )
    .await;

    Ok(Json(e))
}

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
        return Err(AppError::Conflict("encounter not active".into()));
    }
    if e.round != 1 {
        return Err(AppError::BadRequest(
            "surprise can only be set on round 1".into(),
        ));
    }

    let ambusher_set: std::collections::HashSet<Uuid> = body.ambusher_ids.iter().copied().collect();
    let defender_ids: Vec<Uuid> = if let Some(ref ids) = body.defender_ids {
        ids.clone()
    } else {
        sqlx::query_scalar(
            "select id from combatants where encounter_id = $1 and initiative_rolled = true",
        )
        .bind(id)
        .fetch_all(&s.db)
        .await?
        .into_iter()
        .filter(|cid: &Uuid| !ambusher_set.contains(cid))
        .collect()
    };

    let all_ids: Vec<Uuid> = body
        .ambusher_ids
        .iter()
        .chain(defender_ids.iter())
        .copied()
        .collect();
    let snapshots = combat_engine::load_snapshots_batch(&s.db, &all_ids).await?;

    let mut rng = rand::rngs::StdRng::from_os_rng();
    let mut stealth_rolls = Vec::new();
    let mut max_stealth = 0i32;

    for cid in &body.ambusher_ids {
        let snap = snapshots.get(cid).ok_or_else(|| AppError::NotFound)?;
        let stats = combat_engine::compute_stats(snap);
        let stealth_mod = stats
            .skill_mods
            .iter()
            .find(|(s, _)| s == "stealth")
            .map(|(_, m)| *m)
            .unwrap_or(0);
        let expr = format!("1d20+{}", stealth_mod);
        let roll_res = crate::dice::roll(&expr, &mut rng)
            .map_err(|e| AppError::BadRequest(format!("stealth roll: {}", e)))?;
        let nat = roll_res
            .terms
            .first()
            .and_then(|t| t.rolls.first().copied())
            .unwrap_or(0);
        let total = roll_res.total.max(1);
        if total > max_stealth {
            max_stealth = total;
        }
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
        let snap = snapshots.get(cid).ok_or_else(|| AppError::NotFound)?;
        let stats = combat_engine::compute_stats(snap);
        let pp = stats
            .passive_scores
            .iter()
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

    if !surprised_ids.is_empty() {
        sqlx::query(
            "update combatants set conditions = array_append(conditions, 'surprised')
             where id = ANY($1) and not ('surprised' = any(conditions))",
        )
        .bind(&surprised_ids)
        .execute(&s.db)
        .await?;
    }

    ws::publish_persist(&s.db, e.campaign_id, json!({
        "type": "surprise_auto",
        "encounter_id": id,
        "surprised_ids": surprised_ids,
        "stealth_rolls": stealth_rolls.iter().map(|r| json!({"id": r.combatant_id, "total": r.stealth_total})).collect::<Vec<_>>(),
        "max_stealth": max_stealth,
    }))
    .await;

    Ok(Json(SurpriseAutoResult {
        surprised_ids,
        stealth_rolls,
        perceptions,
    }))
}
