// Class feature handler: action_surge, second_wind, rage, lay_on_hands, uncanny_dodge, smite.
use super::*;
use super::super::actions::sync_combatant_hp_to_sheet;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct ClassFeatureBody {
    #[validate(length(min = 1, max = 32))]
    pub feature: String,
    #[serde(alias = "_target_id")]
    pub target_id: Option<Uuid>,
    /// Smite: spell slot level to consume (1-5). None for non-smite features.
    #[validate(range(min = 1, max = 5))]
    pub slot_level: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct ClassFeatureResult {
    pub feature: String,
    pub success: bool,
    pub message: String,
    pub hp_after: Option<i32>,
    pub effect_applied: bool,
    /// Smite-only: radiant damage dealt (rolled) + slot consumed.
    pub smite_damage: Option<i32>,
    pub smite_extra_undead: Option<i32>,
    pub smite_slot_consumed: Option<i32>,
}

pub async fn class_feature(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ClassFeatureBody>,
) -> AppResult<Json<ClassFeatureResult>> {
    let row: (Uuid, Option<Uuid>, String, Option<Uuid>, Uuid) = sqlx::query_as(
        r#"select e.campaign_id, ch.owner_id, e.status::text, c.character_id, c.encounter_id
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let (campaign_id, owner, status, character_id, id_encounter) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }
    if status != "active" {
        return Err(AppError::Conflict("encounter not active".into()));
    }

    let feature = body.feature.to_lowercase();
    let message: String;
    let mut hp_after = None;
    let mut smite_damage = None;
    let mut smite_extra_undead = None;
    let mut smite_slot_consumed = None;
    let effect_applied: bool;

    match feature.as_str() {
        "action_surge" => {
            sqlx::query("update combatants set action_used = false where id = $1")
                .bind(id)
                .execute(&s.db)
                .await?;
            message = "Action Surge! You can take an additional action.".into();
            effect_applied = true;
        }
        "second_wind" => {
            if let Some(chid) = character_id {
                let mut tx = s.db.begin().await?;
                sqlx::query("select id from combatants where id = $1 for update")
                    .bind(id)
                    .fetch_optional(&mut *tx)
                    .await?
                    .ok_or(AppError::NotFound)?;
                let consumed: Option<Uuid> = sqlx::query_scalar(
                    "update combatants set bonus_action_used = true where id = $1 and bonus_action_used = false returning id")
                    .bind(id).fetch_optional(&mut *tx).await?;
                if consumed.is_none() {
                    return Err(AppError::BadRequest("bonus action already used".into()));
                }
                let fighter_level: i32 = sqlx::query_scalar(
                    "select coalesce((sheet->>'level_total')::int, 1) from characters where id = $1")
                    .bind(chid).fetch_one(&mut *tx).await?;
                let mut rng = rand::rngs::StdRng::from_os_rng();
                let roll = crate::dice::roll(&format!("1d10+{}", fighter_level), &mut rng)
                    .map_err(|e| AppError::BadRequest(e.to_string()))?;
                let heal = roll.total;
                let (hp_cur, hp_max, temp_hp): (i32, i32, i32) = sqlx::query_as(
                    "select hp_current, hp_max, temp_hp from combatants where id = $1",
                )
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
                if hp_cur >= hp_max {
                    return Err(AppError::BadRequest("already at full HP".into()));
                }
                let new_hp = (hp_cur + heal).min(hp_max);
                sqlx::query("update combatants set hp_current = $1 where id = $2")
                    .bind(new_hp)
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
                if let Err(e) =
                    super::super::actions::sync_combatant_hp_to_sheet(&s.db, id, new_hp, temp_hp)
                        .await
                {
                    tracing::error!(combatant_id = %id, "sync sheet HP: {e}");
                }
                hp_after = Some(new_hp);
                message = format!("Second Wind heals {} HP", heal);
                effect_applied = true;
            } else {
                return Err(AppError::BadRequest(
                    "Second Wind requires a linked character".into(),
                ));
            }
        }
        "rage" => {
            let chid = character_id.ok_or(AppError::BadRequest(
                "rage requires a linked character".into(),
            ))?;
            let barbarian_level: Option<i32> = sqlx::query_scalar(
                r#"select (elem->>'level')::int
                   from characters, jsonb_array_elements(sheet->'classes') as elem
                   where id = $1 and lower(elem->>'name') = 'barbarian'
                   limit 1"#,
            )
            .bind(chid)
            .fetch_optional(&s.db)
            .await?
            .flatten();
            let barbarian_level = barbarian_level.ok_or_else(|| AppError::BadRequest(
                "only barbarians can rage".into(),
            ))?;
            let rage_dmg_bonus = if barbarian_level >= 16 {
                4
            } else if barbarian_level >= 9 {
                3
            } else {
                2
            };

            sqlx::query("update combatant_effects set active = false where combatant_id = $1 and name = 'Rage' and active = true")
                .bind(id).execute(&s.db).await?;

            let rage_mods = serde_json::json!({
                "damage_bonus": rage_dmg_bonus,
                "damage_resistance": ["bludgeoning", "piercing", "slashing"],
                "attack_advantage": true
            });
            sqlx::query(
                r#"insert into combatant_effects
                   (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
                    concentration, active, modifiers, source_type)
                   values ($1, 'Rage', 'buff', 'swords', 'manual', null, null, 'round_end',
                           false, true, $2, 'ability')"#)
                .bind(id).bind(rage_mods).execute(&s.db).await?;

            let mut conditions: Vec<String> =
                sqlx::query_scalar("select conditions from combatants where id = $1")
                    .bind(id)
                    .fetch_one(&s.db)
                    .await?;
            if !super::super::has_condition(&conditions, "rage") {
                conditions.push("rage".to_string());
            }
            let updated: Option<Uuid> = sqlx::query_scalar(
                "update combatants set conditions = $1, bonus_action_used = true
                 where id = $2 and bonus_action_used = false returning id",
            )
            .bind(&conditions)
            .bind(id)
            .fetch_optional(&s.db)
            .await?;
            if updated.is_none() {
                return Err(AppError::BadRequest("bonus action already used".into()));
            }
            message = format!(
                "Rage! +{} damage, BPS resistance, STR advantage.",
                rage_dmg_bonus
            );
            effect_applied = true;
        }
        "lay_on_hands" => {
            let target_id = body.target_id.ok_or(AppError::BadRequest(
                "target_id required for Lay on Hands".into(),
            ))?;
            let chid = character_id.ok_or(AppError::BadRequest(
                "Lay on Hands requires a linked character".into(),
            ))?;

            // M17: target must be in the same encounter as the caster
            let target_enc: Option<Uuid> =
                sqlx::query_scalar("select encounter_id from combatants where id = $1")
                    .bind(target_id)
                    .fetch_optional(&s.db)
                    .await?;
            let target_enc = target_enc.ok_or(AppError::NotFound)?;
            if target_enc != id_encounter {
                return Err(AppError::BadRequest(
                    "Lay on Hands target must be in the same encounter".into(),
                ));
            }

            let mut tx = s.db.begin().await?;
            // Lock pool row + target row so concurrent heals can't double-spend
            // pool or over-heal target.
            sqlx::query("select id from characters where id = $1 for update")
                .bind(chid)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(AppError::NotFound)?;
            sqlx::query("select id from combatants where id = $1 for update")
                .bind(target_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(AppError::NotFound)?;
            let pool: Option<serde_json::Value> = sqlx::query_scalar(
                r#"select elem from characters, jsonb_array_elements(sheet->'resources') as elem
                   where id = $1 and lower(elem->>'name') like '%lay on hands%'
                   limit 1"#,
            )
            .bind(chid)
            .fetch_optional(&mut *tx)
            .await?;
            let (pool_current, _pool_id): (i32, String) = if let Some(p) = pool {
                let cur = p.get("current").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let rid = p
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                (cur, rid)
            } else {
                return Err(AppError::BadRequest(
                    "No Lay on Hands pool found on character sheet".into(),
                ));
            };
            if pool_current <= 0 {
                return Err(AppError::BadRequest("Lay on Hands pool is empty".into()));
            }

            let (hp_cur, hp_max, temp_hp): (i32, i32, i32) = sqlx::query_as(
                "select hp_current, hp_max, temp_hp from combatants where id = $1",
            )
            .bind(target_id)
            .fetch_one(&mut *tx)
            .await?;
            let missing = (hp_max - hp_cur).max(0);
            let heal_amt = pool_current.min(missing).max(1);
            let new_hp = (hp_cur + heal_amt).min(hp_max);

            sqlx::query(
                r#"update characters set sheet = jsonb_set(
                     sheet,
                     ('{resources,' || idx - 1 || ',current}')::text[],
                     to_jsonb($2::int)
                   )
                   from (select position - 1 as idx
                         from characters, jsonb_array_elements(sheet->'resources') with ordinality as t(elem, position)
                         where id = $1 and lower(t.elem->>'name') like '%lay on hands%'
                         limit 1) sub
                   where id = $1"#)
                .bind(chid).bind(pool_current - heal_amt).execute(&mut *tx).await?;

            sqlx::query("update combatants set hp_current = $1 where id = $2")
                .bind(new_hp)
                .bind(target_id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            if let Err(e) = super::super::actions::sync_combatant_hp_to_sheet(
                &s.db,
                target_id,
                new_hp,
                temp_hp,
            )
            .await
            {
                tracing::error!(combatant_id = %target_id, "sync sheet HP: {e}");
            }

            hp_after = Some(new_hp);
            message = format!(
                "Lay on Hands heals {} HP (pool: {} remaining)",
                heal_amt,
                pool_current - heal_amt
            );
            effect_applied = true;
        }
        "uncanny_dodge" => {
            let mut tx = s.db.begin().await?;
            sqlx::query("select id from combatants where id = $1 for update")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(AppError::NotFound)?;
            let consumed: Option<Uuid> = sqlx::query_scalar(
                "update combatants set reaction_used = true where id = $1 and reaction_used = false and hp_current > 0 returning id")
                .bind(id).fetch_optional(&mut *tx).await?;
            if consumed.is_none() {
                return Err(AppError::BadRequest(
                    "reaction already used or cannot act".into(),
                ));
            }
            // PHB: Uncanny Dodge halves incoming attack damage. Read from pending_hits queue
            // (FIFO) so multiple hits in the same round don't all trigger on the same stale value.
            let row: (serde_json::Value, i32) = sqlx::query_as(
                "select pending_hits, hp_current from combatants where id = $1",
            )
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
            let (pending_raw, hp_cur) = row;
            let mut hits: Vec<serde_json::Value> =
                pending_raw.as_array().cloned().unwrap_or_default();
            let hit = hits.last().cloned();
            let final_dmg: i32 = if let Some(h) = &hit {
                h.get("damage")
                    .and_then(|v| v.as_i64())
                    .map(|v| v as i32)
                    .unwrap_or(0)
            } else {
                // Fallback: legacy last_hit_damage column
                sqlx::query_scalar("select last_hit_damage from combatants where id = $1")
                    .bind(id)
                    .fetch_optional(&mut *tx)
                    .await?
                    .unwrap_or(0)
            };
            // PHB: target takes half damage (floor). Apply halved damage to HP, drop the hit.
            let halve = (final_dmg / 2).max(0);
            let new_hp = (hp_cur - halve).max(0);
            if hit.is_some() {
                hits.pop();
            }
            let new_pending = serde_json::Value::Array(hits);
            sqlx::query("update combatants set hp_current = $1, last_hit_damage = null, pending_hits = $2 where id = $3")
                .bind(new_hp).bind(&new_pending).bind(id).execute(&mut *tx).await?;
            tx.commit().await?;
            message = format!("Uncanny Dodge! Took {} damage ({} halved from {}).", halve, halve, final_dmg);
            effect_applied = true;
        }
        "smite" => {
            // PHB p.85 Divine Smite: 2d8 base + 1d8 per slot level above 1st (max 5d8);
            // +1d8 if target is fiend or undead. Slot consumed.
            let target_id = body.target_id.ok_or(AppError::BadRequest(
                "target_id required for Smite".into(),
            ))?;
            let chid = character_id.ok_or(AppError::BadRequest(
                "Smite requires a linked character".into(),
            ))?;
            let slot_level = body.slot_level.ok_or(AppError::BadRequest(
                "slot_level required for Smite".into(),
            ))?;
            if !(1..=5).contains(&slot_level) {
                return Err(AppError::BadRequest("slot_level must be 1-5".into()));
            }
            // Validate paladin level >= 2 + slot available
            let paladin_level: Option<i32> = sqlx::query_scalar(
                r#"select (elem->>'level')::int
                   from characters, jsonb_array_elements(sheet->'classes') as elem
                   where id = $1 and lower(elem->>'name') = 'paladin'
                   limit 1"#,
            )
            .bind(chid)
            .fetch_optional(&s.db)
            .await?
            .flatten();
            let paladin_level = paladin_level.ok_or_else(|| {
                AppError::BadRequest("only paladins can smite".into())
            })?;
            if paladin_level < 2 {
                return Err(AppError::BadRequest(
                    "Smite requires paladin level 2+".into(),
                ));
            }
            // M17: target must be in same encounter
            let target_enc: Option<Uuid> =
                sqlx::query_scalar("select encounter_id from combatants where id = $1")
                    .bind(target_id)
                    .fetch_optional(&s.db)
                    .await?;
            let target_enc = target_enc.ok_or(AppError::NotFound)?;
            if target_enc != id_encounter {
                return Err(AppError::BadRequest(
                    "Smite target must be in the same encounter".into(),
                ));
            }
            // Atomically check + consume slot
            let mut tx = s.db.begin().await?;
            sqlx::query("select id from characters where id = $1 for update")
                .bind(chid)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(AppError::NotFound)?;
            let slot_key = format!("{}", slot_level);
            let slot_current: Option<i32> = sqlx::query_scalar(
                "select (sheet->'slots'->$1->>'current')::int from characters where id = $2",
            )
            .bind(&slot_key)
            .bind(chid)
            .fetch_optional(&mut *tx)
            .await?;
            let slot_current = slot_current.unwrap_or(0);
            if slot_current <= 0 {
                return Err(AppError::BadRequest(
                    "no spell slots of that level remaining".into(),
                ));
            }
            sqlx::query(
                "update characters set sheet = jsonb_set(sheet, array['slots', $1, 'current'], to_jsonb($2::int)) where id = $3")
                .bind(&slot_key)
                .bind(slot_current - 1)
                .bind(chid)
                .execute(&mut *tx)
                .await?;
            // PHB: 2d8 base + (slot_level - 1)d8, max 5d8; +1d8 if target is fiend or undead.
            let base_dice_count = (1 + slot_level).min(5);
            let base_expr = format!("{}d8", base_dice_count);
            let mut rng = rand::rngs::StdRng::from_os_rng();
            let base_roll = crate::dice::roll(&base_expr, &mut rng)
                .map_err(|e| AppError::BadRequest(format!("smite roll error: {e}")))?;
            let base_dmg = base_roll.total;
            // Check target creature type for +1d8
            let target_npc_type: Option<String> = sqlx::query_scalar(
                "select lower(coalesce(n.stats->>'creature_type', '')) from combatants c left join npcs n on n.id = c.npc_id where c.id = $1",
            )
            .bind(target_id)
            .fetch_optional(&mut *tx)
            .await?
            .flatten();
            let is_undead_or_fiend = matches!(
                target_npc_type.as_deref(),
                Some("undead") | Some("fiend")
            );
            let extra_dmg = if is_undead_or_fiend {
                let r = crate::dice::roll("1d8", &mut rng)
                    .map_err(|e| AppError::BadRequest(format!("smite extra roll error: {e}")))?;
                r.total
            } else {
                0
            };
            let total_smite_dmg = base_dmg + extra_dmg;
            // Apply radiant damage
            let (hp_cur, _hp_max, temp_hp): (i32, i32, i32) = sqlx::query_as(
                "select hp_current, hp_max, temp_hp from combatants where id = $1",
            )
            .bind(target_id)
            .fetch_one(&mut *tx)
            .await?;
            let (new_hp, new_temp) =
                combat_engine::apply_hp_damage(hp_cur, temp_hp, total_smite_dmg);
            sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
                .bind(new_hp)
                .bind(new_temp)
                .bind(target_id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            if let Err(e) = sync_combatant_hp_to_sheet(
                &s.db,
                target_id,
                new_hp,
                new_temp,
            )
            .await
            {
                tracing::error!(combatant_id = %target_id, "smite sync sheet HP: {e}");
            }
            let undead_msg = if is_undead_or_fiend { format!(" +{} (undead/fiend)", extra_dmg) } else { String::new() };
            message = format!(
                "Smite! Dealt {} radiant damage to target ({}d8{}).",
                total_smite_dmg, base_dice_count, undead_msg,
            );
            hp_after = Some(new_hp);
            smite_damage = Some(total_smite_dmg);
            smite_extra_undead = if is_undead_or_fiend { Some(extra_dmg) } else { None };
            smite_slot_consumed = Some(slot_level);
            effect_applied = true;
        }
        _ => {
            return Err(AppError::BadRequest(format!(
                "unknown class feature: {}",
                body.feature
            )));
        }
    }

    ws::publish(
        campaign_id,
        json!({
            "type": "combatant_uses_class_feature",
            "combatant_id": id,
            "feature": feature,
            "message": &message,
            "hp_after": hp_after,
        })
        .to_string(),
    );

    Ok(Json(ClassFeatureResult {
        feature: body.feature,
        success: effect_applied,
        message,
        hp_after,
        effect_applied,
        smite_damage,
        smite_extra_undead,
        smite_slot_consumed,
    }))
}
