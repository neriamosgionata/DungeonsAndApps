// Combat action route handlers: attack, deal_damage, heal, death_save, skill_check, save, computed_stats.
// Extracted from actions.rs to keep the route handler file under the 500-line
// guideline (per AGENTS.md §1.4). Public re-exports preserve call-site compatibility.
use super::super::tactical::{is_between, is_flanking, segments_intersect};
use super::*;
use crate::AppState;
use crate::error::AppResult;
use crate::extract::AuthUser;
use axum::Json;
use axum::extract::{Path, State};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct AttackBody {
    pub target_id: Uuid,
    pub attack_expression: Option<String>,
    pub damage_expression: Option<String>,
    pub damage_type: String,
    pub damage_die: Option<String>,
    pub ability: Option<String>,
    pub proficient: Option<bool>,
    pub advantage: bool,
    pub disadvantage: bool,
    pub cover: Option<String>,
    pub is_spell_attack: bool,
    pub is_magical: bool,
    pub label: Option<String>,
    pub weapon_id: Option<String>,
    pub extra_damage_expression: Option<String>,
    pub extra_damage_type: Option<String>,
    pub power_attack: Option<bool>,
    pub skip_ammo: Option<bool>,
    pub reckless: Option<bool>,
    pub bless_dice: Option<i32>,
    pub bardic_inspiration_dice: Option<i32>,
}

/// Infer ammo name from weapon name (e.g. "Longbow" → "Arrow")
pub fn infer_ammo_type(weapon_name: &str) -> Option<&'static str> {
    let w = weapon_name.to_lowercase();
    if w.contains("bow") && !w.contains("crossbow") {
        Some("Arrow")
    } else if w.contains("crossbow") {
        Some("Bolt")
    } else if w.contains("musket")
        || w.contains("pistol")
        || w.contains("firearm")
        || w.contains("gun")
        || w.contains("rifle")
    {
        Some("Bullet")
    } else if w.contains("sling") {
        Some("Sling Bullet")
    } else if w.contains("blowgun") {
        Some("Needle")
    } else {
        None
    }
}

/// Decrement thrown weapon quantity in character sheet equipment.
pub async fn decrement_thrown_weapon(
    db: &mut sqlx::PgConnection,
    character_id: Uuid,
    weapon_name: &str,
) -> Result<Option<(String, i32)>, AppError> {
    let sheet_json: Option<serde_json::Value> =
        sqlx::query_scalar("select sheet from characters where id = $1")
            .bind(character_id)
            .fetch_optional(&mut *db)
            .await?;
    let mut sheet = sheet_json.unwrap_or_else(|| serde_json::json!({}));
    let equipment = match sheet.get_mut("equipment").and_then(|v| v.as_array_mut()) {
        Some(arr) => arr,
        None => return Ok(None),
    };
    let wname_lower = weapon_name.to_lowercase();
    let mut found = false;
    let mut remaining = 0;
    for item in equipment.iter_mut() {
        if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
            if name.to_lowercase() == wname_lower || name.to_lowercase().starts_with(&wname_lower) {
                let qty = item.get("qty").and_then(|v| v.as_i64()).unwrap_or(0);
                if qty > 0 {
                    let new_qty = qty - 1;
                    item["qty"] = serde_json::json!(new_qty);
                    remaining = new_qty as i32;
                    found = true;
                    break;
                }
            }
        }
    }
    if found {
        sqlx::query("update characters set sheet = $1 where id = $2")
            .bind(&sheet)
            .bind(character_id)
            .execute(db)
            .await?;
        Ok(Some((weapon_name.to_string(), remaining)))
    } else {
        Ok(None)
    }
}

/// Decrement ammunition in character sheet equipment.
pub async fn decrement_ammo(
    db: &mut sqlx::PgConnection,
    character_id: Uuid,
    weapon_name: &str,
) -> Result<Option<(String, i32)>, AppError> {
    let ammo_type = match infer_ammo_type(weapon_name) {
        Some(a) => a,
        None => return Ok(None),
    };
    let sheet_json: Option<serde_json::Value> =
        sqlx::query_scalar("select sheet from characters where id = $1")
            .bind(character_id)
            .fetch_optional(&mut *db)
            .await?;
    let mut sheet = sheet_json.unwrap_or_else(|| serde_json::json!({}));
    let equipment = match sheet.get_mut("equipment").and_then(|v| v.as_array_mut()) {
        Some(arr) => arr,
        None => {
            return Err(AppError::BadRequest(format!(
                "No {} ammunition remaining for {}",
                ammo_type, weapon_name
            )));
        }
    };
    let mut found = false;
    let mut remaining = 0;
    for item in equipment.iter_mut() {
        if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
            if name.to_lowercase().contains(&ammo_type.to_lowercase()) {
                let qty = item.get("qty").and_then(|v| v.as_i64()).unwrap_or(0);
                if qty > 0 {
                    let new_qty = qty - 1;
                    item["qty"] = serde_json::json!(new_qty);
                    remaining = new_qty as i32;
                    found = true;
                    break;
                }
            }
        }
    }
    if !found {
        return Err(AppError::BadRequest(format!(
            "No {} ammunition remaining for {}",
            ammo_type, weapon_name
        )));
    }
    sqlx::query("update characters set sheet = $1 where id = $2")
        .bind(&sheet)
        .bind(character_id)
        .execute(db)
        .await?;
    Ok(Some((ammo_type.to_string(), remaining)))
}

#[tracing::instrument(skip(s, body), fields(uid = %uid, attacker_id = %id))]
pub async fn attack(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<AttackBody>,
) -> AppResult<Json<combat_engine::AttackResult>> {
    let attacker_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let target_snap = combat_engine::load_snapshot(&s.db, body.target_id).await?;

    if attacker_snap.encounter_id != target_snap.encounter_id {
        return Err(AppError::BadRequest(
            "attacker and target not in same encounter".into(),
        ));
    }

    let campaign_id: Uuid = sqlx::query_scalar("select campaign_id from encounters where id = $1")
        .bind(attacker_snap.encounter_id)
        .fetch_one(&s.db)
        .await?;
    let encounter_status: String =
        sqlx::query_scalar("select status::text as status from encounters where id = $1")
            .bind(attacker_snap.encounter_id)
            .fetch_one(&s.db)
            .await?;
    if encounter_status != "active" {
        return Err(AppError::Conflict("encounter not active".into()));
    }
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if attacker_snap.hp_current <= 0 {
        return Err(AppError::BadRequest("cannot act while at 0 HP".into()));
    }
    let attacker_incap = attacker_snap.conditions.iter().any(|c| {
        matches!(c.to_lowercase().as_str(), s if s.starts_with("incapacitated") || s.starts_with("paralyzed") || s.starts_with("petrified") || s.starts_with("stunned") || s.starts_with("unconscious"))
    });
    if attacker_incap {
        return Err(AppError::BadRequest(
            "cannot act while incapacitated".into(),
        ));
    }

    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    let attacker_stats = combat_engine::compute_stats(&attacker_snap);
    let target_stats = combat_engine::compute_stats(&target_snap);

    let mut adv = body.advantage;
    let mut dis = body.disadvantage;

    let is_reckless = body.reckless.unwrap_or(false);
    if is_reckless {
        let weapon = body
            .weapon_id
            .as_deref()
            .and_then(|wid| combat_engine::find_weapon(&attacker_snap, wid));
        let weapon_props = weapon.as_ref().map(|(_, p)| p.clone()).unwrap_or_default();
        if !weapon_props.ranged && !weapon_props.thrown && !body.is_spell_attack {
            let ab = body.ability.as_deref().unwrap_or("str");
            if ab == "str" {
                adv = true;
            }
        }
    }

    let weapon = body
        .weapon_id
        .as_deref()
        .and_then(|wid| combat_engine::find_weapon(&attacker_snap, wid));
    let weapon_props = weapon.as_ref().map(|(_, p)| p.clone()).unwrap_or_default();

    if weapon_props.ranged || weapon_props.thrown {
        let others: Vec<(Option<f32>, Option<f32>)> = sqlx::query_as(
            "select token_x, token_y from combatants where encounter_id = $1 and id != $2 and initiative_rolled = true")
            .bind(attacker_snap.encounter_id).bind(id).fetch_all(&s.db).await?;
        if let (Some(ax), Some(ay)) = (attacker_snap.token_x, attacker_snap.token_y) {
            let within_5ft = others.iter().any(|(ox, oy)| {
                if let (Some(x), Some(y)) = (ox, oy) {
                    let dx = x - ax;
                    let dy = y - ay;
                    (dx * dx + dy * dy).sqrt() < 1.5
                } else {
                    false
                }
            });
            if within_5ft {
                dis = true;
            }
        }
    }

    let overlays: Vec<(String, Option<f32>, Option<f32>, Option<i32>, Option<i32>)> = sqlx::query_as(
        "select zone_type, origin_x, origin_y, radius_ft, length_ft from encounter_overlays
         where active = true and encounter_id = $1 and zone_type in ('magical_darkness', 'low_visibility', 'no_visibility')")
        .bind(attacker_snap.encounter_id).fetch_all(&s.db).await?;
    if let (Some(ax), Some(ay)) = (attacker_snap.token_x, attacker_snap.token_y) {
        let in_darkness = overlays.iter().any(|(zt, ox, oy, rad, _len)| {
            if let (Some(x), Some(y)) = (ox, oy) {
                let dx = ax - x;
                let dy = ay - y;
                let in_zone = if let Some(r) = rad {
                    (dx * dx + dy * dy).sqrt() < (*r as f32)
                } else {
                    (dx * dx + dy * dy).sqrt() < 5.0
                };
                in_zone
                    && (zt == "magical_darkness"
                        || zt == "no_visibility"
                        || (zt == "low_visibility" && attacker_stats.darkvision_range == 0))
            } else {
                false
            }
        });
        if in_darkness {
            dis = true;
        }
    }

    if (weapon_props.ranged || weapon_props.thrown) && !dis {
        if let (Some((w, _)), Some(tx), Some(ty)) =
            (&weapon, target_snap.token_x, target_snap.token_y)
        {
            if let (Some(ax), Some(ay)) = (attacker_snap.token_x, attacker_snap.token_y) {
                if let Some(range_str) = w.get("range").and_then(|v| v.as_str()) {
                    let parts: Vec<&str> = range_str.split('/').collect();
                    if parts.len() == 2 {
                        if let Ok(normal_range) = parts[0].trim().parse::<f32>() {
                            if let Ok(long_range) =
                                parts[1].trim().trim_end_matches("ft").trim().parse::<f32>()
                            {
                                let g_size: i32 = sqlx::query_scalar(
                                    "select map_grid_size from encounters where id = $1",
                                )
                                .bind(attacker_snap.encounter_id)
                                .fetch_one(&s.db)
                                .await?;
                                let cell_pct = (g_size as f32) / 6.0;
                                let dist_pct = ((ax - tx).powi(2) + (ay - ty).powi(2)).sqrt();
                                let dist_ft = dist_pct / cell_pct * 5.0;
                                if dist_ft > long_range {
                                    return Err(AppError::BadRequest(format!(
                                        "target out of weapon range ({} ft > {} ft max)",
                                        dist_ft as i32, long_range as i32
                                    )));
                                }
                                if dist_ft > normal_range {
                                    dis = true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let auto_cover = if body.cover.is_none() {
        let blockers: Vec<(f32, f32)> = sqlx::query_as(
            r#"select coalesce(token_x, 50), coalesce(token_y, 50)
               from combatants
               where encounter_id = $1 and id not in ($2, $3) and token_on_map = true and hp_current > 0"#,
        )
        .bind(attacker_snap.encounter_id).bind(id).bind(body.target_id)
        .fetch_all(&s.db).await?;
        if let (Some(ax), Some(ay)) = (attacker_snap.token_x, attacker_snap.token_y) {
            if let (Some(tx), Some(ty)) = (target_snap.token_x, target_snap.token_y) {
                let mut max_cover = 0i32;
                for (ox, oy) in &blockers {
                    if is_between(*ox, *oy, ax, ay, tx, ty) {
                        max_cover = (max_cover + 1).min(3);
                    }
                }
                if max_cover > 0 {
                    match max_cover {
                        1 => Some("half"),
                        2 => Some("three_quarters"),
                        _ => Some("full"),
                    }
                    .map(|s| s.to_string())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    if !dis {
        let walls: Vec<(f32, f32, f32, f32)> = sqlx::query_as(
            r#"select origin_x, origin_y,
               coalesce(end_x, origin_x) as end_x,
               coalesce(end_y, origin_y + 5) as end_y
               from encounter_overlays
               where encounter_id = $1 and active = true and zone_type = 'wall' and shape = 'line'"#,
        )
        .bind(attacker_snap.encounter_id)
        .fetch_all(&s.db).await?;
        if !walls.is_empty() {
            if let (Some(ax), Some(ay)) = (attacker_snap.token_x, attacker_snap.token_y) {
                if let (Some(tx), Some(ty)) = (target_snap.token_x, target_snap.token_y) {
                    for (wx1, wy1, wx2, wy2) in &walls {
                        if segments_intersect(ax, ay, tx, ty, *wx1, *wy1, *wx2, *wy2) {
                            return Err(AppError::BadRequest(
                                "attack blocked by wall obstacle".into(),
                            ));
                        }
                    }
                }
            }
        }
    }

    if !adv && !dis {
        let flanking_tokens: Vec<(f32, f32, String)> = sqlx::query_as(
            r#"select coalesce(token_x, 50), coalesce(token_y, 50),
               case when ref_type = 'character' then 'ally' else 'enemy' end as side
               from combatants
               where encounter_id = $1 and token_on_map = true and hp_current > 0 and id != $2 and id != $3"#,
        )
        .bind(attacker_snap.encounter_id).bind(id).bind(body.target_id)
        .fetch_all(&s.db).await?;
        if let (Some(ax), Some(ay)) = (attacker_snap.token_x, attacker_snap.token_y) {
            if let (Some(tx), Some(ty)) = (target_snap.token_x, target_snap.token_y) {
                let attacker_side = if attacker_snap.character_id.is_some() {
                    "ally"
                } else {
                    "enemy"
                };
                let grid_size: i32 =
                    sqlx::query_scalar("select map_grid_size from encounters where id = $1")
                        .bind(attacker_snap.encounter_id)
                        .fetch_one(&s.db)
                        .await?;
                for other in &flanking_tokens {
                    if other.2 == attacker_side {
                        if is_flanking(ax, ay, other.0, other.1, tx, ty, grid_size) {
                            adv = true;
                            break;
                        }
                    }
                }
            }
        }
    }

    let cover = auto_cover
        .as_deref()
        .or(body.cover.as_deref())
        .unwrap_or("none")
        .to_string();

    let req = combat_engine::AttackReq {
        target_id: body.target_id,
        attack_expression: body.attack_expression,
        damage_expression: body.damage_expression,
        damage_type: body.damage_type,
        damage_die: body.damage_die,
        ability: body.ability,
        proficient: body.proficient,
        advantage: adv,
        disadvantage: dis,
        cover: Some(cover.clone()),
        is_spell_attack: body.is_spell_attack,
        is_magical: body.is_magical,
        label: body.label,
        weapon_id: body.weapon_id,
        extra_damage_expression: body.extra_damage_expression,
        extra_damage_type: body.extra_damage_type,
        power_attack: body.power_attack.unwrap_or(false),
        reckless: is_reckless,
        bless_dice: body.bless_dice,
        bardic_inspiration_dice: body.bardic_inspiration_dice,
    };

    let result = combat_engine::resolve_attack(
        &attacker_snap,
        &target_snap,
        &req,
        &attacker_stats,
        &target_stats,
    )
    .map_err(|e| AppError::BadRequest(e))?;

    let round: i32 = sqlx::query_scalar("select round from encounters where id = $1")
        .bind(attacker_snap.encounter_id)
        .fetch_one(&s.db)
        .await?;

    let mut tx = s.db.begin().await?;

    let action_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set action_used = true where id = $1 and action_used = false and hp_current > 0 returning id")
        .bind(id).fetch_optional(&mut *tx).await?;
    if action_consumed.is_none() {
        return Err(AppError::BadRequest("action already used".into()));
    }

    let ammo_info: Option<(String, i32)> = if body.skip_ammo.unwrap_or(false) {
        None
    } else if let Some((w, _)) = &weapon {
        let wname = w.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let props = w.get("properties").and_then(|v| v.as_str()).unwrap_or("");
        if props.to_lowercase().contains("ammunition") || props.to_lowercase().contains("ammo") {
            if let Some(chid) = attacker_snap.character_id {
                decrement_ammo(&mut *tx, chid, wname).await?
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };
    let thrown_info: Option<(String, i32)> = if body.skip_ammo.unwrap_or(false) {
        None
    } else if let Some((w, _)) = &weapon {
        let wname = w.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let props = w.get("properties").and_then(|v| v.as_str()).unwrap_or("");
        if props.to_lowercase().contains("thrown") {
            if let Some(chid) = attacker_snap.character_id {
                decrement_thrown_weapon(&mut *tx, chid, wname).await?
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    if result.hit {
        sqlx::query(
            "update combatants set
                last_hit_attack_total = $1,
                last_hit_damage = $2,
                pending_hits = pending_hits || jsonb_build_array(jsonb_build_object(
                    'attacker_id', $3,
                    'attack_total', $1,
                    'damage', $2,
                    'round', $5
                ))
             where id = $4",
        )
        .bind(result.attack_total)
        .bind(result.damage_applied + result.extra_damage_applied)
        .bind(id)
        .bind(body.target_id)
        .bind(round)
        .execute(&mut *tx)
        .await?;

        sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
            .bind(result.target_hp_after)
            .bind(result.target_temp_hp_after)
            .bind(body.target_id)
            .execute(&mut *tx)
            .await?;

        if result.concentration_broken {
            sqlx::query("update combatant_effects set active = false where combatant_id = $1 and concentration = true and active = true")
                .bind(body.target_id)
                .execute(&mut *tx).await?;
        }

        if result.instant_death {
            if let Some(chid) = target_snap.character_id {
                sqlx::query(
                    r#"update characters set sheet = coalesce(sheet,'{}'::jsonb)
                       || jsonb_build_object('alive', false,
                            'death_saves', jsonb_build_object('successes', 0, 'failures', 3))
                       where id = $1"#,
                )
                .bind(chid)
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    if is_reckless {
        sqlx::query(
            r#"insert into combatant_effects
               (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
                concentration, active, modifiers, source_type)
               values ($1, 'Reckless Attack', 'debuff', 'swords', 'rounds', 1, 1, 'caster_turn_start',
                       false, true, '{"attack_advantage_against": true}', 'ability')"#)
            .bind(id)
            .execute(&mut *tx).await?;
    }

    sqlx::query(
        "update combatant_effects set active = false
         where combatant_id = $1 and active = true
           and modifiers->>'hidden' = 'true'",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;

    let total_dmg = result.damage_applied + result.extra_damage_applied;
    let event_action = if result.hit {
        let death_note = if result.instant_death {
            " — INSTANT DEATH"
        } else {
            ""
        };
        format!(
            "{} attacked {}: {} damage{}",
            attacker_snap.display_name, target_snap.display_name, total_dmg, death_note
        )
    } else {
        format!(
            "{} attacked {}: missed ({} vs AC {})",
            attacker_snap.display_name,
            target_snap.display_name,
            result.attack_total,
            result.target_ac
        )
    };
    sqlx::query(
        "insert into combat_events (encounter_id, round, actor_combatant, target_combatant, action, delta_hp, note) values ($1, $2, $3, $4, $5, $6, $7)")
        .bind(attacker_snap.encounter_id)
        .bind(round)
        .bind(id)
        .bind(body.target_id)
        .bind(&event_action)
        .bind(if result.hit { -total_dmg } else { 0 })
        .bind(req.label.as_deref())
        .execute(&mut *tx).await?;

    tx.commit().await?;

    if result.hit {
        if let Err(e) = sync_combatant_hp_to_sheet(
            &s.db,
            body.target_id,
            result.target_hp_after,
            result.target_temp_hp_after,
        )
        .await
        {
            tracing::error!(combatant_id = %body.target_id, "sync sheet HP: {e}");
        }
        let total_dmg = result.damage_applied + result.extra_damage_applied;
        ws::publish(
            campaign_id,
            json!({
                "type": "reaction_window",
                "window_type": "hit_before_damage",
                "target_id": body.target_id,
                "attacker_id": id,
                "attack_total": result.attack_total,
                "target_ac": result.target_ac,
                "damage_pending": total_dmg,
            })
            .to_string(),
        );
        auto_trigger_ready_actions_for_event(
            &s.db,
            campaign_id,
            attacker_snap.encounter_id,
            "target_attacks",
            id,
            body.target_id,
        )
        .await;
    }

    ws::publish(campaign_id, json!({
        "type": "combatant_attacks",
        "attacker_id": id,
        "target_id": body.target_id,
        "hit": result.hit,
        "critical": result.critical,
        "damage": if result.hit { Some(result.damage_applied) } else { None },
        "extra_damage": if result.hit && result.extra_damage_applied > 0 { Some(result.extra_damage_applied) } else { None },
        "extra_damage_type": result.extra_damage_type.as_deref(),
        "hp_after": if result.hit { Some(result.target_hp_after) } else { None },
        "temp_hp_after": if result.hit { Some(result.target_temp_hp_after) } else { None },
        "concentration_breaks": if result.hit { Some(result.concentration_broken) } else { None },
        "instant_death": if result.hit { Some(result.instant_death) } else { None },
        "attack_total": if !result.hit { Some(result.attack_total) } else { None },
        "target_ac": result.target_ac,
        "ammo_consumed": ammo_info.as_ref().map(|(n, q)| serde_json::json!({"type": n, "remaining": q})),
        "thrown_consumed": thrown_info.as_ref().map(|(n, q)| serde_json::json!({"type": n, "remaining": q})),
    }).to_string());

    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
pub struct DamageBody {
    pub amount: i32,
    pub damage_type: String,
    pub source_combatant_id: Option<Uuid>,
    pub label: Option<String>,
    pub is_magical: bool,
}

pub async fn deal_damage(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<DamageBody>,
) -> AppResult<Json<combat_engine::DamageResult>> {
    let target_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let campaign_id: Uuid = sqlx::query_scalar("select campaign_id from encounters where id = $1")
        .bind(target_snap.encounter_id)
        .fetch_one(&s.db)
        .await?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        let source_owner: Option<Uuid> = if let Some(sid) = body.source_combatant_id {
            sqlx::query_scalar(
                "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
                .bind(sid).fetch_optional(&s.db).await?
        } else {
            None
        };
        if owner != Some(uid) && source_owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    let round: i32 = sqlx::query_scalar("select round from encounters where id = $1")
        .bind(target_snap.encounter_id)
        .fetch_one(&s.db)
        .await?;

    let target_stats = combat_engine::compute_stats(&target_snap);
    let req = combat_engine::DamageReq {
        amount: body.amount,
        damage_type: body.damage_type,
        source_combatant_id: body.source_combatant_id,
        label: body.label,
        is_magical: body.is_magical,
    };
    let result = combat_engine::resolve_damage(&target_snap, &req, &target_stats)
        .map_err(|e| AppError::BadRequest(e))?;

    let mut tx = s.db.begin().await?;
    sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
        .bind(result.hp_after)
        .bind(result.temp_hp_after)
        .bind(id)
        .execute(&mut *tx)
        .await?;

    if result.concentration_broken {
        sqlx::query("update combatant_effects set active = false where combatant_id = $1 and concentration = true and active = true")
            .bind(id).execute(&mut *tx).await?;
    }

    if result.instant_death {
        if let Some(chid) = target_snap.character_id {
            sqlx::query(
                r#"update characters set sheet = coalesce(sheet,'{}'::jsonb)
                   || jsonb_build_object('alive', false,
                        'death_saves', jsonb_build_object('successes', 0, 'failures', 3))
                   where id = $1"#,
            )
            .bind(chid)
            .execute(&mut *tx)
            .await?;
        }
    }

    let source_name = if let Some(sid) = body.source_combatant_id {
        sqlx::query_scalar::<_, String>("select display_name from combatants where id = $1")
            .bind(sid)
            .fetch_optional(&s.db)
            .await?
            .unwrap_or_else(|| "Unknown".into())
    } else {
        "DM".into()
    };

    sqlx::query(
        "insert into combat_events (encounter_id, round, actor_combatant, target_combatant, action, delta_hp, note) values ($1, $2, $3, $4, $5, $6, $7)")
        .bind(target_snap.encounter_id)
        .bind(round)
        .bind(body.source_combatant_id)
        .bind(id)
        .bind(format!("{} dealt {} {} damage to {}{}", source_name, result.damage_applied, req.damage_type, target_snap.display_name, if result.instant_death { " — INSTANT DEATH" } else { "" }))
        .bind(-result.damage_applied)
        .bind(req.label.as_deref())
        .execute(&mut *tx).await?;

    tx.commit().await?;

    if let Err(e) =
        sync_combatant_hp_to_sheet(&s.db, id, result.hp_after, result.temp_hp_after).await
    {
        tracing::error!(combatant_id = %id, "sync sheet HP: {e}");
    }

    ws::publish(
        campaign_id,
        json!({
            "type": "combatant_damages",
            "target_id": id,
            "damage": result.damage_applied,
            "hp_after": result.hp_after,
            "temp_hp_after": result.temp_hp_after,
            "concentration_breaks": result.concentration_broken,
            "instant_death": result.instant_death,
        })
        .to_string(),
    );

    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
pub struct HealBody {
    pub amount: i32,
    pub source_combatant_id: Option<Uuid>,
    pub label: Option<String>,
}

pub async fn heal(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<HealBody>,
) -> AppResult<Json<combat_engine::HealResult>> {
    let target_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let campaign_id: Uuid = sqlx::query_scalar("select campaign_id from encounters where id = $1")
        .bind(target_snap.encounter_id)
        .fetch_one(&s.db)
        .await?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
        if let Some(sid) = body.source_combatant_id {
            let source_owner: Option<Uuid> = sqlx::query_scalar(
                "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
                .bind(sid).fetch_optional(&s.db).await?;
            if source_owner != Some(uid) {
                return Err(AppError::Forbidden);
            }
            let factions: (String, String, String, String) = sqlx::query_as(
                r#"select s.faction, s.ref_type::text, t.faction, t.ref_type::text
                   from combatants s, combatants t
                   where s.id = $1 and t.id = $2"#)
                .bind(sid).bind(id).fetch_one(&s.db).await?;
            let derived = |f: &str, r: &str| -> String {
                if f != "auto" { f.to_string() } else if r == "character" { "ally".to_string() } else { "enemy".to_string() }
            };
            if derived(&factions.0, &factions.1) != derived(&factions.2, &factions.3) {
                return Err(AppError::Forbidden);
            }
        }
    }

    let req = combat_engine::HealReq {
        amount: body.amount,
        source_combatant_id: body.source_combatant_id,
        label: body.label,
    };
    let result = combat_engine::resolve_heal(&target_snap, &req);
    let reviving_from_zero = target_snap.hp_current <= 0 && result.hp_after > 0;

    let mut tx = s.db.begin().await?;
    sqlx::query("update combatants set hp_current = $1 where id = $2")
        .bind(result.hp_after)
        .bind(id)
        .execute(&mut *tx)
        .await?;

    if reviving_from_zero {
        if let Some(chid) = target_snap.character_id {
            sqlx::query(
                r#"update characters set sheet = coalesce(sheet,'{}'::jsonb)
                   || jsonb_build_object('alive', true,
                        'death_saves', jsonb_build_object('successes', 0, 'failures', 0))
                   where id = $1"#,
            )
            .bind(chid)
            .execute(&mut *tx)
            .await?;
        }
    }

    let round: i32 = sqlx::query_scalar("select round from encounters where id = $1")
        .bind(target_snap.encounter_id)
        .fetch_one(&s.db)
        .await?;

    let source_name = if let Some(sid) = body.source_combatant_id {
        sqlx::query_scalar::<_, String>("select display_name from combatants where id = $1")
            .bind(sid)
            .fetch_optional(&s.db)
            .await?
            .unwrap_or_else(|| "Unknown".into())
    } else {
        "DM".into()
    };

    sqlx::query(
        "insert into combat_events (encounter_id, round, actor_combatant, target_combatant, action, delta_hp, note) values ($1, $2, $3, $4, $5, $6, $7)")
        .bind(target_snap.encounter_id)
        .bind(round)
        .bind(body.source_combatant_id)
        .bind(id)
        .bind(format!("{} healed {} for {} HP", source_name, target_snap.display_name, result.amount))
        .bind(result.amount)
        .bind(req.label.as_deref())
        .execute(&mut *tx).await?;

    tx.commit().await?;

    if let Err(e) =
        sync_combatant_hp_to_sheet(&s.db, id, result.hp_after, target_snap.temp_hp).await
    {
        tracing::error!(combatant_id = %id, "sync sheet HP: {e}");
    }

    ws::publish(
        campaign_id,
        json!({
            "type": "combatant_heals",
            "target_id": id,
            "amount": result.amount,
            "hp_after": result.hp_after,
            "stabilized": result.stabilized,
            "revived": reviving_from_zero,
        })
        .to_string(),
    );

    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
pub struct DeathSaveBody {
    pub advantage: bool,
    pub disadvantage: bool,
    pub label: Option<String>,
}

pub async fn death_save(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<DeathSaveBody>,
) -> AppResult<Json<combat_engine::DeathSaveResult>> {
    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    let campaign_id: Uuid = sqlx::query_scalar("select campaign_id from encounters where id = $1")
        .bind(snap.encounter_id)
        .fetch_one(&s.db)
        .await?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    if snap.hp_current > 0 {
        return Err(AppError::BadRequest("character is not dying".into()));
    }

    let req = combat_engine::DeathSaveReq {
        advantage: body.advantage,
        disadvantage: body.disadvantage,
        label: body.label,
    };
    let result =
        combat_engine::resolve_death_save(&snap, &req).map_err(|e| AppError::BadRequest(e))?;

    let mut tx = s.db.begin().await?;
    sqlx::query("update combatants set hp_current = $1 where id = $2")
        .bind(result.hp_after)
        .bind(id)
        .execute(&mut *tx)
        .await?;

    if let Some(chid) = snap.character_id {
        sqlx::query(
            r#"update characters set sheet =
                 coalesce(sheet, '{}'::jsonb)
                 || jsonb_build_object(
                      'death_saves', jsonb_build_object('successes', $2::int, 'failures', $3::int),
                      'alive', $4::bool
                    )
               where id = $1"#,
        )
        .bind(chid)
        .bind(result.successes_after)
        .bind(result.failures_after)
        .bind(result.alive)
        .execute(&mut *tx)
        .await?;
    }

    let round: i32 = sqlx::query_scalar("select round from encounters where id = $1")
        .bind(snap.encounter_id)
        .fetch_one(&s.db)
        .await?;

    let action_str = if result.nat20 {
        "Death Save: NAT 20 — regains 1 HP".to_string()
    } else if result.nat1 {
        format!("Death Save: NAT 1 — {} failures", result.failures_after)
    } else if result.passed {
        format!("Death Save: success ({}/3)", result.successes_after)
    } else {
        format!("Death Save: failure ({}/3)", result.failures_after)
    };

    sqlx::query(
        "insert into combat_events (encounter_id, round, actor_combatant, target_combatant, action, delta_hp, note) values ($1, $2, $3, $4, $5, $6, $7)")
        .bind(snap.encounter_id)
        .bind(round)
        .bind(id)
        .bind(id)
        .bind(&action_str)
        .bind(if result.hp_after > 0 { result.hp_after } else { 0 })
        .bind(req.label.as_deref())
        .execute(&mut *tx).await?;

    tx.commit().await?;

    if let Err(e) = sync_combatant_hp_to_sheet(&s.db, id, result.hp_after, snap.temp_hp).await {
        tracing::error!(combatant_id = %id, "sync sheet HP: {e}");
    }

    ws::publish(
        campaign_id,
        json!({
            "type": "combatant_death_saves",
            "combatant_id": id,
            "natural_roll": result.natural_roll,
            "passed": result.passed,
            "successes": result.successes_after,
            "failures": result.failures_after,
            "stabilized": result.stabilized,
            "died": result.died,
            "hp_after": result.hp_after,
            "alive": result.alive,
        })
        .to_string(),
    );

    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
pub struct SkillCheckBody {
    pub skill: String,
    pub dc: Option<i32>,
    pub advantage: bool,
    pub disadvantage: bool,
    pub label: Option<String>,
}

pub async fn skill_check(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SkillCheckBody>,
) -> AppResult<Json<combat_engine::SkillCheckResult>> {
    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    let campaign_id: Uuid = sqlx::query_scalar("select campaign_id from encounters where id = $1")
        .bind(snap.encounter_id)
        .fetch_one(&s.db)
        .await?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    let stats = combat_engine::compute_stats(&snap);
    let req = combat_engine::SkillCheckReq {
        skill: body.skill,
        dc: body.dc,
        advantage: body.advantage,
        disadvantage: body.disadvantage,
        label: body.label,
    };
    let result = combat_engine::resolve_skill_check(&snap, &req, &stats)
        .map_err(|e| AppError::BadRequest(e))?;

    ws::publish(
        campaign_id,
        json!({
            "type": "combatant_skill_checks",
            "combatant_id": id,
            "skill": result.skill,
            "total": result.total,
            "dc": result.dc,
            "passed": result.passed,
        })
        .to_string(),
    );

    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
pub struct SaveBody {
    pub ability: String,
    pub dc: i32,
    pub advantage: bool,
    pub disadvantage: bool,
    pub label: Option<String>,
    pub is_magical: Option<bool>,
}

pub async fn roll_save(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SaveBody>,
) -> AppResult<Json<combat_engine::SaveResult>> {
    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    let campaign_id: Uuid = sqlx::query_scalar("select campaign_id from encounters where id = $1")
        .bind(snap.encounter_id)
        .fetch_one(&s.db)
        .await?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    let stats = combat_engine::compute_stats(&snap);
    let req = combat_engine::SaveReq {
        ability: body.ability,
        dc: body.dc,
        advantage: body.advantage,
        disadvantage: body.disadvantage,
        label: body.label,
        is_magical: body.is_magical,
    };
    let result =
        combat_engine::resolve_save(&snap, &req, &stats).map_err(|e| AppError::BadRequest(e))?;

    ws::publish(
        campaign_id,
        json!({
            "type": "combatant_save",
            "combatant_id": id,
            "passed": result.passed,
            "save_total": result.save_total,
            "dc": result.dc,
            "natural_roll": result.natural_roll,
        })
        .to_string(),
    );

    Ok(Json(result))
}

pub async fn computed_stats(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<combat_engine::ComputedStats>> {
    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    let campaign_id: Uuid = sqlx::query_scalar("select campaign_id from encounters where id = $1")
        .bind(snap.encounter_id)
        .fetch_one(&s.db)
        .await?;
    rbac::require_member(&s.db, uid, campaign_id).await?;
    let stats = combat_engine::compute_stats(&snap);
    Ok(Json(stats))
}
