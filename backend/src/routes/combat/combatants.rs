use super::*;

use super::actions::auto_trigger_ready_actions_for_event;
use tracing::warn;

#[derive(Debug, Serialize, FromRow)]
pub struct Combatant {
    pub id: Uuid,
    pub encounter_id: Uuid,
    pub ref_type: String,
    pub character_id: Option<Uuid>,
    pub npc_id: Option<Uuid>,
    pub display_name: String,
    pub initiative: i32,
    pub dex_tiebreaker: i16,
    pub hp_current: i32,
    pub hp_max: i32,
    pub temp_hp: i32,
    pub ac: i32,
    pub conditions: Vec<String>,
    pub notes: Option<String>,
    pub is_visible: bool,
    pub turn_order: i32,
    pub initiative_rolled: bool,
    pub token_x: Option<f32>,
    pub token_y: Option<f32>,
    pub token_color: Option<String>,
    pub token_on_map: bool,
    pub token_image: Option<String>,
    pub portrait_url: Option<String>,
    pub token_moved_round: Option<i32>,
    pub action_used: bool,
    pub bonus_action_used: bool,
    pub reaction_used: bool,
    pub movement_used_ft: i32,
    pub legendary_actions_max: i32,
    pub legendary_actions_used: i32,
    pub legendary_resistances_max: i32,
    pub legendary_resistances_used: i32,
    pub readied_action: Option<serde_json::Value>,
    pub cover_bonus: i32,
    pub delayed_turn: bool,
    pub action_spell_level: i16,
    pub bonus_action_spell_level: i16,
    pub last_hit_attack_total: Option<i32>,
    pub last_hit_damage: Option<i32>,
    pub last_hit_attacker: Option<Uuid>,
    pub spell_being_cast: Option<String>,
    pub level_override: i32,
    pub vision_range: Option<i32>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CombatantCreate {
    pub ref_type: String,
    pub character_id: Option<Uuid>,
    pub npc_id: Option<Uuid>,
    #[validate(length(min = 1, max = 80))]
    pub display_name: String,
    pub initiative: Option<i32>,
    pub dex_tiebreaker: Option<i16>,
    pub hp_current: Option<i32>,
    pub hp_max: Option<i32>,
    pub ac: Option<i32>,
    pub is_visible: Option<bool>,
    pub initiative_rolled: Option<bool>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CombatantUpdate {
    #[validate(length(min = 1, max = 80))]
    pub display_name: Option<String>,
    pub initiative: Option<i32>,
    pub dex_tiebreaker: Option<i16>,
    pub hp_current: Option<i32>,
    pub hp_max: Option<i32>,
    pub temp_hp: Option<i32>,
    pub ac: Option<i32>,
    pub conditions: Option<Vec<String>>,
    pub notes: Option<String>,
    pub is_visible: Option<bool>,
    pub token_x: Option<f32>,
    pub token_y: Option<f32>,
    #[validate(length(min = 3, max = 20))]
    pub token_color: Option<String>,
    pub token_on_map: Option<bool>,
    pub token_image: Option<String>,
    pub clear_token_image: Option<bool>,
    pub action_used: Option<bool>,
    pub bonus_action_used: Option<bool>,
    pub reaction_used: Option<bool>,
    pub movement_used_ft: Option<i32>,
    pub legendary_actions_used: Option<i32>,
    pub legendary_resistances_used: Option<i32>,
    pub readied_action: Option<serde_json::Value>,
    pub cover_bonus: Option<i32>,
    pub delayed_turn: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CombatantMove {
    pub x: f32,
    pub y: f32,
    #[serde(default)]
    pub movement_cost: Option<f32>,
}

#[derive(Debug, Deserialize)]
pub struct UseAction {
    pub action: String,
}

#[derive(Debug, Deserialize)]
pub struct BulkAddBody {
    pub combatants: Vec<CombatantCreate>,
}

#[derive(Debug, Serialize)]
pub struct BulkAddResult {
    pub added: usize,
    pub combatants: Vec<Combatant>,
}

pub async fn list_combatants(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
) -> AppResult<Json<Vec<Combatant>>> {
    let e = super::fetch(&s, encounter_id).await?;
    let role = rbac::require_member(&s.db, uid, e.campaign_id).await?;
    let rows: Vec<Combatant> = if role == Role::Master {
        sqlx::query_as::<_, Combatant>(
            "select id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                    initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                    token_x, token_y, token_color, token_on_map, token_image,
                    coalesce(token_image, (select portrait_url from characters where id = character_id), (select image_key from npcs where id = npc_id)) as portrait_url,
                    token_moved_round,
                    action_used, bonus_action_used, reaction_used, movement_used_ft,
                    legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                     readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast, level_override, vision_range
              from combatants where encounter_id = $1 order by turn_order, -initiative, -dex_tiebreaker")
            .bind(encounter_id).fetch_all(&s.db).await?
    } else {
        sqlx::query_as::<_, Combatant>(
            "select c.id, c.encounter_id, c.ref_type::text as ref_type, c.character_id, c.npc_id, c.display_name,
                    c.initiative, c.dex_tiebreaker,
                    case when ch.owner_id = $2 then c.hp_current else 0 end as hp_current,
                    case when ch.owner_id = $2 then c.hp_max     else 0 end as hp_max,
                    case when ch.owner_id = $2 then c.temp_hp    else 0 end as temp_hp,
                    case when ch.owner_id = $2 then c.ac         else 0 end as ac,
                    c.conditions, c.notes, c.is_visible, c.turn_order, c.initiative_rolled,
                    c.token_x, c.token_y, c.token_color, c.token_on_map, c.token_image,
                    coalesce(c.token_image, ch.portrait_url, (select image_key from npcs where id = c.npc_id)) as portrait_url,
                    c.token_moved_round,
                    c.action_used, c.bonus_action_used, c.reaction_used, c.movement_used_ft,
                    c.legendary_actions_max, c.legendary_actions_used, c.legendary_resistances_max, c.legendary_resistances_used,
                     c.readied_action, c.cover_bonus, c.delayed_turn, c.action_spell_level, c.bonus_action_spell_level, c.last_hit_attack_total, c.last_hit_damage, c.last_hit_attacker, c.spell_being_cast, c.level_override, c.vision_range
              from combatants c
             left join characters ch on ch.id = c.character_id
             where c.encounter_id = $1 and c.is_visible = true
             order by c.turn_order, -c.initiative, -c.dex_tiebreaker")
            .bind(encounter_id).bind(uid).fetch_all(&s.db).await?
    };
    Ok(Json(rows))
}

pub async fn add_combatant(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
    Json(body): Json<CombatantCreate>,
) -> AppResult<(StatusCode, Json<Combatant>)> {
    body.validate()?;
    let e = super::fetch(&s, encounter_id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;
    if body.ref_type != "character" && body.ref_type != "npc" {
        return Err(AppError::BadRequest("ref_type must be character|npc".into()));
    }
    if body.ref_type == "character" {
        if let Some(chid) = body.character_id {
            let dead: Option<bool> = sqlx::query_scalar(
                "select (sheet->>'alive')::boolean from characters where id = $1 and campaign_id = $2")
                .bind(chid).bind(e.campaign_id).fetch_optional(&s.db).await?.flatten();
            if dead == Some(false) {
                return Err(AppError::BadRequest("character is dead".into()));
            }
        }
    }

    let mut npc_stats: Option<combat_engine::NpcStats> = None;
    if body.ref_type == "npc" && body.npc_id.is_some() {
        let raw: Option<Value> = sqlx::query_scalar(
            "select stats from npcs where id = $1 and campaign_id = $2")
            .bind(body.npc_id.ok_or(AppError::BadRequest("npc_id required".into()))?).bind(e.campaign_id).fetch_optional(&s.db).await?;
        npc_stats = raw.as_ref().and_then(combat_engine::NpcStats::from_value);
    }

    let default_hp_max = npc_stats.as_ref().and_then(|n| n.hp.max).unwrap_or(0);
    let default_hp_current = npc_stats.as_ref().and_then(|n| n.hp.current).unwrap_or(default_hp_max);
    let default_ac = npc_stats.as_ref().and_then(|n| n.ac).unwrap_or(10);
    let default_dex = npc_stats.as_ref().map(|n| n.abilities.dex).unwrap_or(10);
    let default_legendary_actions = npc_stats.as_ref()
        .and_then(|n| n.legendary_actions.first()).map(|_| 3).unwrap_or(0);
    let default_legendary_resistances = npc_stats.as_ref()
        .and_then(|n| n.traits.iter().find(|t| t.name.to_lowercase().contains("legendary resistance")))
        .map(|_| 3).unwrap_or(0);

    let default_rolled = body.ref_type != "character";
    let c: Combatant = sqlx::query_as::<_, Combatant>(
        r#"insert into combatants
           (encounter_id, ref_type, character_id, npc_id, display_name, initiative, dex_tiebreaker,
            hp_current, hp_max, ac, is_visible, initiative_rolled,
            legendary_actions_max, legendary_resistances_max)
           values ($1, $2::combatant_ref, $3, $4, $5, coalesce($6, 0), coalesce($7, $14),
                   coalesce($8, $15), coalesce($9, $16), coalesce($10, $17), coalesce($11, true), coalesce($12, $13),
                   coalesce($18, 0), coalesce($19, 0))
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast, level_override, vision_range"#,
    )
    .bind(encounter_id).bind(&body.ref_type).bind(body.character_id).bind(body.npc_id)
    .bind(&body.display_name).bind(body.initiative).bind(body.dex_tiebreaker)
    .bind(body.hp_current).bind(body.hp_max).bind(body.ac)
    .bind(body.is_visible).bind(body.initiative_rolled).bind(default_rolled)
    .bind(default_dex as i16).bind(default_hp_current).bind(default_hp_max)
    .bind(default_ac).bind(default_legendary_actions).bind(default_legendary_resistances)
    .fetch_one(&s.db).await?;
    ws::publish(e.campaign_id, json!({"type":"combatant_added","encounter_id":encounter_id,"id":c.id}).to_string());
    emit_campaign(&s.db, e.campaign_id, Some(uid),
        "combat.joined",
        &format!("{} joined combat", c.display_name),
        Some(&format!("Init {} · HP {}/{} · AC {}", c.initiative, c.hp_current, c.hp_max, c.ac)),
        Some("encounter"), Some(encounter_id)).await;
    Ok((StatusCode::CREATED, Json(c)))
}

pub async fn bulk_add_combatants(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
    Json(body): Json<BulkAddBody>,
) -> AppResult<Json<BulkAddResult>> {
    let e = super::fetch(&s, encounter_id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;

    let mut added = Vec::new();
    for spec in &body.combatants {
        if spec.ref_type != "character" && spec.ref_type != "npc" { continue; }
        let mut npc_stats: Option<combat_engine::NpcStats> = None;
        if spec.ref_type == "npc" && spec.npc_id.is_some() {
            if let Ok(Some(raw)) = sqlx::query_scalar::<_, Value>(
                "select stats from npcs where id = $1 and campaign_id = $2"
            ).bind(spec.npc_id).bind(e.campaign_id).fetch_optional(&s.db).await {
                npc_stats = combat_engine::NpcStats::from_value(&raw);
            }
        }
        let default_hp_max = npc_stats.as_ref().and_then(|n| n.hp.max).unwrap_or(0);
        let default_hp_current = npc_stats.as_ref().and_then(|n| n.hp.current).unwrap_or(default_hp_max);
        let default_ac = npc_stats.as_ref().and_then(|n| n.ac).unwrap_or(10);
        let default_dex = npc_stats.as_ref().map(|n| n.abilities.dex).unwrap_or(10);
        let default_legendary = npc_stats.as_ref()
            .and_then(|n| n.legendary_actions.first()).map(|_| 3).unwrap_or(0);
        let default_resist = npc_stats.as_ref()
            .and_then(|n| n.traits.iter().find(|t| t.name.to_lowercase().contains("legendary resistance")))
            .map(|_| 3).unwrap_or(0);
        let default_rolled = spec.ref_type != "character";

        if let Ok(c) = sqlx::query_as::<_, Combatant>(
            r#"insert into combatants
               (encounter_id, ref_type, character_id, npc_id, display_name, initiative, dex_tiebreaker,
                hp_current, hp_max, ac, is_visible, initiative_rolled,
                legendary_actions_max, legendary_resistances_max)
               values ($1, $2::combatant_ref, $3, $4, $5, coalesce($6, 0), coalesce($7, $14),
                       coalesce($8, $15), coalesce($9, $16), coalesce($10, $17), coalesce($11, true), coalesce($12, $13),
                       coalesce($18, 0), coalesce($19, 0))
               returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                         initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                         token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                         action_used, bonus_action_used, reaction_used, movement_used_ft,
                         legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                         readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast, level_override, vision_range"#,
        )
        .bind(encounter_id).bind(&spec.ref_type).bind(spec.character_id).bind(spec.npc_id)
        .bind(&spec.display_name).bind(spec.initiative).bind(spec.dex_tiebreaker)
        .bind(spec.hp_current).bind(spec.hp_max).bind(spec.ac)
        .bind(spec.is_visible).bind(spec.initiative_rolled).bind(default_rolled)
        .bind(default_dex as i16).bind(default_hp_current).bind(default_hp_max)
        .bind(default_ac).bind(default_legendary).bind(default_resist)
        .fetch_one(&s.db).await {
            ws::publish(e.campaign_id, json!({"type":"combatant_added","encounter_id":encounter_id,"id":c.id}).to_string());
            added.push(c);
        }
    }
    Ok(Json(BulkAddResult { added: added.len(), combatants: added }))
}

pub async fn update_combatant(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<CombatantUpdate>,
) -> AppResult<Json<Combatant>> {
    body.validate()?;
    let row: (Uuid, Uuid, i32, String, Option<Uuid>) = sqlx::query_as(
        "select c.id, e.campaign_id, c.hp_current, c.ref_type::text, ch.owner_id \
         from combatants c \
         join encounters e on e.id = c.encounter_id \
         left join characters ch on ch.id = c.character_id \
         where c.id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let campaign_id = row.1;
    let ref_type = row.3;
    let owner = row.4;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master {
        if ref_type != "character" || owner != Some(uid) { return Err(AppError::Forbidden); }
        let cosmetic_only = body.display_name.is_none()
            && body.initiative.is_none() && body.dex_tiebreaker.is_none()
            && body.hp_current.is_none() && body.hp_max.is_none()
            && body.temp_hp.is_none() && body.ac.is_none()
            && body.conditions.is_none() && body.notes.is_none()
            && body.is_visible.is_none()
            && body.token_x.is_none() && body.token_y.is_none()
            && body.token_on_map.is_none()
            && body.token_color.is_none() && body.token_image.is_none()
            && body.action_used.is_none() && body.bonus_action_used.is_none()
            && body.reaction_used.is_none() && body.movement_used_ft.is_none()
            && body.legendary_actions_used.is_none() && body.legendary_resistances_used.is_none();
        if !cosmetic_only { return Err(AppError::Forbidden); }
    }
    if ref_type == "character" && (
        body.hp_current.is_some() || body.hp_max.is_some() || body.temp_hp.is_some() || body.ac.is_some()
    ) {
        return Err(AppError::BadRequest("character HP/AC is owned by the player sheet".into()));
    }
    let prev_hp = row.2;
    let clear_token_image = body.clear_token_image.unwrap_or(false);
    let c: Combatant = sqlx::query_as::<_, Combatant>(
        r#"update combatants set
             display_name   = coalesce($2, display_name),
             initiative     = coalesce($3, initiative),
             dex_tiebreaker = coalesce($4, dex_tiebreaker),
             hp_current     = coalesce($5, hp_current),
             hp_max         = coalesce($6, hp_max),
             temp_hp        = case when $7 is not null and $7 > temp_hp then $7 else temp_hp end,
             ac             = coalesce($8, ac),
             conditions     = coalesce($9, conditions),
             notes          = coalesce($10, notes),
             is_visible     = coalesce($11, is_visible),
             token_x        = coalesce($12, token_x),
             token_y        = coalesce($13, token_y),
             token_color    = coalesce($14, token_color),
             token_on_map   = coalesce($15, token_on_map),
             token_image    = case when $17 then null else coalesce($16, token_image) end,
             action_used    = coalesce($18, action_used),
             bonus_action_used = coalesce($19, bonus_action_used),
             reaction_used  = coalesce($20, reaction_used),
             movement_used_ft = coalesce($21, movement_used_ft),
             legendary_actions_used = coalesce($22, legendary_actions_used),
             legendary_resistances_used = coalesce($23, legendary_resistances_used),
             readied_action = coalesce($24, readied_action),
             cover_bonus = coalesce($25, cover_bonus),
             delayed_turn = coalesce($26, delayed_turn)
           where id = $1
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast, level_override, vision_range"#,
    )
    .bind(id).bind(body.display_name).bind(body.initiative).bind(body.dex_tiebreaker)
    .bind(body.hp_current).bind(body.hp_max).bind(body.temp_hp).bind(body.ac)
    .bind(body.conditions).bind(body.notes).bind(body.is_visible)
    .bind(body.token_x).bind(body.token_y).bind(body.token_color).bind(body.token_on_map)
    .bind(body.token_image).bind(clear_token_image)
    .bind(body.action_used).bind(body.bonus_action_used).bind(body.reaction_used)
    .bind(body.movement_used_ft).bind(body.legendary_actions_used).bind(body.legendary_resistances_used)
    .bind(body.readied_action).bind(body.cover_bonus).bind(body.delayed_turn)
    .fetch_one(&s.db).await?;
    ws::publish(campaign_id, json!({"type":"combatant_updated","id":id}).to_string());

    if c.ref_type == "character" {
        if let Some(chid) = c.character_id {
            if body.hp_current.is_some() || body.hp_max.is_some() || body.temp_hp.is_some() || body.ac.is_some() {
                let new_hp = c.hp_current;
                let alive = new_hp > 0;
                if let Err(e) = sqlx::query(
                    r#"update characters set sheet =
                         coalesce(sheet, '{}'::jsonb)
                         || jsonb_build_object(
                              'hp', coalesce(sheet->'hp', '{}'::jsonb)
                                    || jsonb_build_object(
                                         'current', $2::int, 'max', $3::int, 'temp', $4::int
                                       ),
                              'ac', $5::int,
                              'alive', $6::bool,
                              'death_saves', case when $6::bool and coalesce((sheet->>'alive')::bool, true) = false
                                               then jsonb_build_object('successes', 0, 'failures', 0)
                                               else coalesce(sheet->'death_saves', jsonb_build_object('successes', 0, 'failures', 0))
                                             end
                            )
                       where id = $1"#,
                ).bind(chid)
                .bind(body.hp_current.unwrap_or(c.hp_current))
                .bind(body.hp_max.unwrap_or(c.hp_max))
                .bind(body.temp_hp.unwrap_or(c.temp_hp))
                .bind(body.ac.unwrap_or(c.ac))
                .bind(alive)
                .execute(&s.db).await { warn!("sync sheet on combatant update: {e}"); }
                ws::publish(campaign_id, json!({"type":"character_updated","id":chid}).to_string());
            }
        }
    }

    if prev_hp > 0 && c.hp_current <= 0 {
        emit_campaign(&s.db, campaign_id, None,
            "combat.down",
            &format!("{} dropped to 0 HP", c.display_name),
            None, Some("encounter"), Some(c.encounter_id)).await;
    } else if prev_hp <= 0 && c.hp_current > 0 {
        emit_campaign(&s.db, campaign_id, None,
            "combat.revived",
            &format!("{} is back up ({} HP)", c.display_name, c.hp_current),
            None, Some("encounter"), Some(c.encounter_id)).await;
    }
    Ok(Json(c))
}

pub async fn delete_combatant(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let row: (Uuid, Uuid, Uuid) = sqlx::query_as(
        "select c.id, e.campaign_id, c.encounter_id from combatants c join encounters e on e.id = c.encounter_id where c.id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let (_id, campaign_id, encounter_id) = row;
    rbac::require_master(&s.db, uid, campaign_id).await?;
    sqlx::query("delete from combatants where id = $1").bind(id).execute(&s.db).await?;
    ws::publish(campaign_id, json!({"type":"combatant_removed","id":id,"encounter_id":encounter_id}).to_string());
    Ok(StatusCode::NO_CONTENT)
}

pub async fn move_combatant(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<CombatantMove>,
) -> AppResult<Json<Combatant>> {
    if !body.x.is_finite() || !body.y.is_finite() {
        return Err(AppError::BadRequest("invalid coords".into()));
    }
    let x = body.x.clamp(0.0, 100.0);
    let y = body.y.clamp(0.0, 100.0);

    let row: (Uuid, Option<Uuid>, String, String, Option<f64>, Option<f64>, i32) = sqlx::query_as(
        r#"select e.campaign_id, ch.owner_id, c.ref_type::text, e.status::text,
                  c.token_x, c.token_y, c.movement_used_ft
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#)
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let (campaign_id, owner, ref_type, enc_status, old_x, old_y, _movement_used) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && (ref_type != "character" || owner != Some(uid)) {
        return Err(AppError::Forbidden);
    }

    let is_player_in_active = role != Role::Master && enc_status == "active";

    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    let stats = combat_engine::compute_stats(&snap);
    let speed = stats.speed.max(0);

    let dist_pct = if let (Some(ox), Some(oy)) = (old_x, old_y) {
        let dx = x - ox as f32; let dy = y - oy as f32; (dx*dx + dy*dy).sqrt()
    } else { 0.0 };
    let dist_ft = if let Some(cost) = body.movement_cost {
        (cost.max(0.0) / 5.0).round() as i32 * 5
    } else {
        // Fallback: assume 100% map ≈ 100ft (20 cells), snap to 5ft grid
        // This is approximate without map dimensions; prefer client-supplied movement_cost
        (dist_pct * 100.0 / 5.0).round() as i32 * 5
    };

    // Sum dash bonuses from active effects for movement cap
    let dash_bonus: i32 = snap.active_effects.iter()
        .filter_map(|e| {
            e.modifiers.as_object()
                .and_then(|m| m.get("movement"))
                .and_then(|v| v.as_object())
                .filter(|mov| mov.get("type").and_then(|t| t.as_str()) == Some("dash_bonus"))
                .and_then(|mov| mov.get("distance_ft").and_then(|d| d.as_i64()))
                .map(|d| d as i32)
        })
        .sum();

    let overlays: Vec<(String, Option<f64>, Option<f64>, Option<i32>)> = sqlx::query_as(
        "select zone_type, origin_x, origin_y, radius_ft from encounter_overlays
         where active = true and encounter_id = $1 and zone_type = 'difficult_terrain'")
        .bind(snap.encounter_id).fetch_all(&s.db).await?;
    let in_difficult = overlays.iter().any(|(zt, ox, oy, rad)| {
        if let (Some(cx), Some(cy)) = (ox, oy) {
            let dx = x - *cx as f32; let dy = y - *cy as f32;
            let in_zone = if let Some(r) = rad { (dx*dx + dy*dy).sqrt() < (*r as f32) }
                         else { (dx*dx + dy*dy).sqrt() < 5.0 };
            in_zone && zt == "difficult_terrain"
        } else { false }
    });
    let move_cost = if in_difficult {
        let ignores_difficult = snap.active_effects.iter().any(|e| {
            e.modifiers.as_object()
                .map(|m| m.get("ignore_difficult_terrain").is_some())
                .unwrap_or(false)
        });
        if ignores_difficult { dist_ft } else { dist_ft * 2 }
    } else { dist_ft };

    let speed_cap = speed + dash_bonus;

    let c: Option<Combatant> = if is_player_in_active {
        sqlx::query_as::<_, Combatant>(
            r#"update combatants c set token_x = $2, token_y = $3, token_on_map = true,
                   token_moved_round = e.round, movement_used_ft = c.movement_used_ft + $4
               from encounters e
               where c.id = $1 and c.encounter_id = e.id
                 and (c.token_x is not null)
                 and (c.token_moved_round is null or c.token_moved_round < e.round
                   or exists (select 1 from combatant_effects ce where ce.combatant_id = c.id
                     and ce.active = true and ce.modifiers @> '{"movement": {}}'::jsonb
                     and not (ce.modifiers @> '{"movement": {"type": "dash_bonus"}}'::jsonb)))
                 and (c.movement_used_ft + $4 <= $5 or $5 <= 0)
               returning c.id, c.encounter_id, c.ref_type::text as ref_type, c.character_id, c.npc_id, c.display_name,
                         c.initiative, c.dex_tiebreaker, c.hp_current, c.hp_max, c.temp_hp, c.ac, c.conditions, c.notes, c.is_visible, c.turn_order, c.initiative_rolled,
                         c.token_x, c.token_y, c.token_color, c.token_on_map, c.token_image, null::text as portrait_url, c.token_moved_round,
                         c.action_used, c.bonus_action_used, c.reaction_used, c.movement_used_ft,
                         c.legendary_actions_max, c.legendary_actions_used, c.legendary_resistances_max, c.legendary_resistances_used,
                         c.readied_action, c.cover_bonus, c.delayed_turn, c.action_spell_level, c.bonus_action_spell_level, c.last_hit_attack_total, c.last_hit_damage, c.last_hit_attacker, c.spell_being_cast"#)
            .bind(id).bind(x).bind(y).bind(move_cost).bind(speed_cap).fetch_optional(&s.db).await?
    } else {
        sqlx::query_as::<_, Combatant>(
            r#"update combatants set token_x = $2, token_y = $3, token_on_map = true,
                   movement_used_ft = movement_used_ft + $4 where id = $1
               returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                         initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                         token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                     readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast, level_override, vision_range"#)
            .bind(id).bind(x).bind(y).bind(move_cost).fetch_optional(&s.db).await?
    };
    let c = c.ok_or_else(|| AppError::BadRequest(
        if is_player_in_active && speed > 0 && move_cost > 0 {
            "already moved this round or not enough movement".into()
        } else { "already moved this round".into() }
    ))?;
    ws::publish(campaign_id, json!({
        "type":"combatant_moved","id":id,"x":x,"y":y,"token_moved_round":c.token_moved_round,"movement_used_ft":c.movement_used_ft
    }).to_string());

    auto_trigger_ready_actions_for_event(&s.db, campaign_id, c.encounter_id,
        "target_enters_range", id, id).await;
    Ok(Json(c))
}

pub async fn use_action(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UseAction>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Uuid, String, Option<Uuid>) = sqlx::query_as(
        "select c.id, e.campaign_id, c.ref_type::text, ch.owner_id \
         from combatants c join encounters e on e.id = c.encounter_id \
         left join characters ch on ch.id = c.character_id where c.id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let campaign_id = row.1; let ref_type = row.2; let owner = row.3;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master {
        if ref_type != "character" || owner != Some(uid) { return Err(AppError::Forbidden); }
    }

    let c: Combatant = match body.action.as_str() {
        "action" => sqlx::query_as::<_, Combatant>(
            "update combatants set action_used = not action_used where id = $1
             returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                       initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                       token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                       action_used, bonus_action_used, reaction_used, movement_used_ft,
                       legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                     readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast, level_override, vision_range")
            .bind(id).fetch_one(&s.db).await?,
        "bonus_action" => sqlx::query_as::<_, Combatant>(
            "update combatants set bonus_action_used = not bonus_action_used where id = $1
             returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                       initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                       token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                       action_used, bonus_action_used, reaction_used, movement_used_ft,
                       legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                     readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast, level_override, vision_range")
            .bind(id).fetch_one(&s.db).await?,
        "reaction" => sqlx::query_as::<_, Combatant>(
            "update combatants set reaction_used = not reaction_used where id = $1
             returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                       initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                       token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                       action_used, bonus_action_used, reaction_used, movement_used_ft,
                       legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                     readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast, level_override, vision_range")
            .bind(id).fetch_one(&s.db).await?,
        "legendary_action" => sqlx::query_as::<_, Combatant>(
            "update combatants set legendary_actions_used = least(legendary_actions_max, legendary_actions_used + 1) where id = $1
             returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                       initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                       token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                       action_used, bonus_action_used, reaction_used, movement_used_ft,
                       legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                     readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast, level_override, vision_range")
            .bind(id).fetch_one(&s.db).await?,
        "legendary_resistance" => sqlx::query_as::<_, Combatant>(
            "update combatants set legendary_resistances_used = least(legendary_resistances_max, legendary_resistances_used + 1) where id = $1
             returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                       initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                       token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                       action_used, bonus_action_used, reaction_used, movement_used_ft,
                       legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                     readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast, level_override, vision_range")
            .bind(id).fetch_one(&s.db).await?,
        _ => return Err(AppError::BadRequest("action must be action|bonus_action|reaction|legendary_action|legendary_resistance".into())),
    };

    ws::publish(campaign_id, json!({"type":"combatant_updated","id":id}).to_string());
    Ok(Json(c))
}
