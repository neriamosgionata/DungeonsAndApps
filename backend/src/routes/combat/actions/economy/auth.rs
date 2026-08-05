// Auth + action-consumption helpers.
use super::*;
use sqlx::PgPool;
use uuid::Uuid;

/// Auth + encounter status + round context, returned by `require_action_auth`.
/// One DB roundtrip replaces the previous (campaign_id) + (status) + (round, turn_index)
/// pattern that each handler was doing individually.
pub struct ActionAuth {
    pub campaign_id: Uuid,
    pub encounter_id: Uuid,
    pub round: i32,
    pub turn_index: i32,
    /// Role of the calling user (Master / Player). Exposed so post-auth
    /// handlers (e.g. heal's HIGH-4 faction check) can branch on master vs
    /// non-master without an extra require_member call.
    pub role: Role,
}

/// Validates campaign membership, ownership (or master bypass), and active encounter
/// status in a single query. Eliminates the N+1 pattern that the previous audit flagged.
pub async fn require_action_auth(
    db: &PgPool,
    uid: Uuid,
    combatant_id: Uuid,
) -> AppResult<ActionAuth> {
    let row: (Uuid, Uuid, String, Option<Uuid>, i32, i32) = sqlx::query_as(
        r#"select e.campaign_id, e.id, e.status::text, ch.owner_id, e.round, e.turn_index
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(combatant_id)
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)?;
    let (campaign_id, encounter_id, status, owner, round, turn_index) = row;
    let role = rbac::require_member(db, uid, campaign_id).await?;
    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }
    if status != "active" {
        return Err(AppError::Conflict("encounter not active".into()));
    }
    Ok(ActionAuth {
        campaign_id,
        encounter_id,
        round,
        turn_index,
        role,
    })
}

/// Atomically consume action or bonus-action slot. Returns Err on already-used / 0 HP.
pub async fn consume_action_or_bonus(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    combatant_id: Uuid,
    use_bonus_action: bool,
) -> AppResult<()> {
    let (column, kind) = if use_bonus_action {
        ("bonus_action_used", "bonus action")
    } else {
        ("action_used", "action")
    };
    let row: Option<Uuid> = match column {
        "action_used" => sqlx::query_scalar(
            "update combatants set action_used = true
             where id = $1 and action_used = false and hp_current > 0 returning id",
        )
        .bind(combatant_id)
        .fetch_optional(&mut **tx)
        .await?,
        "bonus_action_used" => {
            // M-24: BA Dash/Disengage/Hide = Cunning Action (Rogue 2+) or
            // Step of the Wind (Monk 2+) — PHB p.96 / p.78. NPC combatants
            // have no class gate (GM-controlled).
            let class_gate: Option<(bool, bool)> = sqlx::query_as(
                r#"select
                     coalesce(bool_or(lower(elem->>'name') = 'rogue' and (elem->>'level')::int >= 2), false),
                     coalesce(bool_or(lower(elem->>'name') = 'monk' and (elem->>'level')::int >= 2), false)
                   from combatants c
                   left join characters ch on ch.id = c.character_id,
                   jsonb_array_elements(coalesce(ch.sheet->'classes', '[]'::jsonb)) as elem
                   where c.id = $1
                   group by c.id"#,
            )
            .bind(combatant_id)
            .fetch_optional(&mut **tx)
            .await?;
            match class_gate {
                None => {}
                Some((rogue2, monk2)) if rogue2 || monk2 => {}
                Some(_) => {
                    return Err(AppError::BadRequest(
                        "bonus-action dash/disengage/hide requires Rogue 2+ (Cunning Action) or Monk 2+ (Step of the Wind)".into(),
                    ));
                }
            }
            sqlx::query_scalar(
                "update combatants set bonus_action_used = true
                 where id = $1 and bonus_action_used = false and hp_current > 0 returning id",
            )
            .bind(combatant_id)
            .fetch_optional(&mut **tx)
            .await?
        }
        _ => unreachable!("column is one of the two hardcoded literals above"),
    };
    if row.is_none() {
        return Err(AppError::BadRequest(format!("{kind} already used")));
    }
    Ok(())
}
