// Route handlers and helpers for combat actions.
use super::*;

#[derive(Debug, Deserialize)]
pub struct AttackBody {
    target_id: Uuid,
    attack_expression: Option<String>,
    damage_expression: Option<String>,
    damage_type: String,
    damage_die: Option<String>,
    ability: Option<String>,
    proficient: Option<bool>,
    advantage: bool,
    disadvantage: bool,
    cover: Option<String>,
    is_spell_attack: bool,
    is_magical: bool,
    label: Option<String>,
    weapon_id: Option<String>,
    extra_damage_expression: Option<String>,
    extra_damage_type: Option<String>,
    power_attack: Option<bool>,
    skip_ammo: Option<bool>,
    reckless: Option<bool>,
    bless_dice: Option<i32>,
    bardic_inspiration_dice: Option<i32>,
}

/// Infer ammo name from weapon name (e.g. "Longbow" → "Arrow")
pub fn infer_ammo_type(weapon_name: &str) -> Option<&'static str> {
    let w = weapon_name.to_lowercase();
    if w.contains("bow") && !w.contains("crossbow") {
        Some("Arrow")
    } else if w.contains("crossbow") {
        Some("Bolt")
    } else if w.contains("musket") || w.contains("pistol") || w.contains("firearm") || w.contains("gun") || w.contains("rifle") {
        Some("Bullet")
    } else if w.contains("sling") {
        Some("Sling Bullet")
    } else if w.contains("blowgun") {
        Some("Needle")
    } else {
        None
    }
}

/// Decrement thrown weapon quantity in character sheet equipment. Returns (name, remaining_qty) or None.
pub async fn decrement_thrown_weapon(
    db: &mut sqlx::PgConnection,
    character_id: Uuid,
    weapon_name: &str,
) -> Result<Option<(String, i32)>, crate::error::AppError> {
    let sheet_json: Option<serde_json::Value> = sqlx::query_scalar(
        "select sheet from characters where id = $1")
        .bind(character_id).fetch_optional(&mut *db).await?;
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
            .bind(&sheet).bind(character_id).execute(db).await?;
        Ok(Some((weapon_name.to_string(), remaining)))
    } else {
        Ok(None) // not tracked or already 0 — don't throw error, just silently return
    }
}

/// Decrement ammunition in character sheet equipment. Returns (ammo_name, remaining_qty) or None.
pub async fn decrement_ammo(
    db: &mut sqlx::PgConnection,
    character_id: Uuid,
    weapon_name: &str,
) -> Result<Option<(String, i32)>, crate::error::AppError> {
    let ammo_type = match infer_ammo_type(weapon_name) {
        Some(a) => a,
        None => return Ok(None),
    };

    // Find matching ammo in equipment
    let sheet_json: Option<serde_json::Value> = sqlx::query_scalar(
        "select sheet from characters where id = $1")
        .bind(character_id).fetch_optional(&mut *db).await?;

    let mut sheet = sheet_json.unwrap_or_else(|| serde_json::json!({}));
    let equipment = match sheet.get_mut("equipment").and_then(|v| v.as_array_mut()) {
        Some(arr) => arr,
        None => {
            return Err(crate::error::AppError::BadRequest(
                format!("No {} ammunition remaining for {}", ammo_type, weapon_name)
            ));
        }
    };

    // Find ammo item by fuzzy name match
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
        return Err(crate::error::AppError::BadRequest(
            format!("No {} ammunition remaining for {}", ammo_type, weapon_name)
        ));
    }

    sqlx::query("update characters set sheet = $1 where id = $2")
        .bind(&sheet)
        .bind(character_id)
        .execute(db).await?;

    Ok(Some((ammo_type.to_string(), remaining)))
}

pub async fn attack(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<AttackBody>,
) -> AppResult<Json<combat_engine::AttackResult>> {
    let attacker_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let target_snap = combat_engine::load_snapshot(&s.db, body.target_id).await?;

    if attacker_snap.encounter_id != target_snap.encounter_id {
        return Err(AppError::BadRequest("attacker and target not in same encounter".into()));
    }

    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(attacker_snap.encounter_id).fetch_one(&s.db).await?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    // Authorize: master can use anyone; players only their own character
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

    // Reckless Attack: attacker gains advantage, but counter-effect gives enemies advantage
    let is_reckless = body.reckless.unwrap_or(false);
    if is_reckless {
        adv = true;
    }

    // Look up weapon for property checks
    let weapon = body.weapon_id.as_deref().and_then(|wid| combat_engine::find_weapon(&attacker_snap, wid));
    let weapon_props = weapon.as_ref().map(|(_, p)| p.clone()).unwrap_or_default();

    // Ranged attack within 5ft of another combatant = disadvantage
    if weapon_props.ranged || weapon_props.thrown {
        let others: Vec<(Option<f32>, Option<f32>)> = sqlx::query_as(
            "select token_x, token_y from combatants where encounter_id = $1 and id != $2 and initiative_rolled = true")
            .bind(attacker_snap.encounter_id).bind(id).fetch_all(&s.db).await?;
        if let (Some(ax), Some(ay)) = (attacker_snap.token_x, attacker_snap.token_y) {
            let within_5ft = others.iter().any(|(ox, oy)| {
                if let (Some(x), Some(y)) = (ox, oy) {
                    let dx = x - ax;
                    let dy = y - ay;
                    (dx*dx + dy*dy).sqrt() < 1.5 // ~5ft in map percent
                } else { false }
            });
            if within_5ft {
                dis = true;
            }
        }
    }

    // Visibility check: attacker in magical darkness / low visibility without darkvision
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
                    // Approximate: 5ft = 1 grid cell. Map is ~100% = encounter map size.
                    // Use a rough conversion: 30ft radius ≈ 30% of map
                    (dx*dx + dy*dy).sqrt() < (*r as f32)
                } else {
                    // Default small zone
                    (dx*dx + dy*dy).sqrt() < 5.0
                };
                in_zone && (zt == "magical_darkness" || zt == "no_visibility" || (zt == "low_visibility" && attacker_stats.darkvision_range == 0))
            } else { false }
        });
        if in_darkness {
            dis = true;
        }
    }

    // Long-range disadvantage: if ranged/thrown weapon target is beyond normal range
    if (weapon_props.ranged || weapon_props.thrown) && !dis {
        if let (Some((w, _)), Some(tx), Some(ty)) = (&weapon, target_snap.token_x, target_snap.token_y) {
            if let (Some(ax), Some(ay)) = (attacker_snap.token_x, attacker_snap.token_y) {
                if let Some(range_str) = w.get("range").and_then(|v| v.as_str()) {
                    // Parse "normal/long ft" e.g. "60/120 ft" or "20/60 ft"
                    let parts: Vec<&str> = range_str.split('/').collect();
                    if parts.len() == 2 {
                        if let Ok(normal_range) = parts[0].trim().parse::<f32>() {
                            if let Ok(long_range) = parts[1].trim().trim_end_matches("ft").trim().parse::<f32>() {
                                // Map percent ≈ rough ft: 100% ≈ encounter dimension
                                let g_size: i32 = sqlx::query_scalar("select map_grid_size from encounters where id = $1")
                                    .bind(attacker_snap.encounter_id).fetch_one(&s.db).await?;
                                let cell_pct = (g_size as f32) / 6.0; // ~px per grid cell
                                let dist_pct = ((ax - tx).powi(2) + (ay - ty).powi(2)).sqrt();
                                let dist_ft = dist_pct / cell_pct * 5.0; // each cell = 5ft
                                if dist_ft > long_range {
                                    return Err(AppError::BadRequest(format!(
                                        "target out of weapon range ({} ft > {} ft max)", dist_ft as i32, long_range as i32
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

    // Auto-cover: if no explicit cover provided, check token positions for cover
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
                    }.map(|s| s.to_string())
                } else { None }
            } else { None }
        } else { None }
    } else { None };

    // Wall LOS check: wall overlay between attacker and target blocks the attack
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
                                "attack blocked by wall obstacle".into()
                            ));
                        }
                    }
                }
            }
        }
    }

    // Flanking check: if attacker + any ally flank the target, attacker has advantage
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
                let attacker_side = if attacker_snap.character_id.is_some() { "ally" } else { "enemy" };
                let grid_size: i32 = sqlx::query_scalar("select map_grid_size from encounters where id = $1")
                    .bind(attacker_snap.encounter_id).fetch_one(&s.db).await?;
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

    let cover = auto_cover.as_deref().or(body.cover.as_deref()).unwrap_or("none").to_string();

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

    let result = combat_engine::resolve_attack(&attacker_snap, &target_snap, &req, &attacker_stats, &target_stats)
        .map_err(|e| AppError::BadRequest(e))?;

    let round: i32 = sqlx::query_scalar("select round from encounters where id = $1")
        .bind(attacker_snap.encounter_id).fetch_one(&s.db).await?;

    // Decrement ammunition if weapon uses ammo (check before committing attack)
    let mut tx = s.db.begin().await?;

    // Atomic action consumption first
    let action_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set action_used = true where id = $1 and action_used = false returning id")
        .bind(id).fetch_optional(&mut *tx).await?;
    if action_consumed.is_none() {
        return Err(AppError::BadRequest("action already used".into()));
    }

    // Decrement ammunition if weapon uses ammo (inside transaction), unless skip_ammo is set
    let ammo_info: Option<(String, i32)> = if body.skip_ammo.unwrap_or(false) {
        None
    } else if let Some((w, _)) = &weapon {
        let wname = w.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let props = w.get("properties").and_then(|v| v.as_str()).unwrap_or("");
        if props.to_lowercase().contains("ammunition") || props.to_lowercase().contains("ammo") {
            if let Some(chid) = attacker_snap.character_id {
                decrement_ammo(&mut *tx, chid, wname).await?
            } else { None }
        } else { None }
    } else { None };
    // Decrement thrown weapon quantity if applicable
    let thrown_info: Option<(String, i32)> = if body.skip_ammo.unwrap_or(false) {
        None
    } else if let Some((w, _)) = &weapon {
        let wname = w.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let props = w.get("properties").and_then(|v| v.as_str()).unwrap_or("");
        if props.to_lowercase().contains("thrown") {
            if let Some(chid) = attacker_snap.character_id {
                decrement_thrown_weapon(&mut *tx, chid, wname).await?
            } else { None }
        } else { None }
    } else { None };

    // Apply damage to DB if hit
    if result.hit {
        // Record last hit on target for Shield reaction window
        sqlx::query(
            "update combatants set last_hit_attack_total = $1, last_hit_damage = $2, last_hit_attacker = $3 where id = $4")
            .bind(result.attack_total)
            .bind(result.damage_applied + result.extra_damage_applied)
            .bind(id)
            .bind(body.target_id)
            .execute(&mut *tx).await?;

        // Update target HP
        sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
            .bind(result.target_hp_after)
            .bind(result.target_temp_hp_after)
            .bind(body.target_id)
            .execute(&mut *tx).await?;

        // Break concentration if needed
        if result.concentration_broken {
            sqlx::query("update combatant_effects set active = false where combatant_id = $1 and concentration = true and active = true")
                .bind(body.target_id)
                .execute(&mut *tx).await?;
        }

        // Massive damage: instant death
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

    // Apply Reckless Attack counter-effect (enemies have advantage against attacker)
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

    // Reveal hidden attacker regardless of hit/miss (PHB: attacking ends hidden status)
    sqlx::query(
        "update combatant_effects set active = false
         where combatant_id = $1 and active = true
           and modifiers->>'hidden' = 'true'")
        .bind(id).execute(&mut *tx).await?;

    // Log combat event
    let total_dmg = result.damage_applied + result.extra_damage_applied;
    let event_action = if result.hit {
        let death_note = if result.instant_death { " — INSTANT DEATH" } else { "" };
        format!("{} attacked {}: {} damage{}", attacker_snap.display_name, target_snap.display_name, total_dmg, death_note)
    } else {
        format!("{} attacked {}: missed ({} vs AC {})", attacker_snap.display_name, target_snap.display_name, result.attack_total, result.target_ac)
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
        if let Err(e) = sync_combatant_hp_to_sheet(&s.db, body.target_id, result.target_hp_after, result.target_temp_hp_after).await {
            tracing::warn!("sync sheet HP: {e}");
        }
        // Notify target they can react with Shield (reaction window)
        let total_dmg = result.damage_applied + result.extra_damage_applied;
        ws::publish(campaign_id, json!({
            "type": "reaction_window",
            "window_type": "hit_before_damage",
            "target_id": body.target_id,
            "attacker_id": id,
            "attack_total": result.attack_total,
            "target_ac": result.target_ac,
            "damage_pending": total_dmg,
        }).to_string());

        // Auto-trigger ready actions for combatants watching for "target_attacks" trigger
        auto_trigger_ready_actions_for_event(&s.db, campaign_id, attacker_snap.encounter_id,
            "target_attacks", id, body.target_id).await;
    }

    // Broadcast
    ws::publish(campaign_id, json!({
        "type": "combatant_attacked",
        "attacker_id": id,
        "target_id": body.target_id,
        "hit": result.hit,
        "critical": result.critical,
        "damage": if result.hit { Some(result.damage_applied) } else { None },
        "extra_damage": if result.hit && result.extra_damage_applied > 0 { Some(result.extra_damage_applied) } else { None },
        "extra_damage_type": result.extra_damage_type.as_deref(),
        "hp_after": if result.hit { Some(result.target_hp_after) } else { None },
        "temp_hp_after": if result.hit { Some(result.target_temp_hp_after) } else { None },
        "concentration_broken": if result.hit { Some(result.concentration_broken) } else { None },
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
    amount: i32,
    damage_type: String,
    source_combatant_id: Option<Uuid>,
    label: Option<String>,
    is_magical: bool,
}

pub async fn deal_damage(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<DamageBody>,
) -> AppResult<Json<combat_engine::DamageResult>> {
    let target_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(target_snap.encounter_id).fetch_one(&s.db).await?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        // Players can only damage their own character, or if source is their character
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        let source_owner: Option<Uuid> = if let Some(sid) = body.source_combatant_id {
            sqlx::query_scalar(
                "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
                .bind(sid).fetch_optional(&s.db).await?
        } else { None };
        if owner != Some(uid) && source_owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    let round: i32 = sqlx::query_scalar("select round from encounters where id = $1")
        .bind(target_snap.encounter_id).fetch_one(&s.db).await?;

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
        .execute(&mut *tx).await?;

    if result.concentration_broken {
        sqlx::query("update combatant_effects set active = false where combatant_id = $1 and concentration = true and active = true")
            .bind(id)
            .execute(&mut *tx).await?;
    }

    // Massive damage: instant death — write to character sheet
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

    let source_name = if let Some(sid) = body.source_combatant_id {
        sqlx::query_scalar::<_, String>("select display_name from combatants where id = $1")
            .bind(sid).fetch_optional(&s.db).await?.unwrap_or_else(|| "Unknown".into())
    } else { "DM".into() };

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

    if let Err(e) = sync_combatant_hp_to_sheet(&s.db, id, result.hp_after, result.temp_hp_after).await {
        tracing::warn!("sync sheet HP: {e}");
    }

    ws::publish(campaign_id, json!({
        "type": "combatant_damaged",
        "target_id": id,
        "damage": result.damage_applied,
        "hp_after": result.hp_after,
        "temp_hp_after": result.temp_hp_after,
        "concentration_broken": result.concentration_broken,
        "instant_death": result.instant_death,
    }).to_string());

    Ok(Json(result))
}

// =====================================================================
// Heal
// =====================================================================

#[derive(Debug, Deserialize)]
pub struct HealBody {
    amount: i32,
    source_combatant_id: Option<Uuid>,
    label: Option<String>,
}

pub async fn heal(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<HealBody>,
) -> AppResult<Json<combat_engine::HealResult>> {
    let target_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(target_snap.encounter_id).fetch_one(&s.db).await?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
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
        .execute(&mut *tx).await?;

    // PHB p.197: healing a dying creature (0 HP) resets death saves
    if reviving_from_zero {
        if let Some(chid) = target_snap.character_id {
            sqlx::query(
                r#"update characters set sheet = coalesce(sheet,'{}'::jsonb)
                   || jsonb_build_object('alive', true,
                        'death_saves', jsonb_build_object('successes', 0, 'failures', 0))
                   where id = $1"#)
                .bind(chid).execute(&mut *tx).await?;
        }
    }

    let round: i32 = sqlx::query_scalar("select round from encounters where id = $1")
        .bind(target_snap.encounter_id).fetch_one(&s.db).await?;

    let source_name = if let Some(sid) = body.source_combatant_id {
        sqlx::query_scalar::<_, String>("select display_name from combatants where id = $1")
            .bind(sid).fetch_optional(&s.db).await?.unwrap_or_else(|| "Unknown".into())
    } else { "DM".into() };

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

    if let Err(e) = sync_combatant_hp_to_sheet(&s.db, id, result.hp_after, result.temp_hp_after).await {
        tracing::warn!("sync sheet HP: {e}");
    }

    ws::publish(campaign_id, json!({
        "type": "combatant_healed",
        "target_id": id,
        "amount": result.amount,
        "hp_after": result.hp_after,
        "stabilized": result.stabilized,
        "revived": reviving_from_zero,
    }).to_string());

    Ok(Json(result))
}

// =====================================================================
// Death Save
// =====================================================================

#[derive(Debug, Deserialize)]
pub struct DeathSaveBody {
    advantage: bool,
    disadvantage: bool,
    label: Option<String>,
}

pub async fn death_save(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<DeathSaveBody>,
) -> AppResult<Json<combat_engine::DeathSaveResult>> {
    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(snap.encounter_id).fetch_one(&s.db).await?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    // Only allow death saves at 0 HP
    if snap.hp_current > 0 {
        return Err(AppError::BadRequest("character is not dying".into()));
    }

    let req = combat_engine::DeathSaveReq {
        advantage: body.advantage,
        disadvantage: body.disadvantage,
        label: body.label,
    };
    let result = combat_engine::resolve_death_save(&snap, &req)
        .map_err(|e| AppError::BadRequest(e))?;

    let mut tx = s.db.begin().await?;

    // Update combatant HP
    sqlx::query("update combatants set hp_current = $1 where id = $2")
        .bind(result.hp_after)
        .bind(id)
        .execute(&mut *tx).await?;

    // Update character sheet death_saves + alive
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
        .execute(&mut *tx).await?;
    }

    let round: i32 = sqlx::query_scalar("select round from encounters where id = $1")
        .bind(snap.encounter_id).fetch_one(&s.db).await?;

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
        tracing::warn!("sync sheet HP: {e}");
    }

    ws::publish(campaign_id, json!({
        "type": "combatant_death_save",
        "combatant_id": id,
        "natural_roll": result.natural_roll,
        "passed": result.passed,
        "successes": result.successes_after,
        "failures": result.failures_after,
        "stabilized": result.stabilized,
        "died": result.died,
        "hp_after": result.hp_after,
        "alive": result.alive,
    }).to_string());

    Ok(Json(result))
}

// =====================================================================
// Skill Check
// =====================================================================

#[derive(Debug, Deserialize)]
pub struct SkillCheckBody {
    skill: String,
    dc: Option<i32>,
    advantage: bool,
    disadvantage: bool,
    label: Option<String>,
}

pub async fn skill_check(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SkillCheckBody>,
) -> AppResult<Json<combat_engine::SkillCheckResult>> {
    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(snap.encounter_id).fetch_one(&s.db).await?;
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

    ws::publish(campaign_id, json!({
        "type": "combatant_skill_check",
        "combatant_id": id,
        "skill": result.skill,
        "total": result.total,
        "dc": result.dc,
        "passed": result.passed,
    }).to_string());

    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
pub struct SaveBody {
    ability: String,
    dc: i32,
    advantage: bool,
    disadvantage: bool,
    label: Option<String>,
    is_magical: Option<bool>,
}

pub async fn roll_save(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SaveBody>,
) -> AppResult<Json<combat_engine::SaveResult>> {
    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(snap.encounter_id).fetch_one(&s.db).await?;
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
    let result = combat_engine::resolve_save(&snap, &req, &stats)
        .map_err(|e| AppError::BadRequest(e))?;

    ws::publish(campaign_id, json!({
        "type": "combatant_save",
        "combatant_id": id,
        "passed": result.passed,
        "save_total": result.save_total,
        "dc": result.dc,
        "natural_roll": result.natural_roll,
    }).to_string());

    Ok(Json(result))
}

pub async fn computed_stats(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<combat_engine::ComputedStats>> {
    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(snap.encounter_id).fetch_one(&s.db).await?;
    rbac::require_member(&s.db, uid, campaign_id).await?;
    let stats = combat_engine::compute_stats(&snap);
    Ok(Json(stats))
}

pub async fn sync_combatant_hp_to_sheet(db: &sqlx::PgPool, combatant_id: Uuid, hp: i32, temp: i32) -> AppResult<()> {
    let row: Option<(Uuid, i32, i32)> = sqlx::query_as(
        "select character_id, hp_max, ac from combatants where id = $1 and ref_type = 'character'")
        .bind(combatant_id).fetch_optional(db).await?;
    if let Some((chid, hp_max, ac)) = row {
        let alive = hp > 0;
        sqlx::query(
            r#"update characters set sheet =
                 coalesce(sheet, '{}'::jsonb)
                 || jsonb_build_object(
                      'hp', coalesce(sheet->'hp', '{}'::jsonb)
                            || jsonb_build_object('current', $2::int, 'max', $3::int, 'temp', $4::int),
                      'ac', $5::int,
                      'alive', $6::bool,
                      'death_saves', case when $6::bool and coalesce((sheet->>'alive')::bool, true) = false
                                       then jsonb_build_object('successes', 0, 'failures', 0)
                                       else coalesce(sheet->'death_saves', jsonb_build_object('successes', 0, 'failures', 0))
                                     end
                    )
               where id = $1"#,
        )
        .bind(chid).bind(hp).bind(hp_max).bind(temp).bind(ac).bind(alive)
        .execute(db).await?;
    }
    Ok(())
}

pub async fn sync_combatant_hp_to_sheet_tx(conn: &mut sqlx::PgConnection, combatant_id: Uuid, hp: i32, temp: i32) -> AppResult<()> {
    let row: Option<(Uuid, i32, i32)> = sqlx::query_as(
        "select character_id, hp_max, ac from combatants where id = $1 and ref_type = 'character'")
        .bind(combatant_id).fetch_optional(&mut *conn).await?;
    if let Some((chid, hp_max, ac)) = row {
        let alive = hp > 0;
        sqlx::query(
            r#"update characters set sheet =
                 coalesce(sheet, '{}'::jsonb)
                 || jsonb_build_object(
                      'hp', coalesce(sheet->'hp', '{}'::jsonb)
                            || jsonb_build_object('current', $2::int, 'max', $3::int, 'temp', $4::int),
                      'ac', $5::int,
                      'alive', $6::bool,
                      'death_saves', case when $6::bool and coalesce((sheet->>'alive')::bool, true) = false
                                       then jsonb_build_object('successes', 0, 'failures', 0)
                                       else coalesce(sheet->'death_saves', jsonb_build_object('successes', 0, 'failures', 0))
                                     end
                    )
               where id = $1"#,
        )
        .bind(chid).bind(hp).bind(hp_max).bind(temp).bind(ac).bind(alive)
        .execute(&mut *conn).await?;
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct ReactBody {
    pub reaction_type: String, // shield | counterspell | opportunity_attack | custom
    pub label: Option<String>,
}

pub async fn react(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ReactBody>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Uuid, String, bool, Option<Uuid>) = sqlx::query_as(
        r#"select e.campaign_id, e.id, e.status::text, c.reaction_used, ch.owner_id
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let (campaign_id, encounter_id, status, _reaction_used, owner) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }
    if status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }
    let mut tx = s.db.begin().await?;

    // Atomic reaction consumption
    let c: Combatant = sqlx::query_as::<_, Combatant>(
        r#"update combatants set reaction_used = true where id = $1 and reaction_used = false
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast"#,
    )
    .bind(id)
    .fetch_optional(&mut *tx).await?
    .ok_or(AppError::BadRequest("reaction already used this round".into()))?;

    // Apply reaction-specific effects
    let mut shield_blocked_hit = false;
    match body.reaction_type.as_str() {
        "shield" => {
            // PHB: Shield reaction only valid when hit (has last_hit_attack_total this round).
            // Check trigger context.
            let last_hit: Option<(Option<i32>, Option<i32>, Option<Uuid>)> = sqlx::query_as(
                "select last_hit_attack_total, last_hit_damage, last_hit_attacker from combatants where id = $1")
                .bind(id).fetch_optional(&mut *tx).await?;

            let (atk_total, pending_dmg, _attacker) = last_hit.unwrap_or((None, None, None));

            if atk_total.is_none() {
                return Err(AppError::BadRequest(
                    "Shield can only be used when you have been hit (no pending hit this round)".into()
                ));
            }

            // Shield adds +5 AC. Check if the hit now misses.
            let snap = combat_engine::load_snapshot(&s.db, id).await?;
            let stats = combat_engine::compute_stats(&snap);
            let ac_with_shield = stats.ac + 5;
            let attack_total = atk_total.unwrap_or(0);

            // Apply the +5 AC effect for the rest of the round
            sqlx::query(
                r#"insert into combatant_effects
                   (combatant_id, name, kind, duration_unit, duration_value, remaining, tick_trigger,
                    concentration, active, modifiers, source_type)
                   values ($1, 'Shield (Reaction)', 'buff', 'rounds', 1, 1, 'caster_turn_start',
                           false, true, '{"ac_bonus": 5}', 'spell')"#,
            )
            .bind(id).execute(&mut *tx).await?;

            if attack_total < ac_with_shield {
                // Hit is retroactively negated — reverse the damage
                let dmg_to_restore = pending_dmg.unwrap_or(0);
                let current_hp: (i32, i32) = sqlx::query_as(
                    "select hp_current, temp_hp from combatants where id = $1")
                    .bind(id).fetch_one(&mut *tx).await?;
                let new_hp = (current_hp.0 + dmg_to_restore).min(snap.hp_max);
                sqlx::query("update combatants set hp_current = $1, last_hit_attack_total = null, last_hit_damage = null where id = $2")
                    .bind(new_hp).bind(id).execute(&mut *tx).await?;
                shield_blocked_hit = true;
            } else {
                // Hit still lands even with +5 AC; just clear the pending hit
                sqlx::query("update combatants set last_hit_attack_total = null, last_hit_damage = null where id = $1")
                    .bind(id).execute(&mut *tx).await?;
            }
        }
        "counterspell" => {
            // Counterspell only valid when a spell is actively being cast.
            // Check spell_being_cast on the combatants in the encounter.
            let active_cast: Option<(Uuid, String)> = sqlx::query_as(
                r#"select id, spell_being_cast from combatants
                   where encounter_id = $1 and spell_being_cast is not null
                   limit 1"#)
                .bind(encounter_id).fetch_optional(&mut *tx).await?;

            if active_cast.is_none() {
                return Err(AppError::BadRequest(
                    "Counterspell can only be used when a spell is being cast".into()
                ));
            }
            // Clear the spell_being_cast flag — spell is countered
            if let Some((caster_id, _slug)) = active_cast {
                sqlx::query("update combatants set spell_being_cast = null where id = $1")
                    .bind(caster_id).execute(&mut *tx).await?;
            }
        }
        _ => {}
    }

    tx.commit().await?;

    let label = body.label.unwrap_or_else(|| body.reaction_type.clone());
    ws::publish(campaign_id, json!({
        "type": "combatant_reacted",
        "combatant_id": id,
        "reaction_type": body.reaction_type,
        "label": label,
        "shield_blocked_hit": shield_blocked_hit,
    }).to_string());

    emit_campaign(&s.db, campaign_id, None,
        "combat.reaction",
        &format!("{} used reaction: {}", c.display_name, label),
        None, Some("encounter"), Some(encounter_id)).await;

    Ok(Json(c))
}


// =====================================================================
// Dodge / Disengage / Help Actions
// =====================================================================

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

    // Atomic action consumption MUST happen first.
    // If action is already used, we return early WITHOUT persisting any effects.
    let mut tx = s.db.begin().await?;

    let action_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set action_used = true where id = $1 and action_used = false returning id")
        .bind(id).fetch_optional(&mut *tx).await?;
    if action_consumed.is_none() {
        return Err(AppError::BadRequest("action already used".into()));
    }

    // Remove existing dodge effect first
    sqlx::query("update combatant_effects set active = false where combatant_id = $1 and name = 'Dodge'")
        .bind(id).execute(&mut *tx).await?;

    // Apply dodge: attackers have disadvantage
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
    ws::publish(campaign_id, json!({"type":"combatant_dodged","id":id}).to_string());
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

    // Atomic action/BA consumption MUST happen first.
    // If action is already used, return early without persisting any effect.
    let mut tx = s.db.begin().await?;

    if body.use_bonus_action {
        let ba_consumed: Option<Uuid> = sqlx::query_scalar(
            "update combatants set bonus_action_used = true where id = $1 and bonus_action_used = false returning id")
            .bind(id).fetch_optional(&mut *tx).await?;
        if ba_consumed.is_none() {
            return Err(AppError::BadRequest("bonus action already used".into()));
        }
    } else {
        let action_consumed: Option<Uuid> = sqlx::query_scalar(
            "update combatants set action_used = true where id = $1 and action_used = false returning id")
            .bind(id).fetch_optional(&mut *tx).await?;
        if action_consumed.is_none() {
            return Err(AppError::BadRequest("action already used".into()));
        }
    }

    // Deactivate old disengage, then insert new one (only after action consumed successfully)
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
    ws::publish(campaign_id, json!({"type":"combatant_disengaged","id":id}).to_string());
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

    // Atomic action consumption MUST happen first.
    // If action is already used, return early without persisting the Helped effect on target.
    let mut tx = s.db.begin().await?;

    let action_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set action_used = true where id = $1 and action_used = false returning id")
        .bind(id).fetch_optional(&mut *tx).await?;
    if action_consumed.is_none() {
        return Err(AppError::BadRequest("action already used".into()));
    }

    // Apply "Helped" effect on the target: next attack against target gets advantage
    // and target gains advantage on next skill check
    sqlx::query(
        r#"insert into combatant_effects
           (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
            concentration, active, modifiers, source_type)
           values ($1, 'Helped', 'buff', 'hand', 'rounds', 1, 1, 'target_turn_start',
                   false, true, '{"attack_advantage_against": true, "save_advantage": true}', 'ability')"#,
    )
    .bind(target_id)
    .execute(&mut *tx).await?;

    tx.commit().await?;

    let c = refresh_combatant(&s.db, id).await?;
    ws::publish(campaign_id, json!({"type":"combatant_helped","helper_id":id,"target_id":target_id}).to_string());
    Ok(Json(c))
}

// =====================================================================
// Opportunity Attack
// =====================================================================

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
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    // Verify attacker has reaction available
    if attacker_snap.active_effects.iter().any(|e| e.modifiers.get("reaction_used").is_some()) {
        // Actually, we need to check the combatant's reaction_used field
    }

    // Check attacker is not incapacitated
    let attacker_stats = combat_engine::compute_stats(&attacker_snap);
    if attacker_stats.incapacitated {
        return Err(AppError::BadRequest("attacker is incapacitated".into()));
    }

    // Check target doesn't have disengage
    let target_stats = combat_engine::compute_stats(&target_snap);
    // Note: disengage check would need to look for active disengage effect
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
        damage_type: "slashing".to_string(),
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

    // Atomic reaction consumption
    let reaction_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set reaction_used = true where id = $1 and reaction_used = false returning id")
        .bind(id).fetch_optional(&mut *tx).await?;
    if reaction_consumed.is_none() {
        return Err(AppError::BadRequest("reaction already used".into()));
    }

    if result.hit {
        sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
            .bind(result.target_hp_after)
            .bind(result.target_temp_hp_after)
            .bind(body.target_id)
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

    // Reveal hidden attacker on any attack (hit or miss)
    sqlx::query(
        "update combatant_effects set active = false
         where combatant_id = $1 and active = true
           and modifiers->>'hidden' = 'true'")
        .bind(id).execute(&mut *tx).await?;

    tx.commit().await?;

    if result.hit {
        if let Err(e) = sync_combatant_hp_to_sheet(&s.db, body.target_id, result.target_hp_after, result.target_temp_hp_after).await {
            tracing::warn!("sync sheet HP: {e}");
        }
    }

    ws::publish(campaign_id, json!({
        "type": "combatant_opportunity_attack",
        "attacker_id": id,
        "target_id": body.target_id,
        "hit": result.hit,
        "damage": result.damage_applied,
        "instant_death": result.instant_death,
    }).to_string());

    Ok(Json(result))
}

pub async fn refresh_combatant(db: &sqlx::PgPool, id: Uuid) -> AppResult<Combatant> {
    sqlx::query_as::<_, Combatant>(
        r#"select id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                token_x, token_y, token_color, token_on_map, token_image,
                coalesce(token_image, (select portrait_url from characters where id = character_id), (select image_key from npcs where id = npc_id)) as portrait_url,
                token_moved_round,
                action_used, bonus_action_used, reaction_used, movement_used_ft,
                legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast
         from combatants where id = $1"#,
    )
    .bind(id)
    .fetch_one(db)
    .await
    .map_err(|_| AppError::NotFound)
}

pub async fn auto_trigger_ready_actions_for_event(
    db: &sqlx::PgPool,
    campaign_id: Uuid,
    encounter_id: Uuid,
    event_type: &str,
    actor_id: Uuid,
    subject_id: Uuid,
) {
    // Fetch all combatants with a readied action in this encounter
    let readied: Vec<(Uuid, serde_json::Value, bool)> = match sqlx::query_as(
        r#"select id, readied_action, reaction_used
           from combatants
           where encounter_id = $1 and readied_action is not null and reaction_used = false"#)
        .bind(encounter_id).fetch_all(db).await {
        Ok(rows) => rows,
        Err(_) => return,
    };

    for (cid, action_json, _) in readied {
        // Skip the actor themselves
        if cid == actor_id { continue; }

        let trigger_event = action_json.get("trigger_event")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let watch_target = action_json.get("watch_target_id")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<Uuid>().ok());

        // Check if event type matches
        if trigger_event != event_type { continue; }

        // Check if they're watching a specific target (or watching anyone)
        if let Some(wid) = watch_target {
            if wid != subject_id { continue; }
        }

        // Trigger: consume reaction, clear readied_action, grant free action
        let ok = sqlx::query(
            "update combatants set reaction_used = true, readied_action = null, action_used = false
             where id = $1 and reaction_used = false")
            .bind(cid).execute(db).await.is_ok();

        if ok {
            ws::publish(campaign_id, json!({
                "type": "combatant_readied_triggered",
                "combatant_id": cid,
                "trigger_event": event_type,
                "triggered_by": actor_id,
                "readied_action": action_json,
            }).to_string());
        }
    }
}


// =====================================================================
// Ready Action
// =====================================================================

#[derive(Debug, Deserialize)]
pub struct ReadyBody {
    pub trigger: String, // e.g. "enemy moves within reach", "spell is cast"
    pub action: String,  // e.g. "attack", "cast spell", "dash"
    pub _target_id: Option<Uuid>,
    /// Automated trigger event: "target_attacks" | "target_casts" | "target_enters_range"
    pub trigger_event: Option<String>,
    /// Specific combatant to watch (None = watch anyone)
    pub watch_target_id: Option<Uuid>,
}

pub async fn ready_action(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ReadyBody>,
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

    // Atomic action consumption: check AND set in one query to prevent TOCTOU.
    let readied = json!({
        "trigger": body.trigger,
        "action": body.action,
        "target_id": body._target_id,
        "trigger_event": body.trigger_event,
        "watch_target_id": body.watch_target_id,
    });

    let c: Option<Combatant> = sqlx::query_as::<_, Combatant>(
        r#"update combatants set action_used = true, readied_action = $2
           where id = $1 and action_used = false
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast"#,
    )
    .bind(id)
    .bind(readied)
    .fetch_optional(&s.db).await?;

    let c = c.ok_or_else(|| AppError::BadRequest("action already used this turn".into()))?;

    ws::publish(campaign_id, json!({
        "type": "combatant_readied",
        "id": id,
        "trigger": body.trigger,
        "action": body.action,
    }).to_string());

    Ok(Json(c))
}

// =====================================================================
// Delay Turn
// =====================================================================

#[derive(Debug, Deserialize)]
pub struct DelayBody {
    pub insert_after_turn_index: i32, // re-insert after this turn index
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

    // Set delayed_turn flag and atomically consume action.
    let c: Option<Combatant> = sqlx::query_as::<_, Combatant>(
        r#"update combatants set delayed_turn = true, action_used = true, readied_action = null
           where id = $1 and action_used = false
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast"#,
    )
    .bind(id)
    .fetch_optional(&mut *tx).await?;

    let c = c.ok_or_else(|| AppError::BadRequest("action already used this turn".into()))?;

    // Reorder: shift all combatants with turn_order > current_turn down by 1,
    // then place the delayed combatant after insert_after_turn_index
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
    use_bonus_action: bool,
}

// =====================================================================
// Two-Weapon Fighting
// =====================================================================

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
    let target_stats = combat_engine::compute_stats(&target_snap);

    // Check for Two-Weapon Fighting style in sheet features
    let twf_style = attacker_snap.sheet_raw.get("features")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().any(|f| {
            f.get("name").and_then(|v| v.as_str())
                .map(|n| n.to_lowercase().contains("two-weapon fighting"))
                .unwrap_or(false)
        }))
        .unwrap_or(false);

    // Look up offhand weapon for range + thrown checks
    let offhand_weapon = combat_engine::find_weapon(&attacker_snap, &body.offhand_weapon_id);
    let offhand_props = offhand_weapon.as_ref().map(|(_, p)| p.clone()).unwrap_or_default();

    // Long-range check for ranged/thrown off-hand weapons
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

    // Consume BONUS action (not action)
    let bonus_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set bonus_action_used = true where id = $1 and bonus_action_used = false returning id")
        .bind(id).fetch_optional(&mut *tx).await?;
    if bonus_consumed.is_none() {
        return Err(AppError::BadRequest("bonus action already used".into()));
    }

    // Decrement thrown weapon quantity for off-hand if applicable
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
            .bind(result.target_hp_after)
            .bind(result.target_temp_hp_after)
            .bind(body.target_id)
            .execute(&mut *tx).await?;

        if result.concentration_broken {
            sqlx::query("update combatant_effects set active = false where combatant_id = $1 and concentration = true and active = true")
                .bind(body.target_id)
                .execute(&mut *tx).await?;
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
            tracing::warn!("sync sheet HP: {e}");
        }
    }

    ws::publish(campaign_id, json!({
        "type": "combatant_two_weapon_fought",
        "attacker_id": id,
        "target_id": body.target_id,
        "hit": result.hit,
        "critical": result.critical,
        "damage": if result.hit { Some(result.damage_applied) } else { None },
        "hp_after": if result.hit { Some(result.target_hp_after) } else { None },
        "temp_hp_after": if result.hit { Some(result.target_temp_hp_after) } else { None },
        "concentration_broken": if result.hit { Some(result.concentration_broken) } else { None },
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

    // Atomic action/BA consumption
    let mut tx = s.db.begin().await?;

    if body.use_bonus_action {
        let ba_consumed: Option<Uuid> = sqlx::query_scalar(
            "update combatants set bonus_action_used = true where id = $1 and bonus_action_used = false returning id")
            .bind(id).fetch_optional(&mut *tx).await?;
        if ba_consumed.is_none() {
            return Err(AppError::BadRequest("bonus action already used".into()));
        }
    } else {
        let action_consumed: Option<Uuid> = sqlx::query_scalar(
            "update combatants set action_used = true where id = $1 and action_used = false returning id")
            .bind(id).fetch_optional(&mut *tx).await?;
        if action_consumed.is_none() {
            return Err(AppError::BadRequest("action already used".into()));
        }
    }

    // Apply Dash: grants extra movement equal to speed for this turn
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
    .bind(json!({"extra_movement": extra}))
    .execute(&mut *tx).await?;

    tx.commit().await?;

    let c = refresh_combatant(&s.db, id).await?;
    ws::publish(campaign_id, json!({"type":"combatant_dashed","id":id,"extra_movement":extra}).to_string());
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

    // Atomic action/BA consumption
    let mut tx = s.db.begin().await?;

    if body.use_bonus_action {
        let ba_consumed: Option<Uuid> = sqlx::query_scalar(
            "update combatants set bonus_action_used = true where id = $1 and bonus_action_used = false returning id")
            .bind(id).fetch_optional(&mut *tx).await?;
        if ba_consumed.is_none() {
            return Err(AppError::BadRequest("bonus action already used".into()));
        }
    } else {
        let action_consumed: Option<Uuid> = sqlx::query_scalar(
            "update combatants set action_used = true where id = $1 and action_used = false returning id")
            .bind(id).fetch_optional(&mut *tx).await?;
        if action_consumed.is_none() {
            return Err(AppError::BadRequest("action already used".into()));
        }
    }

    // Apply Hidden effect
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
    ws::publish(campaign_id, json!({"type":"combatant_hid","id":id}).to_string());
    Ok(Json(c))
}

// =====================================================================
// Contested Hide — Stealth vs Passive Perception
// =====================================================================

#[derive(Debug, Deserialize)]
pub struct ContestedHideBody {
    /// If empty, all enemies in encounter are considered observers
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

    // Consume action/BA
    let mut tx = s.db.begin().await?;

    if body.use_bonus_action.unwrap_or(false) {
        let ba_consumed: Option<Uuid> = sqlx::query_scalar(
            "update combatants set bonus_action_used = true where id = $1 and bonus_action_used = false returning id")
            .bind(id).fetch_optional(&mut *tx).await?;
        if ba_consumed.is_none() {
            return Err(AppError::BadRequest("bonus action already used".into()));
        }
    } else {
        let action_consumed: Option<Uuid> = sqlx::query_scalar(
            "update combatants set action_used = true where id = $1 and action_used = false returning id")
            .bind(id).fetch_optional(&mut *tx).await?;
        if action_consumed.is_none() {
            return Err(AppError::BadRequest("action already used".into()));
        }
    }

    // Apply Hidden only if not spotted by any observer
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

    // Atomic action consumption
    let mut tx = s.db.begin().await?;

    let action_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set action_used = true where id = $1 and action_used = false returning id")
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
    ws::publish(campaign_id, json!({"type":"combatant_searched","id":id,"label":label}).to_string());
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

    // Atomic action consumption
    let mut tx = s.db.begin().await?;

    let action_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set action_used = true where id = $1 and action_used = false returning id")
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
    ws::publish(campaign_id, json!({"type":"combatant_used_object","id":id,"label":label}).to_string());
    Ok(Json(c))
}

