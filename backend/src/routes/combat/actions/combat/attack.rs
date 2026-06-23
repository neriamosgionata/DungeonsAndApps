// Attack handler — main combat attack endpoint.
// Split into pre-tx modifier computation, resolve, and post-tx application.
use super::*;
use super::super::economy::require_action_auth;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use uuid::Uuid;
use validator::Validate;


#[derive(Debug, Deserialize, Validate)]
pub struct AttackBody {
    pub target_id: Uuid,
    #[validate(length(max = 64))]
    pub attack_expression: Option<String>,
    #[validate(length(max = 64))]
    pub damage_expression: Option<String>,
    #[validate(length(min = 1, max = 32))]
    pub damage_type: String,
    #[validate(length(max = 16))]
    pub damage_die: Option<String>,
    #[validate(length(max = 8))]
    pub ability: Option<String>,
    pub proficient: Option<bool>,
    pub advantage: bool,
    pub disadvantage: bool,
    #[validate(length(max = 16))]
    pub cover: Option<String>,
    pub is_spell_attack: bool,
    pub is_magical: bool,
    #[validate(length(max = 80))]
    pub label: Option<String>,
    #[validate(length(max = 64))]
    pub weapon_id: Option<String>,
    #[validate(length(max = 64))]
    pub extra_damage_expression: Option<String>,
    #[validate(length(max = 32))]
    pub extra_damage_type: Option<String>,
    pub power_attack: Option<bool>,
    pub skip_ammo: Option<bool>,
    pub reckless: Option<bool>,
    pub bless_dice: Option<i32>,
    pub bardic_inspiration_dice: Option<i32>,
}

#[tracing::instrument(skip(s, body), fields(uid = %uid, attacker_id = %id))]
pub async fn attack(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<AttackBody>,
) -> AppResult<Json<combat_engine::AttackResult>> {
    body.validate()
        .map_err(|e| AppError::BadRequest(format!("invalid body: {e}")))?;
    // MED-4: auth + status + role + owner in one query (was 4 separate
    // queries: campaign_id, status, require_member, owner). The encounter
    // ownership check below ensures the target is in the same encounter.
    let auth = require_action_auth(&s.db, uid, id).await?;
    let campaign_id = auth.campaign_id;

    // MED-4 (cont): cache map_grid_size once. Pre-fix code re-queried it in
    // both the range check (line 178) and the flanking check (line 281).
    let map_grid_size: i32 = sqlx::query_scalar("select map_grid_size from encounters where id = $1")
        .bind(auth.encounter_id)
        .fetch_optional(&s.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let attacker_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let target_snap = combat_engine::load_snapshot(&s.db, body.target_id).await?;

    if attacker_snap.encounter_id != target_snap.encounter_id {
        return Err(AppError::BadRequest(
            "attacker and target not in same encounter".into(),
        ));
    }

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

    // F10: merge 3 overlapping encounter-wide combatant scans (5ft, cover,
    // flanking) into 1 query. 50-combatant encounter = 1 round-trip instead
    // of 3. Filter in memory — the filters differ slightly per use site
    // (initiative_rolled for 5ft; token_on_map+hp_current for cover/flank;
    // side for flank; not target for cover/flank).
    #[derive(sqlx::FromRow)]
    struct OtherToken {
        id: Uuid,
        ref_type: String,
        token_x: Option<f32>,
        token_y: Option<f32>,
        hp_current: i32,
        token_on_map: bool,
        initiative_rolled: bool,
    }
    let others: Vec<OtherToken> = sqlx::query_as(
        r#"select id, ref_type::text as ref_type,
                  token_x::float8 as token_x, token_y::float8 as token_y,
                  hp_current, token_on_map, initiative_rolled
           from combatants
           where encounter_id = $1 and id != $2"#,
    )
    .bind(attacker_snap.encounter_id)
    .bind(id)
    .fetch_all(&s.db)
    .await?;

    if weapon_props.ranged || weapon_props.thrown {
        // 5ft check: any enemy within 5ft of attacker imposes dis on ranged.
        if let (Some(ax), Some(ay)) = (attacker_snap.token_x, attacker_snap.token_y) {
            let within_5ft = others.iter().any(|o| {
                if !o.initiative_rolled { return false; }
                if let (Some(x), Some(y)) = (o.token_x, o.token_y) {
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
                                // HIGH-4: 1 cell = 5ft = 20% of the map.
                                let _ = map_grid_size;
                                let dx = (ax - tx) as f32;
                                let dy = (ay - ty) as f32;
                                let dist_ft = (dx * dx + dy * dy).sqrt() * 0.25;
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
        // Cover blockers: filter `others` (already fetched) by token_on_map,
        // hp_current>0, exclude target. Original WHERE matches.
        if let (Some(ax), Some(ay)) = (attacker_snap.token_x, attacker_snap.token_y) {
            if let (Some(tx), Some(ty)) = (target_snap.token_x, target_snap.token_y) {
                let mut max_cover = 0i32;
                for o in &others {
                    if o.id == body.target_id { continue; }
                    if !o.token_on_map || o.hp_current <= 0 { continue; }
                    let ox = o.token_x.unwrap_or(50.0);
                    let oy = o.token_y.unwrap_or(50.0);
                    if super::is_between(ox, oy, ax, ay, tx, ty) {
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
                        if super::segments_intersect(ax, ay, tx, ty, *wx1, *wy1, *wx2, *wy2) {
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
        // Flanking: same filter as cover. Use the same `others` Vec.
        if let (Some(ax), Some(ay)) = (attacker_snap.token_x, attacker_snap.token_y) {
            if let (Some(tx), Some(ty)) = (target_snap.token_x, target_snap.token_y) {
                let attacker_side = if attacker_snap.character_id.is_some() {
                    "ally"
                } else {
                    "enemy"
                };
                let grid_size = map_grid_size;
                for o in &others {
                    if o.id == body.target_id { continue; }
                    if !o.token_on_map || o.hp_current <= 0 { continue; }
                    let side = if o.ref_type == "character" { "ally" } else { "enemy" };
                    if side != attacker_side { continue; }
                    let ox = o.token_x.unwrap_or(50.0);
                    let oy = o.token_y.unwrap_or(50.0);
                    if super::is_flanking(ax, ay, ox, oy, tx, ty, grid_size) {
                        adv = true;
                        break;
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

    // Delegate post-resolution tx + ws to helper.
    apply_attack_outcome(
        &s,
        &attacker_snap,
        &target_snap,
        weapon.map(|(v, p)| (v.clone(), p.clone())),
        id,
        body.target_id,
        body.skip_ammo.unwrap_or(false),
        &result,
        campaign_id,
        is_reckless,
        &req,
    )
    .await?;

    Ok(Json(result))
}
