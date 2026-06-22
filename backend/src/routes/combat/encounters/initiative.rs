// set_initiative — set initiative values for one or more combatants in an encounter.
use crate::rbac;
use crate::ws;
use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::extract::AuthUser;
use super::read::fetch;
use super::types::SetInitiativeBody;
use axum::Json;
use axum::extract::{Path, State};
use serde_json::json;
use uuid::Uuid;

pub async fn set_initiative(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
    Json(body): Json<SetInitiativeBody>,
) -> AppResult<Json<()>> {
    if body.combatants.is_empty() || body.combatants.len() > 50 {
        return Err(AppError::BadRequest(format!(
            "combatants array must contain 1-50 items, got {}",
            body.combatants.len()
        )));
    }
    let e = fetch(&s, encounter_id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;
    if e.status == "ended" {
        return Err(AppError::Conflict("encounter has ended".into()));
    }

    let mut tx = s.db.begin().await?;

    let ids: Vec<Uuid> = body.combatants.iter().map(|c| c.combatant_id).collect();
    let inits: Vec<i32> = body.combatants.iter().map(|c| c.initiative).collect();

    sqlx::query(
        "update combatants set initiative = c.initiative, initiative_rolled = true
         from unnest($1::uuid[], $2::int[]) as c(id, initiative)
         where combatants.id = c.id and combatants.encounter_id = $3"
    )
    .bind(&ids)
    .bind(&inits)
    .bind(encounter_id)
    .execute(&mut *tx)
    .await?;

    let matched: i64 = sqlx::query_scalar(
        "select count(*) from combatants
         where encounter_id = $1 and id = any($2)"
    )
    .bind(encounter_id)
    .bind(&ids)
    .fetch_one(&mut *tx)
    .await?;
    if matched as usize != ids.len() {
        tx.rollback().await?;
        return Err(AppError::NotFound);
    }

    sqlx::query(
        r#"update combatants c
           set turn_order = sub.new_order
           from (
             select id, (row_number() over (order by initiative desc, dex_tiebreaker desc) - 1)::int as new_order
             from combatants
             where encounter_id = $1 and initiative_rolled = true
           ) sub
           where c.id = sub.id"#,
    )
    .bind(encounter_id)
    .execute(&mut *tx)
    .await?;

    let new_orders: Vec<(Uuid, i32)> = sqlx::query_as(
        "select id, turn_order from combatants
         where encounter_id = $1 and id = any($2) and initiative_rolled = true"
    )
    .bind(encounter_id)
    .bind(&ids)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;

    for (id, _ord) in &new_orders {
        let init = body
            .combatants
            .iter()
            .find(|c| c.combatant_id == *id)
            .map(|c| c.initiative)
            .unwrap_or(0);
        ws::publish(
            e.campaign_id,
            json!({
                "type": "combatant_updates",
                "id": id,
                "initiative": init,
                "initiative_rolled": true,
            })
            .to_string(),
        );
    }

    Ok(Json(()))
}
