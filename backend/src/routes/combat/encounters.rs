use super::*;
use tracing::warn;

#[derive(Debug, Deserialize, Validate)]
pub struct EncounterCreate {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct EncounterUpdate {
    #[validate(length(min = 1, max = 120))]
    pub name: Option<String>,
    pub notes: Option<String>,
    pub map_image: Option<String>,
    pub clear_map_image: Option<bool>,
    #[validate(range(min = 20, max = 200))]
    pub map_grid_size: Option<i32>,
    pub show_grid: Option<bool>,
    pub grid_type: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct SetInitiativeBody {
    pub character_id: Uuid,
    pub initiative: i32,
}

#[derive(Debug, Deserialize)]
pub struct GotoTurnBody {
    pub turn_index: i32,
}

pub async fn list(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(campaign_id): Path<Uuid>,
) -> AppResult<Json<Vec<Encounter>>> {
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    let rows: Vec<Encounter> = if role == Role::Master {
        sqlx::query_as::<_, Encounter>(
            "select id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at
             from encounters where campaign_id = $1 order by updated_at desc")
            .bind(campaign_id).fetch_all(&s.db).await?
    } else {
        sqlx::query_as::<_, Encounter>(
            "select id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at
             from encounters where campaign_id = $1 and status in ('planned','active') order by updated_at desc")
            .bind(campaign_id).fetch_all(&s.db).await?
    };
    Ok(Json(rows))
}

pub async fn create(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(campaign_id): Path<Uuid>,
    Json(body): Json<EncounterCreate>,
) -> AppResult<(StatusCode, Json<Encounter>)> {
    body.validate()?;
    rbac::require_master(&s.db, uid, campaign_id).await?;
    let e: Encounter = sqlx::query_as::<_, Encounter>(
        "insert into encounters (campaign_id, name, notes) values ($1, $2, $3)
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at")
        .bind(campaign_id).bind(&body.name).bind(&body.notes).fetch_one(&s.db).await?;

    // auto-add all LIVING campaign characters as pending combatants so
    // players can roll initiative as soon as the encounter exists. Dead
    // characters (sheet.alive = false) are skipped.
    sqlx::query(
        r#"insert into combatants
             (encounter_id, ref_type, character_id, display_name, initiative,
              hp_current, hp_max, ac, initiative_rolled)
           select $1, 'character'::combatant_ref, ch.id, ch.name,
                  0,
                  greatest(1, coalesce((ch.sheet->'hp'->>'current')::int, (ch.sheet->'hp'->>'max')::int, 10)),
                  greatest(1, coalesce((ch.sheet->'hp'->>'max')::int, 10)),
                  coalesce((ch.sheet->>'ac')::int, 10),
                  false
           from characters ch
           where ch.campaign_id = $2
             and coalesce((ch.sheet->>'alive')::boolean, true) = true"#,
    )
    .bind(e.id).bind(campaign_id).execute(&s.db).await?;

    ws::publish(campaign_id, json!({"type":"encounter_created","id":e.id,"name":e.name}).to_string());
    emit_campaign(&s.db, campaign_id, Some(uid),
        "combat.roll_initiative",
        &format!("Combat: {}", e.name),
        Some("Roll initiative to join!"),
        Some("encounter"), Some(e.id)).await;
    Ok((StatusCode::CREATED, Json(e)))
}

pub async fn read(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Encounter>> {
    let e = fetch(&s, id).await?;
    rbac::require_member(&s.db, uid, e.campaign_id).await?;
    Ok(Json(e))
}

pub async fn update(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<EncounterUpdate>,
) -> AppResult<Json<Encounter>> {
    body.validate()?;
    let e = fetch(&s, id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;
    let clear_map = body.clear_map_image.unwrap_or(false);
    let e: Encounter = sqlx::query_as::<_, Encounter>(
        r#"update encounters set
             name           = coalesce($2, name),
             notes          = coalesce($3, notes),
             map_image      = case when $5 then null else coalesce($4, map_image) end,
             map_grid_size  = coalesce($6, map_grid_size),
             show_grid      = coalesce($7, show_grid),
             grid_type      = coalesce($8, grid_type)
           where id = $1
           returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at"#)
        .bind(id)
        .bind(body.name)
        .bind(body.notes)
        .bind(body.map_image)
        .bind(clear_map)
        .bind(body.map_grid_size)
        .bind(body.show_grid)
        .bind(body.grid_type)
        .fetch_one(&s.db).await?;
    ws::publish(e.campaign_id, json!({"type":"encounter_updated","id":id}).to_string());
    Ok(Json(e))
}

pub async fn delete(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let e = fetch(&s, id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;
    sqlx::query("delete from encounters where id = $1").bind(id).execute(&s.db).await?;
    ws::publish(e.campaign_id, json!({"type":"encounter_deleted","id":id}).to_string());
    Ok(StatusCode::NO_CONTENT)
}

pub async fn start(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Encounter>> {
    let e = fetch(&s, id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;

    let mut tx = s.db.begin().await?;

    // Reset stale per-round move trackers from any prior session of this encounter.
    sqlx::query(
        "update combatants set token_moved_round = null where encounter_id = $1")
        .bind(id).execute(&mut *tx).await?;

    // auto-add any party characters not already in this encounter. They
    // start with initiative_rolled = false so they sit out until the owner
    // rolls initiative.
    sqlx::query(
        r#"insert into combatants
             (encounter_id, ref_type, character_id, display_name, initiative,
              hp_current, hp_max, ac, initiative_rolled)
           select $1, 'character'::combatant_ref, ch.id, ch.name,
                  0,
                  greatest(1, coalesce((ch.sheet->'hp'->>'current')::int, (ch.sheet->'hp'->>'max')::int, 10)),
                  greatest(1, coalesce((ch.sheet->'hp'->>'max')::int, 10)),
                  coalesce((ch.sheet->>'ac')::int, 10),
                  false
           from characters ch
           where ch.campaign_id = $2
             and coalesce((ch.sheet->>'alive')::boolean, true) = true
             and not exists (
               select 1 from combatants c
               where c.encounter_id = $1 and c.character_id = ch.id
             )"#,
    )
    .bind(id).bind(e.campaign_id).execute(&mut *tx).await?;

    // Re-check for unrolled character combatants INSIDE the transaction.
    // A character created between the request arriving and now will have been
    // auto-added above; they must also roll before the encounter can start.
    let pending_names: Vec<String> = sqlx::query_scalar(
        "select c.display_name from combatants c
         where c.encounter_id = $1
           and c.ref_type = 'character'
           and c.initiative_rolled = false
         order by c.display_name")
        .bind(id).fetch_all(&mut *tx).await?;
    if !pending_names.is_empty() {
        tx.rollback().await?;
        let who = pending_names.join(", ");
        return Err(AppError::BadRequest(
            format!("Waiting for initiative from: {who}")));
    }

    // order only combatants that have rolled — unrolled go to the end, skipped
    // during turn cycling.
    sqlx::query(
        r#"with ordered as (
             select id,
                    row_number() over (
                        order by initiative_rolled desc,
                                 initiative desc,
                                 dex_tiebreaker desc
                    ) - 1 as ord
             from combatants where encounter_id = $1
           )
           update combatants c set turn_order = o.ord
           from ordered o where c.id = o.id"#,
    )
    .bind(id).execute(&mut *tx).await?;

    // first turn = first rolled combatant, or -1 if none rolled yet
    let first_idx: Option<i64> = sqlx::query_scalar(
        "select min(turn_order)::bigint from combatants where encounter_id = $1 and initiative_rolled = true")
        .bind(id).fetch_one(&mut *tx).await?;
    let start_idx = first_idx.unwrap_or(0) as i32;

    let e: Encounter = sqlx::query_as::<_, Encounter>(
        "update encounters set status = 'active', round = 1, turn_index = $2 where id = $1
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at")
        .bind(id).bind(start_idx).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    ws::publish(e.campaign_id, json!({"type":"encounter_started","id":id,"round":1,"turn_index":start_idx}).to_string());
    emit_campaign(&s.db, e.campaign_id, None,
        "combat.started", &format!("Combat started: {}", e.name),
        None, Some("encounter"), Some(id)).await;

    // notify every party character owner whose combatant still needs init
    let pending: Vec<(Uuid, String)> = sqlx::query_as(
        r#"select ch.owner_id, c.display_name
           from combatants c join characters ch on ch.id = c.character_id
           where c.encounter_id = $1 and c.initiative_rolled = false"#,
    )
    .bind(id).fetch_all(&s.db).await.unwrap_or_else(|e| { warn!(%e, "initiative notification query failed"); Vec::new() });
    for (owner, name) in pending {
        emit(&s.db, NewNotif {
            user_id: owner, campaign_id: Some(e.campaign_id),
            kind: "combat.roll_initiative",
            title: "Roll initiative!",
            body: Some(&format!("Combat started — roll initiative for {}", name)),
            ref_kind: Some("encounter"), ref_id: Some(e.id),
        }).await;
    }

    if first_idx.is_some() { notify_turn(&s, &e, 0).await; }
    Ok(Json(e))
}

pub async fn set_initiative(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
    Json(body): Json<SetInitiativeBody>,
) -> AppResult<Json<Combatant>> {
    let e = fetch(&s, encounter_id).await?;
    rbac::require_member(&s.db, uid, e.campaign_id).await?;
    // verify character belongs to user
    let ch: Option<(Uuid, serde_json::Value)> = sqlx::query_as(
        "select owner_id, sheet from characters where id = $1 and campaign_id = $2")
        .bind(body.character_id).bind(e.campaign_id).fetch_optional(&s.db).await?;
    let (owner, sheet) = ch.ok_or(AppError::NotFound)?;
    if owner != uid { return Err(AppError::Forbidden); }
    if sheet.get("alive").and_then(|v| v.as_bool()) == Some(false) {
        return Err(AppError::BadRequest("character is dead".into()));
    }

    let mut tx = s.db.begin().await?;
    let c: Combatant = sqlx::query_as::<_, Combatant>(
        r#"update combatants
           set initiative = $3, initiative_rolled = true
           where encounter_id = $1 and character_id = $2
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast, level_override, vision_range, pending_hits"#,
    )
    .bind(encounter_id).bind(body.character_id).bind(body.initiative)
    .fetch_optional(&mut *tx).await?.ok_or(AppError::NotFound)?;

    // re-order all combatants; rolled first, then unrolled (stable-ish)
    sqlx::query(
        r#"with ordered as (
             select id,
                    row_number() over (
                        order by initiative_rolled desc, initiative desc, dex_tiebreaker desc
                    ) - 1 as ord
             from combatants where encounter_id = $1
           )
           update combatants c set turn_order = o.ord
           from ordered o where c.id = o.id"#,
    )
    .bind(encounter_id).execute(&mut *tx).await?;
    tx.commit().await?;

    ws::publish(e.campaign_id, json!({
        "type":"combatant_updated","id":c.id,"initiative":c.initiative,"initiative_rolled":true
    }).to_string());
    emit_campaign(&s.db, e.campaign_id, Some(uid),
        "combat.rolled",
        &format!("{} rolled initiative: {}", c.display_name, c.initiative),
        None, Some("encounter"), Some(encounter_id)).await;
    Ok(Json(c))
}

pub async fn next_turn(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Encounter>> {
    let e = fetch(&s, id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;
    if e.status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }

    let mut tx = s.db.begin().await?;
    let rolled: i64 = sqlx::query_scalar(
        "select count(*) from combatants where encounter_id = $1 and initiative_rolled = true")
        .bind(id).fetch_one(&mut *tx).await?;
    if rolled == 0 {
        tx.rollback().await?;
        return Err(AppError::BadRequest("waiting for initiative rolls".into()));
    }
    let next_idx = e.turn_index + 1;
    let (new_idx, new_round) = if (next_idx as i64) >= rolled {
        (0, e.round + 1)
    } else {
        (next_idx, e.round)
    };
    let prev_round = e.round;
    let e: Encounter = sqlx::query_as::<_, Encounter>(
        "update encounters set turn_index = $2, round = $3 where id = $1
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at")
        .bind(id).bind(new_idx).bind(new_round).fetch_one(&mut *tx).await?;
    // New round started — reset all player token_moved_round so they can move again.
    // Also reset reactions for ALL combatants, legendary actions for legendary creatures,
    // and lair action.
    if new_round > prev_round {
        sqlx::query(
            "update combatants set token_moved_round = null, reaction_used = false
             where encounter_id = $1")
            .bind(id).execute(&mut *tx).await?;
        sqlx::query(
            "update encounters set lair_action_used = false where id = $1")
            .bind(id).execute(&mut *tx).await?;
        // PHB: readied actions expire at end of next round (set_at_round + 1).
        // New round = old round + 1, so anything with expires_at_round < new_round is stale.
        sqlx::query(
            "update combatants set readied_action = null
             where encounter_id = $1
               and readied_action is not null
               and coalesce((readied_action->>'expires_at_round')::int, 0) < $2")
            .bind(id).bind(new_round).execute(&mut *tx).await?;
    }
    // Reset action/bonus/movement for the combatant whose turn is starting.
    let combatants: Vec<(i32, Uuid)> = sqlx::query_as(
        "select turn_order, id from combatants where encounter_id = $1 and initiative_rolled = true order by turn_order")
        .bind(id).fetch_all(&mut *tx).await?;
    if let Some((_, cid)) = combatants.iter().find(|(t, _)| *t == new_idx) {
        sqlx::query(
            "update combatants set action_used = false, bonus_action_used = false, movement_used_ft = 0, action_spell_level = 0, bonus_action_spell_level = 0, last_hit_attack_total = null, last_hit_damage = null, last_hit_attacker = null, spell_being_cast = null, legendary_actions_used = 0, pending_hits = '[]'::jsonb where id = $1")
            .bind(cid).execute(&mut *tx).await?;
    }
    // Tick down effects based on triggers
    tick_effects(&mut tx, id, prev_round, e.turn_index, new_round, new_idx, e.campaign_id).await?;

    tx.commit().await?;
    ws::publish(e.campaign_id, json!({"type":"next_turn","id":id,"round":new_round,"turn_index":new_idx}).to_string());
    notify_turn(&s, &e, prev_round).await;
    Ok(Json(e))
}

pub async fn prev_turn(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Encounter>> {
    let e = fetch(&s, id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;
    if e.status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }

    let mut tx = s.db.begin().await?;
    let rolled: i64 = sqlx::query_scalar(
        "select count(*) from combatants where encounter_id = $1 and initiative_rolled = true")
        .bind(id).fetch_one(&mut *tx).await?;
    if rolled == 0 {
        tx.rollback().await?;
        return Err(AppError::BadRequest("waiting for initiative rolls".into()));
    }
    let (new_idx, new_round) = if e.turn_index == 0 {
        if e.round <= 1 { (0, 1) } else { ((rolled - 1) as i32, e.round - 1) }
    } else {
        (e.turn_index - 1, e.round)
    };
    let prev_round = e.round;
    let e: Encounter = sqlx::query_as::<_, Encounter>(
        "update encounters set turn_index = $2, round = $3 where id = $1
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at")
        .bind(id).bind(new_idx).bind(new_round).fetch_one(&mut *tx).await?;
    // Round rewound: clear any stale token_moved_round that now points at a
    // round >= the new round, so players can move again in the restored round.
    if new_round < prev_round {
        sqlx::query(
            "update combatants set token_moved_round = null
             where encounter_id = $1 and ref_type = 'character' and token_moved_round >= $2")
            .bind(id).bind(new_round).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    ws::publish(e.campaign_id, json!({"type":"next_turn","id":id,"round":new_round,"turn_index":new_idx}).to_string());
    notify_turn(&s, &e, prev_round).await;
    Ok(Json(e))
}

pub async fn goto_turn(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<GotoTurnBody>,
) -> AppResult<Json<Encounter>> {
    let e = fetch(&s, id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;
    if e.status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }
    let rolled: i64 = sqlx::query_scalar(
        "select count(*) from combatants where encounter_id = $1 and initiative_rolled = true")
        .bind(id).fetch_one(&s.db).await?;
    if rolled == 0 || body.turn_index < 0 || (body.turn_index as i64) >= rolled {
        return Err(AppError::BadRequest("turn_index out of range".into()));
    }
    let prev_round = e.round;
    let mut tx = s.db.begin().await?;
    let e: Encounter = sqlx::query_as::<_, Encounter>(
        "update encounters set turn_index = $2 where id = $1
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at")
        .bind(id).bind(body.turn_index).fetch_one(&mut *tx).await?;
    // Reset per-turn fields for the combatant whose turn is starting.
    let combatants: Vec<(i32, Uuid)> = sqlx::query_as(
        "select turn_order, id from combatants where encounter_id = $1 and initiative_rolled = true order by turn_order")
        .bind(id).fetch_all(&mut *tx).await?;
    if let Some((_, cid)) = combatants.iter().find(|(t, _)| *t == body.turn_index) {
        sqlx::query(
            "update combatants set action_used = false, bonus_action_used = false, movement_used_ft = 0, action_spell_level = 0, bonus_action_spell_level = 0, last_hit_attack_total = null, last_hit_damage = null, last_hit_attacker = null, spell_being_cast = null, legendary_actions_used = 0, pending_hits = '[]'::jsonb where id = $1")
            .bind(cid).execute(&mut *tx).await?;
    }
    tick_effects(&mut tx, id, prev_round, e.turn_index, e.round, body.turn_index, e.campaign_id).await?;
    tx.commit().await?;
    ws::publish(e.campaign_id, json!({"type":"next_turn","id":id,"round":e.round,"turn_index":body.turn_index}).to_string());
    notify_turn(&s, &e, prev_round).await;
    Ok(Json(e))
}

pub async fn end_encounter(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Encounter>> {
    let e = fetch(&s, id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;

    let mut tx = s.db.begin().await?;
    let e: Encounter = sqlx::query_as::<_, Encounter>(
        "update encounters set status = 'ended' where id = $1
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at")
        .bind(id).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    ws::publish(e.campaign_id, json!({"type":"encounter_ended","id":id}).to_string());
    emit_campaign(&s.db, e.campaign_id, None,
        "combat.ended", &format!("Combat ended: {}", e.name),
        None, Some("encounter"), Some(id)).await;
    Ok(Json(e))
}
