// Economy & utility action route handlers: dodge, disengage, help, dash, hide,
// search, use_object, opportunity_attack, two_weapon_fight, delay_turn, contested_hide.
// Extracted from actions.rs to keep the route handler file under the 500-line
// guideline (per AGENTS.md §1.4). Public re-exports preserve call-site compatibility.
use super::*;
use crate::error::AppResult;
use crate::extract::AuthUser;
use crate::AppState;
use axum::extract::{Path, State};
use axum::Json;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct SpecialActionBody {
    pub _target_id: Option<Uuid>, // for Help action
}

pub async fn dodge(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Uuid, String, Option<Uuid>) = sqlx::query_as(
        r#"select e.campaign_id, e.id, e.status::text, ch.owner_id
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let (campaign_id, _encounter_id, status, owner) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }
    if status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }

    let (_round, _turn_index): (i32, i32) = sqlx::query_as(
        "select round, turn_index from encounters where id = $1")
        .bind(_encounter_id).fetch_one(&s.db).await?;

    let mut tx = s.db.begin().await?;
    let action_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set action_used = true where id = $1 and action_used = false and hp_current > 0 returning id")
        .bind(id).fetch_optional(&mut *tx).await?;
    if action_consumed.is_none() {
        return Err(AppError::BadRequest("action already used".into()));
    }

    sqlx::query("update combatant_effects set active = false where combatant_id = $1 and name = 'Dodge'")
        .bind(id).execute(&mut *tx).await?;

    sqlx::query(
        r#"insert into combatant_effects
           (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
            concentration, active, modifiers, source_type)
           values ($1, 'Dodge', 'buff', 'shield', 'rounds', 1, 1, 'caster_turn_start',
                   false, true, '{"attack_disadvantage_against": true, "dex_save_advantage": true}', 'ability')"#,
    )
    .bind(id)
    .execute(&mut *tx).await?;

    tx.commit().await?;

    let c = refresh_combatant(&s.db, id).await?;
    ws::publish(campaign_id, json!({"type":"combatant_dodges","id":id}).to_string());
    Ok(Json(c))
}

pub async fn disengage(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ActionBody>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Uuid, String, Option<Uuid>) = sqlx::query_as(
        r#"select e.campaign_id, e.id, e.status::text, ch.owner_id
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let (campaign_id, _encounter_id, status, owner) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }
    if status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }

    let (_round, _turn_index): (i32, i32) = sqlx::query_as(
        "select round, turn_index from encounters where id = $1")
        .bind(_encounter_id).fetch_one(&s.db).await?;

    let mut tx = s.db.begin().await?;
    if body.use_bonus_action {
        let ba_consumed: Option<Uuid> = sqlx::query_scalar(
            "update combatants set bonus_action_used = true where id = $1 and bonus_action_used = false and hp_current > 0 returning id")
            .bind(id).fetch_optional(&mut *tx).await?;
        if ba_consumed.is_none() {
            return Err(AppError::BadRequest("bonus action already used".into()));
        }
    } else {
        let action_consumed: Option<Uuid> = sqlx::query_scalar(
            "update combatants set action_used = true where id = $1 and action_used = false and hp_current > 0 returning id")
            .bind(id).fetch_optional(&mut *tx).await?;
        if action_consumed.is_none() {
            return Err(AppError::BadRequest("action already used".into()));
        }
    }

    sqlx::query("update combatant_effects set active = false where combatant_id = $1 and name = 'Disengage'")
        .bind(id).execute(&mut *tx).await?;

    sqlx::query(
        r#"insert into combatant_effects
           (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
            concentration, active, modifiers, source_type)
           values ($1, 'Disengage', 'buff', 'wind', 'rounds', 1, 1, 'caster_turn_start',
                   false, true, '{"disengage": true}', 'ability')"#,
    )
    .bind(id)
    .execute(&mut *tx).await?;

    tx.commit().await?;

    let c = refresh_combatant(&s.db, id).await?;
    ws::publish(campaign_id, json!({"type":"combatant_disengages","id":id}).to_string());
    Ok(Json(c))
}

pub async fn help_action(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SpecialActionBody>,
) -> AppResult<Json<Combatant>> {
    let target_id = body._target_id.ok_or(AppError::BadRequest("target_id required".into()))?;
    let row: (Uuid, Uuid, String, Option<Uuid>) = sqlx::query_as(
        r#"select e.campaign_id, e.id, e.status::text, ch.owner_id
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let (campaign_id, encounter_id, status, owner) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }
    if status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }

    let (_round, _turn_index): (i32, i32) = sqlx::query_as(
        "select round, turn_index from encounters where id = $1")
        .bind(encounter_id).fetch_one(&s.db).await?;

    let mut tx = s.db.begin().await?;
    let action_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set action_used = true where id = $1 and action_used = false and hp_current > 0 returning id")
        .bind(id).fetch_optional(&mut *tx).await?;
    if action_consumed.is_none() {
        return Err(AppError::BadRequest("action already used".into()));
    }

    sqlx::query(
        r#"insert into combatant_effects
           (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
            concentration, active, modifiers, source_type)
           values ($1, 'Helped', 'buff', 'hand', 'rounds', 1, 1, 'target_turn_start',
                   false, true, '{"attack_advantage_against": true}', 'ability')"#,
    )
    .bind(target_id)
    .execute(&mut *tx).await?;

    tx.commit().await?;

    let c = refresh_combatant(&s.db, id).await?;
    ws::publish(campaign_id, json!({"type":"combatant_helps","helper_id":id,"target_id":target_id}).to_string());
    Ok(Json(c))
}

#[derive(Debug, Deserialize)]
pub struct OppAttackBody {
    pub target_id: Uuid,
}

pub async fn opportunity_attack(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<OppAttackBody>,
) -> AppResult<Json<combat_engine::AttackResult>> {
    let attacker_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let target_snap = combat_engine::load_snapshot(&s.db, body.target_id).await?;

    if attacker_snap.encounter_id != target_snap.encounter_id {
        return Err(AppError::BadRequest("not in same encounter".into()));
    }

    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(attacker_snap.encounter_id).fetch_one(&s.db).await?;
    let encounter_status: String = sqlx::query_scalar(
        "select status::text as status from encounters where id = $1")
        .bind(attacker_snap.encounter_id).fetch_one(&s.db).await?;
    if encounter_status != "active" {
        return Err(AppError::Conflict("encounter not active".into()));
    }
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    let attacker_stats = combat_engine::compute_stats(&attacker_snap);
    if attacker_stats.incapacitated {
        return Err(AppError::BadRequest("attacker is incapacitated".into()));
    }

    let target_stats = combat_engine::compute_stats(&target_snap);
    let has_disengage = target_snap.active_effects.iter().any(|e| {
        e.modifiers.as_object().map(|m| m.get("disengage").and_then(|v| v.as_bool()) == Some(true)).unwrap_or(false)
    });
    if has_disengage {
        return Err(AppError::BadRequest("target has disengaged".into()));
    }

    let req = combat_engine::AttackReq {
        target_id: body.target_id,
        attack_expression: None,
        damage_expression: None,
        damage_type: "bludgeoning".to_string(),
        damage_die: None,
        ability: Some("str".to_string()),
        proficient: Some(true),
        advantage: false,
        disadvantage: false,
        cover: None,
        is_spell_attack: false,
        is_magical: false,
        label: Some("Opportunity Attack".to_string()),
        weapon_id: None,
        extra_damage_expression: None,
        extra_damage_type: None,
        power_attack: false,
        reckless: false,
        bless_dice: None,
        bardic_inspiration_dice: None,
    };

    let result = combat_engine::resolve_attack(&attacker_snap, &target_snap, &req, &attacker_stats, &target_stats)
        .map_err(|e| AppError::BadRequest(e))?;

    let mut tx = s.db.begin().await?;
    let reaction_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set reaction_used = true where id = $1 and reaction_used = false and hp_current > 0 returning id")
        .bind(id).fetch_optional(&mut *tx).await?;
    if reaction_consumed.is_none() {
        return Err(AppError::BadRequest("reaction already used".into()));
    }

    if result.hit {
        sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
            .bind(result.target_hp_after).bind(result.target_temp_hp_after).bind(body.target_id)
            .execute(&mut *tx).await?;
        if result.concentration_broken {
            sqlx::query("update combatant_effects set active = false where combatant_id = $1 and concentration = true and active = true")
                .bind(body.target_id).execute(&mut *tx).await?;
        }
        if result.instant_death {
            if let Some(chid) = target_snap.character_id {
                sqlx::query(
                    r#"update characters set sheet = coalesce(sheet,'{}'::jsonb)
                       || jsonb_build_object('alive', false,
                            'death_saves', jsonb_build_object('successes', 0, 'failures', 3))
                       where id = $1"#)
                    .bind(chid).execute(&mut *tx).await?;
            }
        }
    }

    sqlx::query(
        "update combatant_effects set active = false
         where combatant_id = $1 and active = true
           and modifiers->>'hidden' = 'true'")
        .bind(id).execute(&mut *tx).await?;

    tx.commit().await?;

    if result.hit {
        if let Err(e) = sync_combatant_hp_to_sheet(&s.db, body.target_id, result.target_hp_after, result.target_temp_hp_after).await {
            tracing::error!(combatant_id = %body.target_id, "sync sheet HP: {e}");
        }
    }

    ws::publish(campaign_id, json!({
        "type": "combatant_opportunity_attacks",
        "attacker_id": id,
        "target_id": body.target_id,
        "hit": result.hit,
        "damage": result.damage_applied,
        "instant_death": result.instant_death,
    }).to_string());

    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
pub struct DelayBody {
    pub insert_after_turn_index: i32,
}

pub async fn delay_turn(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<DelayBody>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Uuid, String, i32, Option<Uuid>) = sqlx::query_as(
        r#"select e.campaign_id, e.id, e.status::text, c.turn_order, ch.owner_id
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let (campaign_id, encounter_id, status, current_turn, owner) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }
    if status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }

    let mut tx = s.db.begin().await?;
    let c: Option<Combatant> = sqlx::query_as::<_, Combatant>(
        r#"update combatants set delayed_turn = true, action_used = true, readied_action = null
           where id = $1 and action_used = false
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast, level_override, vision_range, pending_hits"#,
    )
    .bind(id)
    .fetch_optional(&mut *tx).await?;

    let c = c.ok_or_else(|| AppError::BadRequest("action already used this turn".into()))?;

    sqlx::query(
        r#"update combatants set turn_order = case
            when turn_order > $1 and turn_order <= $2 then turn_order - 1
            when turn_order = $1 then $2
            else turn_order
           end
           where encounter_id = $3"#,
    )
    .bind(current_turn)
    .bind(body.insert_after_turn_index)
    .bind(encounter_id)
    .execute(&mut *tx).await?;

    tx.commit().await?;

    ws::publish(campaign_id, json!({
        "type": "combatant_delayed",
        "id": id,
        "insert_after": body.insert_after_turn_index,
    }).to_string());

    Ok(Json(c))
}

#[derive(Debug, Deserialize)]
pub struct ActionBody {
    #[serde(default)]
    pub use_bonus_action: bool,
}

#[derive(Debug, Deserialize)]
pub struct TwoWeaponFightBody {
    pub target_id: Uuid,
    pub offhand_weapon_id: String,
}

pub async fn two_weapon_fight(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<TwoWeaponFightBody>,
) -> AppResult<Json<combat_engine::AttackResult>> {
    let attacker_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let target_snap = combat_engine::load_snapshot(&s.db, body.target_id).await?;

    if attacker_snap.encounter_id != target_snap.encounter_id {
        return Err(AppError::BadRequest("attacker and target not in same encounter".into()));
    }

    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(attacker_snap.encounter_id).fetch_one(&s.db).await?;
    let encounter_status: String = sqlx::query_scalar(
        "select status::text as status from encounters where id = $1")
        .bind(attacker_snap.encounter_id).fetch_one(&s.db).await?;
    if encounter_status != "active" {
        return Err(AppError::Conflict("encounter not active".into()));
    }
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    if attacker_snap.hp_current <= 0 {
        return Err(AppError::BadRequest("cannot act while at 0 HP".into()));
    }
    let incapacitated = attacker_snap.conditions.iter().any(|c| {
        let cl = c.to_lowercase();
        cl.starts_with("incapacitated") || cl.starts_with("paralyzed") || cl.starts_with("petrified") || cl.starts_with("stunned") || cl.starts_with("unconscious")
    });
    if incapacitated {
        return Err(AppError::BadRequest("cannot act while incapacitated".into()));
    }

    let attacker_stats = combat_engine::compute_stats(&attacker_snap);
    let target_stats = combat_engine::compute_stats(&target_snap);

    let twf_style = attacker_snap.sheet_raw.get("features")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().any(|f| {
            f.get("name").and_then(|v| v.as_str())
                .map(|n| n.to_lowercase().contains("two-weapon fighting"))
                .unwrap_or(false)
        }))
        .unwrap_or(false);

    let offhand_weapon = combat_engine::find_weapon(&attacker_snap, &body.offhand_weapon_id);
    let offhand_props = offhand_weapon.as_ref().map(|(_, p)| p.clone()).unwrap_or_default();

    if (offhand_props.ranged || offhand_props.thrown)
        && let (Some((w, _)), Some(tx), Some(ty)) = (&offhand_weapon, target_snap.token_x, target_snap.token_y)
        && let (Some(ax), Some(ay)) = (attacker_snap.token_x, attacker_snap.token_y)
        && let Some(range_str) = w.get("range").and_then(|v| v.as_str())
    {
        let parts: Vec<&str> = range_str.split('/').collect();
        if parts.len() == 2 {
            if let Ok(_normal_range) = parts[0].trim().parse::<f32>() {
                if let Ok(long_range) = parts[1].trim().trim_end_matches("ft").trim().parse::<f32>() {
                    let g_size: i32 = sqlx::query_scalar("select map_grid_size from encounters where id = $1")
                        .bind(attacker_snap.encounter_id).fetch_one(&s.db).await?;
                    let cell_pct = (g_size as f32) / 6.0;
                    let dist_pct = ((ax - tx).powi(2) + (ay - ty).powi(2)).sqrt();
                    let dist_ft = dist_pct / cell_pct * 5.0;
                    if dist_ft > long_range {
                        return Err(AppError::BadRequest(format!(
                            "target out of off-hand weapon range ({} ft > {} ft max)", dist_ft as i32, long_range as i32
                        )));
                    }
                }
            }
        }
    }

    let result = combat_engine::resolve_two_weapon_attack(
        &attacker_snap, &target_snap, &body.offhand_weapon_id, &attacker_stats, &target_stats, twf_style
    ).map_err(|e| AppError::BadRequest(e))?;

    let round: i32 = sqlx::query_scalar("select round from encounters where id = $1")
        .bind(attacker_snap.encounter_id).fetch_one(&s.db).await?;

    let mut tx = s.db.begin().await?;

    let bonus_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set bonus_action_used = true where id = $1 and bonus_action_used = false and hp_current > 0 returning id")
        .bind(id).fetch_optional(&mut *tx).await?;
    if bonus_consumed.is_none() {
        return Err(AppError::BadRequest("bonus action already used".into()));
    }

    if let Some((w, _)) = &offhand_weapon {
        let wname = w.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let props = w.get("properties").and_then(|v| v.as_str()).unwrap_or("");
        if props.to_lowercase().contains("thrown") {
            if let Some(chid) = attacker_snap.character_id {
                let _ = decrement_thrown_weapon(&mut *tx, chid, wname).await?;
            }
        }
    }

    if result.hit {
        sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
            .bind(result.target_hp_after).bind(result.target_temp_hp_after).bind(body.target_id)
            .execute(&mut *tx).await?;
        if result.concentration_broken {
            sqlx::query("update combatant_effects set active = false where combatant_id = $1 and concentration = true and active = true")
                .bind(body.target_id).execute(&mut *tx).await?;
        }
    }

    let event_action = if result.hit {
        format!("{} TWF {}: {} damage", attacker_snap.display_name, target_snap.display_name, result.damage_applied)
    } else {
        format!("{} TWF {}: missed ({} vs AC {})", attacker_snap.display_name, target_snap.display_name, result.attack_total, result.target_ac)
    };
    sqlx::query(
        "insert into combat_events (encounter_id, round, actor_combatant, target_combatant, action, delta_hp, note) values ($1, $2, $3, $4, $5, $6, $7)")
        .bind(attacker_snap.encounter_id)
        .bind(round)
        .bind(id)
        .bind(body.target_id)
        .bind(&event_action)
        .bind(if result.hit { -result.damage_applied } else { 0 })
        .execute(&mut *tx).await?;

    tx.commit().await?;

    if result.hit {
        if let Err(e) = sync_combatant_hp_to_sheet(&s.db, body.target_id, result.target_hp_after, result.target_temp_hp_after).await {
            tracing::error!(combatant_id = %body.target_id, "sync sheet HP: {e}");
        }
    }

    ws::publish(campaign_id, json!({
        "type": "combatant_two_weapon_fights",
        "attacker_id": id,
        "target_id": body.target_id,
        "hit": result.hit,
        "critical": result.critical,
        "damage": if result.hit { Some(result.damage_applied) } else { None },
        "hp_after": if result.hit { Some(result.target_hp_after) } else { None },
        "temp_hp_after": if result.hit { Some(result.target_temp_hp_after) } else { None },
        "concentration_breaks": if result.hit { Some(result.concentration_broken) } else { None },
        "attack_total": if !result.hit { Some(result.attack_total) } else { None },
        "target_ac": result.target_ac,
    }).to_string());

    Ok(Json(result))
}

pub async fn dash(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ActionBody>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Uuid, String, Option<Uuid>) = sqlx::query_as(
        r#"select e.campaign_id, e.id, e.status::text, ch.owner_id
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let (campaign_id, _encounter_id, status, owner) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }
    if status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }

    let mut tx = s.db.begin().await?;

    if body.use_bonus_action {
        let ba_consumed: Option<Uuid> = sqlx::query_scalar(
            "update combatants set bonus_action_used = true where id = $1 and bonus_action_used = false and hp_current > 0 returning id")
            .bind(id).fetch_optional(&mut *tx).await?;
        if ba_consumed.is_none() {
            return Err(AppError::BadRequest("bonus action already used".into()));
        }
    } else {
        let action_consumed: Option<Uuid> = sqlx::query_scalar(
            "update combatants set action_used = true where id = $1 and action_used = false and hp_current > 0 returning id")
            .bind(id).fetch_optional(&mut *tx).await?;
        if action_consumed.is_none() {
            return Err(AppError::BadRequest("action already used".into()));
        }
    }

    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    let stats = combat_engine::compute_stats(&snap);
    let extra = stats.speed.max(0);

    sqlx::query(
        r#"insert into combatant_effects
           (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
            concentration, active, modifiers, source_type)
           values ($1, 'Dash', 'buff', 'bolt', 'rounds', 1, 1, 'caster_turn_start',
                   false, true, $2, 'ability')"#,
    )
    .bind(id)
    .bind(json!({"movement": {"type": "dash_bonus", "distance_ft": extra}}))
    .execute(&mut *tx).await?;

    tx.commit().await?;

    let c = refresh_combatant(&s.db, id).await?;
    ws::publish(campaign_id, json!({"type":"combatant_dashes","id":id,"extra_movement":extra}).to_string());
    Ok(Json(c))
}

pub async fn hide(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ActionBody>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Uuid, String, Option<Uuid>) = sqlx::query_as(
        r#"select e.campaign_id, e.id, e.status::text, ch.owner_id
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let (campaign_id, _encounter_id, status, owner) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }
    if status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }

    let mut tx = s.db.begin().await?;

    if body.use_bonus_action {
        let ba_consumed: Option<Uuid> = sqlx::query_scalar(
            "update combatants set bonus_action_used = true where id = $1 and bonus_action_used = false and hp_current > 0 returning id")
            .bind(id).fetch_optional(&mut *tx).await?;
        if ba_consumed.is_none() {
            return Err(AppError::BadRequest("bonus action already used".into()));
        }
    } else {
        let action_consumed: Option<Uuid> = sqlx::query_scalar(
            "update combatants set action_used = true where id = $1 and action_used = false and hp_current > 0 returning id")
            .bind(id).fetch_optional(&mut *tx).await?;
        if action_consumed.is_none() {
            return Err(AppError::BadRequest("action already used".into()));
        }
    }

    sqlx::query(
        r#"insert into combatant_effects
           (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
            concentration, active, modifiers, source_type)
           values ($1, 'Hidden', 'buff', 'eye-slash', 'rounds', 1, 1, 'caster_turn_start',
                   false, true, '{"hidden": true}', 'ability')"#,
    )
    .bind(id)
    .execute(&mut *tx).await?;

    tx.commit().await?;

    let c = refresh_combatant(&s.db, id).await?;
    ws::publish(campaign_id, json!({"type":"combatant_hides","id":id}).to_string());
    Ok(Json(c))
}

#[derive(Debug, Deserialize)]
pub struct ContestedHideBody {
    pub observer_ids: Option<Vec<Uuid>>,
    pub use_bonus_action: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ContestedHideResult {
    pub hider_id: Uuid,
    pub hider_name: String,
    pub stealth_total: i32,
    pub natural: i32,
    pub observers: Vec<HideObserverResult>,
    pub hidden: bool,
}

#[derive(Debug, Serialize)]
pub struct HideObserverResult {
    pub observer_id: Uuid,
    pub observer_name: String,
    pub passive_perception: i32,
    pub spotted: bool,
}

pub async fn contested_hide(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ContestedHideBody>,
) -> AppResult<Json<ContestedHideResult>> {
    let row: (Uuid, Uuid, String, Option<Uuid>) = sqlx::query_as(
        r#"select e.campaign_id, e.id, e.status::text, ch.owner_id
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let (campaign_id, encounter_id, status, owner) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }
    if status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }

    let hider_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let hider_stats = combat_engine::compute_stats(&hider_snap);
    let stealth_mod = hider_stats.skill_mods.iter()
        .find(|(s, _)| s == "stealth")
        .map(|(_, m)| *m)
        .unwrap_or(0);

    let mut rng = rand::rngs::StdRng::from_os_rng();
    let expr = format!("1d20+{}", stealth_mod);
    let roll = crate::dice::roll(&expr, &mut rng)
        .map_err(|e| AppError::BadRequest(format!("stealth roll: {}", e)))?;
    let natural = roll.terms.first().and_then(|t| t.rolls.first().copied()).unwrap_or(0);
    let stealth_total = roll.total.max(1);

    let observer_ids: Vec<Uuid> = if let Some(ref ids) = body.observer_ids {
        ids.clone()
    } else {
        sqlx::query_scalar(
            r#"select c.id from combatants c
               where c.encounter_id = $1 and c.id != $2
               and c.hp_current > 0 and c.initiative_rolled = true
               and ((c.ref_type = 'character' and $3 = 'npc') or (c.ref_type = 'npc' and $3 = 'character'))"#,
        )
        .bind(encounter_id).bind(id)
        .bind(if hider_snap.character_id.is_some() { "character" } else { "npc" })
        .fetch_all(&s.db).await?
    };
    if observer_ids.is_empty() {
        return Err(AppError::BadRequest("no observers to hide from".into()));
    }

    let mut observers = Vec::new();
    let mut all_spotted = true;

    for oid in &observer_ids {
        let snap = combat_engine::load_snapshot(&s.db, *oid).await?;
        let stats = combat_engine::compute_stats(&snap);
        let pp = stats.passive_scores.iter()
            .find(|(s, _)| s == "perception")
            .map(|(_, m)| *m)
            .unwrap_or(10);
        let spotted = pp >= stealth_total;
        if !spotted { all_spotted = false; }
        observers.push(HideObserverResult {
            observer_id: *oid,
            observer_name: snap.display_name.clone(),
            passive_perception: pp,
            spotted,
        });
    }

    let mut tx = s.db.begin().await?;

    if body.use_bonus_action.unwrap_or(false) {
        let ba_consumed: Option<Uuid> = sqlx::query_scalar(
            "update combatants set bonus_action_used = true where id = $1 and bonus_action_used = false and hp_current > 0 returning id")
            .bind(id).fetch_optional(&mut *tx).await?;
        if ba_consumed.is_none() {
            return Err(AppError::BadRequest("bonus action already used".into()));
        }
    } else {
        let action_consumed: Option<Uuid> = sqlx::query_scalar(
            "update combatants set action_used = true where id = $1 and action_used = false and hp_current > 0 returning id")
            .bind(id).fetch_optional(&mut *tx).await?;
        if action_consumed.is_none() {
            return Err(AppError::BadRequest("action already used".into()));
        }
    }

    if !all_spotted {
        sqlx::query(
            r#"insert into combatant_effects
               (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
                concentration, active, modifiers, source_type)
               values ($1, 'Hidden', 'buff', 'eye-slash', 'rounds', 1, 1, 'caster_turn_start',
                       false, true, '{"hidden": true}', 'ability')"#,
        )
        .bind(id)
        .execute(&mut *tx).await?;
    }

    tx.commit().await?;

    let hidden = !all_spotted;
    ws::publish(campaign_id, json!({
        "type": "combatant_contested_hide",
        "hider_id": id,
        "stealth_total": stealth_total,
        "hidden": hidden,
        "observer_count": observers.len(),
    }).to_string());

    Ok(Json(ContestedHideResult {
        hider_id: id,
        hider_name: hider_snap.display_name.clone(),
        stealth_total,
        natural,
        observers,
        hidden,
    }))
}

#[derive(Debug, Deserialize)]
pub struct SearchBody {
    pub label: Option<String>,
}

pub async fn search_action(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SearchBody>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Uuid, String, Option<Uuid>) = sqlx::query_as(
        r#"select e.campaign_id, e.id, e.status::text, ch.owner_id
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let (campaign_id, encounter_id, status, owner) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }
    if status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }

    let mut tx = s.db.begin().await?;
    let action_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set action_used = true where id = $1 and action_used = false and hp_current > 0 returning id")
        .bind(id).fetch_optional(&mut *tx).await?;
    if action_consumed.is_none() {
        return Err(AppError::BadRequest("action already used".into()));
    }

    let round: i32 = sqlx::query_scalar("select round from encounters where id = $1")
        .bind(encounter_id).fetch_one(&s.db).await?;

    let label = body.label.unwrap_or_else(|| "Search".to_string());
    sqlx::query(
        "insert into combat_events (encounter_id, round, actor_combatant, action, note) values ($1, $2, $3, $4, $5)")
        .bind(encounter_id).bind(round).bind(id).bind(&label).bind("search")
        .execute(&mut *tx).await?;

    tx.commit().await?;

    let c = refresh_combatant(&s.db, id).await?;
    ws::publish(campaign_id, json!({"type":"combatant_searches","id":id,"label":label}).to_string());
    Ok(Json(c))
}

#[derive(Debug, Deserialize)]
pub struct UseObjectBody {
    pub label: Option<String>,
    pub target_id: Option<Uuid>,
}

pub async fn use_object(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UseObjectBody>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Uuid, String, Option<Uuid>) = sqlx::query_as(
        r#"select e.campaign_id, e.id, e.status::text, ch.owner_id
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let (campaign_id, encounter_id, status, owner) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }
    if status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }

    let mut tx = s.db.begin().await?;
    let action_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set action_used = true where id = $1 and action_used = false and hp_current > 0 returning id")
        .bind(id).fetch_optional(&mut *tx).await?;
    if action_consumed.is_none() {
        return Err(AppError::BadRequest("action already used".into()));
    }

    let round: i32 = sqlx::query_scalar("select round from encounters where id = $1")
        .bind(encounter_id).fetch_one(&s.db).await?;

    let label = body.label.unwrap_or_else(|| "Use an Object".to_string());
    sqlx::query(
        "insert into combat_events (encounter_id, round, actor_combatant, target_combatant, action, note) values ($1, $2, $3, $4, $5, $6)")
        .bind(encounter_id).bind(round).bind(id).bind(body.target_id).bind(&label).bind("use_object")
        .execute(&mut *tx).await?;

    tx.commit().await?;

    let c = refresh_combatant(&s.db, id).await?;
    ws::publish(campaign_id, json!({"type":"combatant_uses_object","id":id,"label":label}).to_string());
    Ok(Json(c))
}
