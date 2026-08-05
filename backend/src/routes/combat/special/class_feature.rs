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
    let row: (Uuid, Option<Uuid>, String, Option<Uuid>, Uuid, i32, i32) = sqlx::query_as(
        r#"select e.campaign_id, ch.owner_id, e.status::text, c.character_id, c.encounter_id, e.round, e.turn_index
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let (campaign_id, owner, status, character_id, id_encounter, enc_round, enc_turn_index) = row;
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
            let mut tx = s.db.begin().await?;
            let already_used: Option<Uuid> = sqlx::query_scalar(
                "select id from combatant_effects
                 where combatant_id = $1 and name = 'Action Surge' and active = true
                 limit 1"
            )
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
            if already_used.is_some() {
                return Err(AppError::BadRequest(
                    "Action Surge already used this rest (clear via short rest to reuse)".into(),
                ));
            }
            sqlx::query("update combatants set action_used = false where id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            sqlx::query(
                "insert into combatant_effects
                 (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
                  concentration, active, modifiers, source_type, applied_at_round, applied_at_turn_index)
                 values ($1, 'Action Surge', 'buff', 'zap', 'rounds', 1, 1, 'round_end',
                         false, true, '{}', 'ability', $2, $3)"
            )
            .bind(id)
            .bind(enc_round)
            .bind(enc_turn_index)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
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
                    r#"select coalesce(sum((elem->>'level')::int), 0)
                       from characters, jsonb_array_elements(sheet->'classes') as elem
                       where id = $1 and lower(elem->>'name') = 'fighter'"#)
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
            // PHB p.48: can't rage in heavy armor
            let armor_type: String = sqlx::query_scalar(
                "select lower(coalesce(sheet->'armor'->>'type', '')) from characters where id = $1",
            )
            .bind(chid)
            .fetch_one(&s.db)
            .await?;
            if armor_type == "heavy" {
                return Err(AppError::BadRequest(
                    "cannot rage while wearing heavy armor".into(),
                ));
            }
            let rage_dmg_bonus = if barbarian_level >= 16 {
                4
            } else if barbarian_level >= 9 {
                3
            } else {
                2
            };

            let mut tx = s.db.begin().await?;
            sqlx::query("update combatant_effects set active = false where combatant_id = $1 and name = 'Rage' and active = true")
                .bind(id).execute(&mut *tx).await?;

            let rage_mods = serde_json::json!({
                "damage_bonus": rage_dmg_bonus,
                "damage_resistance": ["bludgeoning", "piercing", "slashing"],
                "attack_advantage": true
            });
            sqlx::query(
                r#"insert into combatant_effects
                   (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
                    concentration, active, modifiers, source_type, applied_at_round, applied_at_turn_index)
                   values ($1, 'Rage', 'buff', 'swords', 'rounds', 10, 10, 'round_end',
                           false, true, $2, 'ability', $3, $4)"#)
                .bind(id).bind(rage_mods).bind(enc_round).bind(enc_turn_index).execute(&mut *tx).await?;

            let mut conditions: Vec<String> =
                sqlx::query_scalar("select conditions from combatants where id = $1")
                    .bind(id)
                    .fetch_one(&mut *tx)
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
            .fetch_optional(&mut *tx)
            .await?;
            if updated.is_none() {
                return Err(AppError::BadRequest("bonus action already used".into()));
            }
            tx.commit().await?;
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

            // H6: exhaustion 6 = dead (PHB p.291) — healing cannot revive.
            let target_snap = combat_engine::load_snapshot(&s.db, target_id).await?;
            let target_stats = combat_engine::compute_stats(&target_snap);
            if target_stats.exhaustion_dead {
                return Err(AppError::BadRequest(
                    "Lay on Hands target is dead (exhaustion 6)".into(),
                ));
            }
            // H9 consistency: 3 failed death saves + alive=false = dead.
            let sheet_alive = target_snap
                .sheet_raw
                .get("alive")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let sheet_fails = target_snap
                .sheet_raw
                .get("death_saves")
                .and_then(|d| d.get("failures"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            if !sheet_alive && sheet_fails >= 3 {
                return Err(AppError::BadRequest(
                    "Lay on Hands target is dead".into(),
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
                "update combatants set reaction_used = true where id = $1 and reaction_used = false and hp_current > 0 and not ('surprised' = any(conditions)) returning id")
                .bind(id).fetch_optional(&mut *tx).await?;
            if consumed.is_none() {
                return Err(AppError::BadRequest(
                    "reaction already used or cannot act".into(),
                ));
            }
            // PHB: Uncanny Dodge halves incoming attack damage. Read from pending_hits queue
            // (FIFO) so multiple hits in the same round don't all trigger on the same stale value.
            let row: (serde_json::Value, i32, i32) = sqlx::query_as(
                "select pending_hits, hp_current, hp_max from combatants where id = $1",
            )
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
            let (pending_raw, hp_cur, hp_max) = row;
            let mut hits: Vec<serde_json::Value> =
                pending_raw.as_array().cloned().unwrap_or_default();
            let hit = hits.first().cloned();
            let final_dmg: i32 = if let Some(h) = &hit {
                h.get("damage")
                    .and_then(|v| v.as_i64())
                    .map(|v| v as i32)
                    .unwrap_or(0)
            } else {
                // Fallback: legacy last_hit_damage column (nullable).
                sqlx::query_scalar::<_, Option<i32>>("select last_hit_damage from combatants where id = $1")
                    .bind(id)
                    .fetch_optional(&mut *tx)
                    .await?
                    .flatten()
                    .unwrap_or(0)
            };
            // PHB: target takes half damage (floor). The attack already applied the
            // full damage; refund the halved remainder. Refund is capped at the
            // ACTUAL HP lost (hp_before - hp_after) so temp HP absorption isn't
            // double-refunded, and at effective max (hp_max column is already the
            // effective max — reduction is applied at sheet→combatant sync).
            let halve = (final_dmg / 2).max(0);
            let hp_lost = hit
                .as_ref()
                .and_then(|h| h.get("hp_before").and_then(|v| v.as_i64()))
                .zip(hit.as_ref().and_then(|h| h.get("hp_after").and_then(|v| v.as_i64())))
                .map(|(b, a)| (b - a).max(0) as i32)
                .unwrap_or(final_dmg);
            let refund = hp_lost.min(final_dmg - halve);
            let new_hp = (hp_cur + refund).min(hp_max.max(1));
            if hit.is_some() {
                hits.remove(0);
            }
            let new_pending = serde_json::Value::Array(hits);
            sqlx::query("update combatants set hp_current = $1, last_hit_damage = null, pending_hits = $2 where id = $3")
                .bind(new_hp).bind(&new_pending).bind(id).execute(&mut *tx).await?;
            tx.commit().await?;
            message = format!("Uncanny Dodge! Took {} damage ({} halved from {}).", halve, halve, final_dmg);
            effect_applied = true;
        }
        "indomitable" => {
            let chid = character_id.ok_or(AppError::BadRequest(
                "Indomitable requires a linked character".into(),
            ))?;
            let fighter_level: i32 = sqlx::query_scalar(
                r#"select coalesce(sum((elem->>'level')::int), 0)
                   from characters, jsonb_array_elements(sheet->'classes') as elem
                   where id = $1 and lower(elem->>'name') = 'fighter'"#,
            )
            .bind(chid)
            .fetch_one(&s.db)
            .await?;
            if fighter_level < 9 {
                return Err(AppError::BadRequest(
                    "Indomitable requires fighter level 9+".into(),
                ));
            }
            let mut tx = s.db.begin().await?;
            let already_used: Option<Uuid> = sqlx::query_scalar(
                "select id from combatant_effects
                 where combatant_id = $1 and name = 'Indomitable' and active = true
                 limit 1",
            )
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
            if already_used.is_some() {
                return Err(AppError::BadRequest(
                    "Indomitable already used this rest (clear via short rest to reuse)".into(),
                ));
            }
            let indomitable_mods = serde_json::json!({
                "save_advantage": true,
            });
            sqlx::query(
                r#"insert into combatant_effects
                   (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
                    concentration, active, modifiers, source_type, applied_at_round, applied_at_turn_index)
                   values ($1, 'Indomitable', 'buff', 'rotate-cw', 'rounds', 1, 1, 'caster_turn_start',
                           false, true, $2, 'ability', $3, $4)"#,
            )
            .bind(id)
            .bind(indomitable_mods)
            .bind(enc_round)
            .bind(enc_turn_index)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            message = "Indomitable! You have advantage on your next saving throw.".into();
            effect_applied = true;
        }
        "flurry_of_blows" => {
            let target_id = body.target_id.ok_or(AppError::BadRequest(
                "target_id required for Flurry of Blows".into(),
            ))?;
            let chid = character_id.ok_or(AppError::BadRequest(
                "Flurry of Blows requires a linked character".into(),
            ))?;
            let monk_level: i32 = sqlx::query_scalar(
                r#"select (elem->>'level')::int
                   from characters, jsonb_array_elements(sheet->'classes') as elem
                   where id = $1 and lower(elem->>'name') = 'monk'
                   limit 1"#,
            )
            .bind(chid)
            .fetch_optional(&s.db)
            .await?
            .flatten()
            .ok_or(AppError::BadRequest("only monks can use Flurry of Blows".into()))?;
            if monk_level < 2 {
                return Err(AppError::BadRequest("Flurry of Blows requires monk level 2+".into()));
            }
            let mut tx = s.db.begin().await?;
            sqlx::query("select id from characters where id = $1 for update")
                .bind(chid)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(AppError::NotFound)?;
            let ba_consumed: Option<Uuid> = sqlx::query_scalar(
                "update combatants set bonus_action_used = true where id = $1 and bonus_action_used = false returning id",
            )
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
            if ba_consumed.is_none() {
                return Err(AppError::BadRequest("bonus action already used".into()));
            }
            // Consume 1 Ki
            let idx: i32 = sqlx::query_scalar(
                r#"select position - 1
                   from characters, jsonb_array_elements(sheet->'resources') with ordinality as t(elem, position)
                   where id = $1 and lower(t.elem->>'name') = 'ki'
                   limit 1"#,
            )
            .bind(chid)
            .fetch_optional(&mut *tx)
            .await?
            .unwrap_or(-1);
            if idx < 0 {
                return Err(AppError::BadRequest("no Ki resource found on character sheet".into()));
            }
            let ki_cur: i32 = sqlx::query_scalar(
                r#"select (elem->>'current')::int
                   from characters, jsonb_array_elements(sheet->'resources') as elem
                   where id = $1 and lower(elem->>'name') = 'ki'
                   limit 1"#,
            )
            .bind(chid)
            .fetch_optional(&mut *tx)
            .await?
            .flatten()
            .unwrap_or(0);
            if ki_cur < 1 {
                return Err(AppError::BadRequest("not enough Ki".into()));
            }
            sqlx::query(
                r#"update characters set sheet = jsonb_set(
                     sheet, ('{resources,' || $2 || ',current}')::text[],
                     to_jsonb($3::int)
                   ) where id = $1"#,
            )
            .bind(chid)
            .bind(idx)
            .bind(ki_cur - 1)
            .execute(&mut *tx)
            .await?;
            // Unarmed strike damage die by monk level
            let unarmed_die = if monk_level >= 17 { "d12" }
                else if monk_level >= 11 { "d10" }
                else if monk_level >= 5 { "d8" }
                else { "d6" };
            let dex_mod: i32 = sqlx::query_scalar(
                "select ((sheet->'abilities'->>'dex')::int - 10) / 2 from characters where id = $1",
            )
            .bind(chid)
            .fetch_optional(&mut *tx)
            .await?
            .flatten()
            .unwrap_or(0);
            let mut rng = rand::rngs::StdRng::from_os_rng();
            let hit_expr = format!("1d20+{}+{}", dex_mod, combat_engine::proficiency_from_level(monk_level));
            let dmg_expr = format!("{}+{}", unarmed_die, dex_mod);
            let target_ac: i32 = sqlx::query_scalar(
                "select ac from combatants where id = $1",
            )
            .bind(target_id)
            .fetch_optional(&mut *tx)
            .await?
            .unwrap_or(12);
            let mut total_dmg = 0i32;
            for _ in 0..2 {
                let hit_roll = crate::dice::roll(&hit_expr, &mut rng)
                    .map_err(|e| AppError::BadRequest(format!("flurry hit roll error: {e}")))?;
                if hit_roll.total >= target_ac {
                    let dmg_roll = crate::dice::roll(&dmg_expr, &mut rng)
                        .map_err(|e| AppError::BadRequest(format!("flurry dmg roll error: {e}")))?;
                    total_dmg += dmg_roll.total;
                }
            }
            let (hp_cur, _hp_max, temp_hp): (i32, i32, i32) = sqlx::query_as(
                "select hp_current, hp_max, temp_hp from combatants where id = $1",
            )
            .bind(target_id)
            .fetch_one(&mut *tx)
            .await?;
            let (new_hp, new_temp) = combat_engine::apply_hp_damage(hp_cur, temp_hp, total_dmg);
            sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
                .bind(new_hp)
                .bind(new_temp)
                .bind(target_id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            if let Err(e) = sync_combatant_hp_to_sheet(&s.db, target_id, new_hp, new_temp).await {
                tracing::error!(combatant_id = %target_id, "flurry sync sheet HP: {e}");
            }
            hp_after = Some(new_hp);
            message = format!(
                "Flurry of Blows! Two unarmed strikes, {} total damage.",
                total_dmg
            );
            effect_applied = true;
        }
        "patient_defense" => {
            let chid = character_id.ok_or(AppError::BadRequest(
                "Patient Defense requires a linked character".into(),
            ))?;
            let monk_level: i32 = sqlx::query_scalar(
                r#"select (elem->>'level')::int
                   from characters, jsonb_array_elements(sheet->'classes') as elem
                   where id = $1 and lower(elem->>'name') = 'monk'
                   limit 1"#,
            )
            .bind(chid)
            .fetch_optional(&s.db)
            .await?
            .flatten()
            .ok_or(AppError::BadRequest("only monks can use Patient Defense".into()))?;
            if monk_level < 2 {
                return Err(AppError::BadRequest("Patient Defense requires monk level 2+".into()));
            }
            let mut tx = s.db.begin().await?;
            sqlx::query("select id from characters where id = $1 for update")
                .bind(chid)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(AppError::NotFound)?;
            let ba_consumed: Option<Uuid> = sqlx::query_scalar(
                "update combatants set bonus_action_used = true where id = $1 and bonus_action_used = false returning id",
            )
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
            if ba_consumed.is_none() {
                return Err(AppError::BadRequest("bonus action already used".into()));
            }
            // Consume 1 Ki
            let idx: i32 = sqlx::query_scalar(
                r#"select position - 1
                   from characters, jsonb_array_elements(sheet->'resources') with ordinality as t(elem, position)
                   where id = $1 and lower(t.elem->>'name') = 'ki'
                   limit 1"#,
            )
            .bind(chid)
            .fetch_optional(&mut *tx)
            .await?
            .unwrap_or(-1);
            if idx < 0 {
                return Err(AppError::BadRequest("no Ki resource found".into()));
            }
            let ki_cur: i32 = sqlx::query_scalar(
                r#"select (elem->>'current')::int
                   from characters, jsonb_array_elements(sheet->'resources') as elem
                   where id = $1 and lower(elem->>'name') = 'ki'
                   limit 1"#,
            )
            .bind(chid)
            .fetch_optional(&mut *tx)
            .await?
            .flatten()
            .unwrap_or(0);
            if ki_cur < 1 {
                return Err(AppError::BadRequest("not enough Ki".into()));
            }
            sqlx::query(
                r#"update characters set sheet = jsonb_set(
                     sheet, ('{resources,' || $2 || ',current}')::text[],
                     to_jsonb($3::int)
                   ) where id = $1"#,
            )
            .bind(chid)
            .bind(idx)
            .bind(ki_cur - 1)
            .execute(&mut *tx)
            .await?;
            // Insert Dodge effect (same pattern as /dodge endpoint)
            sqlx::query(
                "update combatant_effects set active = false where combatant_id = $1 and name = 'Dodge' and active = true",
            )
            .bind(id)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"insert into combatant_effects
                   (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
                    concentration, active, modifiers, source_type, applied_at_round, applied_at_turn_index)
                   values ($1, 'Dodge', 'buff', 'shield', 'rounds', 1, 1, 'caster_turn_start',
                           false, true, '{"attack_disadvantage_against": true, "dex_save_advantage": true}', 'ability', $2, $3)"#,
            )
            .bind(id)
            .bind(enc_round)
            .bind(enc_turn_index)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            message = "Patient Defense! You take the Dodge action as a bonus action.".into();
            effect_applied = true;
        }
        "step_of_the_wind" => {
            let chid = character_id.ok_or(AppError::BadRequest(
                "Step of the Wind requires a linked character".into(),
            ))?;
            let monk_level: i32 = sqlx::query_scalar(
                r#"select (elem->>'level')::int
                   from characters, jsonb_array_elements(sheet->'classes') as elem
                   where id = $1 and lower(elem->>'name') = 'monk'
                   limit 1"#,
            )
            .bind(chid)
            .fetch_optional(&s.db)
            .await?
            .flatten()
            .ok_or(AppError::BadRequest("only monks can use Step of the Wind".into()))?;
            if monk_level < 2 {
                return Err(AppError::BadRequest("Step of the Wind requires monk level 2+".into()));
            }
            // Determine action type from body: dash (default) or disengage
            let action_type = body.target_id.map(|_| "disengage").unwrap_or("dash");
            let mut tx = s.db.begin().await?;
            sqlx::query("select id from characters where id = $1 for update")
                .bind(chid)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(AppError::NotFound)?;
            let ba_consumed: Option<Uuid> = sqlx::query_scalar(
                "update combatants set bonus_action_used = true where id = $1 and bonus_action_used = false returning id",
            )
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
            if ba_consumed.is_none() {
                return Err(AppError::BadRequest("bonus action already used".into()));
            }
            // Consume 1 Ki
            let idx: i32 = sqlx::query_scalar(
                r#"select position - 1
                   from characters, jsonb_array_elements(sheet->'resources') with ordinality as t(elem, position)
                   where id = $1 and lower(t.elem->>'name') = 'ki'
                   limit 1"#,
            )
            .bind(chid)
            .fetch_optional(&mut *tx)
            .await?
            .unwrap_or(-1);
            if idx < 0 {
                return Err(AppError::BadRequest("no Ki resource found".into()));
            }
            let ki_cur: i32 = sqlx::query_scalar(
                r#"select (elem->>'current')::int
                   from characters, jsonb_array_elements(sheet->'resources') as elem
                   where id = $1 and lower(elem->>'name') = 'ki'
                   limit 1"#,
            )
            .bind(chid)
            .fetch_optional(&mut *tx)
            .await?
            .flatten()
            .unwrap_or(0);
            if ki_cur < 1 {
                return Err(AppError::BadRequest("not enough Ki".into()));
            }
            sqlx::query(
                r#"update characters set sheet = jsonb_set(
                     sheet, ('{resources,' || $2 || ',current}')::text[],
                     to_jsonb($3::int)
                   ) where id = $1"#,
            )
            .bind(chid)
            .bind(idx)
            .bind(ki_cur - 1)
            .execute(&mut *tx)
            .await?;
            if action_type == "dash" {
                sqlx::query("update combatants set movement_used_ft = 0 where id = $1")
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
            }
            tx.commit().await?;
            if action_type == "dash" {
                message = "Step of the Wind! You Dash as a bonus action (movement reset).".into();
            } else {
                message = "Step of the Wind! You Disengage as a bonus action.".into();
            }
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
            // L9: defense-in-depth — PHB smite slots are 1-5. Without
            // this check a slot_level of e.g. 9 silently caps to 5 via
            // the .min(5) below, potentially consuming the wrong slot.
            if !(1..=5).contains(&slot_level) {
                return Err(AppError::BadRequest(format!(
                    "smite slot_level must be 1-5, got {slot_level}"
                )));
            }
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
        "trip_attack" | "menacing_attack" | "disarming_attack" | "pushing_attack" | "sweeping_attack" | "riposte" | "goading_attack" => {
            let target_id = body.target_id.ok_or(AppError::BadRequest(
                "target_id required for maneuver".into(),
            ))?;
            let chid = character_id.ok_or(AppError::BadRequest(
                "maneuver requires a linked character".into(),
            ))?;
            // Validate fighter level
            let fighter_level: i32 = sqlx::query_scalar(
                r#"select coalesce(sum((elem->>'level')::int), 0)
                   from characters, jsonb_array_elements(sheet->'classes') as elem
                   where id = $1 and lower(elem->>'name') = 'fighter'"#,
            )
            .bind(chid)
            .fetch_one(&s.db)
            .await?;
            if fighter_level < 3 {
                return Err(AppError::BadRequest("maneuvers require fighter level 3+".into()));
            }
            let mut tx = s.db.begin().await?;
            let sd_roll = consume_superiority_die(&mut *tx, chid, fighter_level).await?;
            // Maneuver DC: 8 + prof + STR or DEX mod (fighter's choice)
            let pb = combat_engine::proficiency_from_level(fighter_level);
            let str_mod: i32 = sqlx::query_scalar(
                "select ((sheet->'abilities'->>'str')::int - 10) / 2 from characters where id = $1",
            )
            .bind(chid)
            .fetch_optional(&s.db)
            .await?
            .flatten()
            .unwrap_or(0);
            let dex_mod: i32 = sqlx::query_scalar(
                "select ((sheet->'abilities'->>'dex')::int - 10) / 2 from characters where id = $1",
            )
            .bind(chid)
            .fetch_optional(&s.db)
            .await?
            .flatten()
            .unwrap_or(0);
            let dc = 8 + pb + str_mod.max(dex_mod);
            let attack_bonus = pb + str_mod.max(dex_mod);
            let target_ac: i32 = sqlx::query_scalar("select ac from combatants where id = $1")
                .bind(target_id)
                .fetch_one(&mut *tx)
                .await?;
            let maneuver = feature.as_str();
            let mut rng = rand::rngs::StdRng::from_os_rng();

            // A2 maneuvers:
            //  - sweeping_attack: weapon attack vs a second creature's AC;
            //    hit deals the SD roll as damage (PHB p.74)
            //  - riposte: reaction — melee attack vs the creature that
            //    missed you; hit deals 1d8 + SD (weapon die approximation)
            //  - trip/menacing/disarming/pushing: SD damage + save or
            //    effect (existing pattern)
            if maneuver == "sweeping_attack" || maneuver == "riposte" {
                let atk = crate::dice::roll(&format!("1d20+{attack_bonus}"), &mut rng)
                    .map_err(|e| AppError::BadRequest(e.to_string()))?;
                if atk.total >= target_ac {
                    let weapon_die = if maneuver == "riposte" {
                        crate::dice::roll("1d8", &mut rng)
                            .map_err(|e| AppError::BadRequest(e.to_string()))?
                            .total
                    } else {
                        0
                    };
                    let total_dmg = sd_roll + weapon_die;
                    let (hp_cur, _hp_max, temp_hp): (i32, i32, i32) = sqlx::query_as(
                        "select hp_current, hp_max, temp_hp from combatants where id = $1",
                    )
                    .bind(target_id)
                    .fetch_one(&mut *tx)
                    .await?;
                    let (new_hp, new_temp) =
                        combat_engine::apply_hp_damage(hp_cur, temp_hp, total_dmg);
                    sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
                        .bind(new_hp)
                        .bind(new_temp)
                        .bind(target_id)
                        .execute(&mut *tx)
                        .await?;
                    message = format!(
                        "{}! {} vs AC {}: {} damage.",
                        if maneuver == "sweeping_attack" { "Sweeping Attack" } else { "Riposte" },
                        atk.total, target_ac, total_dmg,
                    );
                    hp_after = Some(new_hp);
                } else {
                    message = format!(
                        "{} missed ({} vs AC {}).",
                        if maneuver == "sweeping_attack" { "Sweeping Attack" } else { "Riposte" },
                        atk.total, target_ac,
                    );
                }
                tx.commit().await?;
                effect_applied = true;
            } else {
                let (save_ability, condition_name, condition_msg) = match maneuver {
                    "trip_attack" => ("str", "prone", "knocked prone"),
                    "disarming_attack" => ("str", "disarmed", "disarmed (weapon dropped)"),
                    "pushing_attack" => ("str", "", "pushed 15 ft"),
                    "goading_attack" => ("wis", "goaded", "goaded (disadvantage vs others, informational)"),
                    _ => ("wis", "frightened", "frightened"), // menacing_attack
                };
                // Apply superiority die damage to target
                let (hp_cur, _hp_max, temp_hp): (i32, i32, i32) = sqlx::query_as(
                    "select hp_current, hp_max, temp_hp from combatants where id = $1",
                )
                .bind(target_id)
                .fetch_one(&mut *tx)
                .await?;
                let (new_hp, new_temp) =
                    combat_engine::apply_hp_damage(hp_cur, temp_hp, sd_roll);
                sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
                    .bind(new_hp)
                    .bind(new_temp)
                    .bind(target_id)
                    .execute(&mut *tx)
                    .await?;
                // Compute target's save modifier including proficiency
                let target_save_total: i32 = sqlx::query_scalar(
                    r#"select coalesce(
                        (select ((n.stats->'abilities'->> $2)::int - 10) / 2 +
                                case when lower(coalesce(n.stats->'saves'->>$2, 'false')) = 'true'
                                     then coalesce(n.stats->>'pb', '2')::int else 0 end
                         from combatants c2 join npcs n on n.id = c2.npc_id where c2.id = $1),
                        (select ((ch.sheet->'abilities'->> $2)::int - 10) / 2 +
                                case when (ch.sheet->'saves'->>$2)::boolean
                                     then (coalesce((ch.sheet->>'level_total')::int, 1) - 1) / 4 + 2 else 0 end
                         from combatants c2 join characters ch on ch.id = c2.character_id where c2.id = $1),
                        0
                    )"#,
                )
                .bind(target_id)
                .bind(save_ability)
                .fetch_one(&mut *tx)
                .await?;
                let save_roll = crate::dice::roll(&format!("1d20+{}", target_save_total), &mut rng)
                    .map_err(|e| AppError::BadRequest(e.to_string()))?;
                let save_failed = save_roll.total < dc;
                if save_failed {
                    if !condition_name.is_empty() {
                        let mut conds: Vec<String> = sqlx::query_scalar(
                            "select conditions from combatants where id = $1",
                        )
                        .bind(target_id)
                        .fetch_optional(&mut *tx)
                        .await?
                        .unwrap_or_default();
                        if !conds.iter().any(|c| c.split(':').next().unwrap_or(c) == condition_name) {
                            conds.push(format!("{}:1", condition_name));
                            sqlx::query("update combatants set conditions = $1 where id = $2")
                                .bind(&conds)
                                .bind(target_id)
                                .execute(&mut *tx)
                                .await?;
                        }
                    }
                    if maneuver == "pushing_attack" {
                        // Push 15 ft away from the fighter (5 ft = 20% map).
                        let (ax, ay, txp, typ): (Option<f32>, Option<f32>, Option<f32>, Option<f32>) =
                            sqlx::query_as(
                                "select a.token_x, a.token_y, t.token_x, t.token_y
                                 from combatants a, combatants t where a.id = $1 and t.id = $2",
                            )
                            .bind(id)
                            .bind(target_id)
                            .fetch_one(&mut *tx)
                            .await?;
                        if let (Some(ax), Some(ay), Some(txp), Some(typ)) = (ax, ay, txp, typ) {
                            let dx = txp - ax;
                            let dy = typ - ay;
                            let len = (dx * dx + dy * dy).sqrt();
                            if len > 0.001 {
                                let push = 60.0_f32.min(len);
                                let nx = (txp - dx / len * push).clamp(0.0, 100.0);
                                let ny = (typ - dy / len * push).clamp(0.0, 100.0);
                                sqlx::query(
                                    "update combatants set token_x = $1, token_y = $2 where id = $3",
                                )
                                .bind(nx)
                                .bind(ny)
                                .bind(target_id)
                                .execute(&mut *tx)
                                .await?;
                            }
                        }
                    }
                }
                tx.commit().await?;
                let display = match maneuver {
                    "trip_attack" => "Trip Attack",
                    "disarming_attack" => "Disarming Attack",
                    "pushing_attack" => "Pushing Attack",
                    "goading_attack" => "Goading Attack",
                    _ => "Menacing Attack",
                };
                if save_failed {
                    message = format!(
                        "{}! {} damage + target {} (save {} vs DC {}).",
                        display, sd_roll, condition_msg, save_roll.total, dc,
                    );
                } else {
                    message = format!(
                        "{}! {} damage, target saved vs DC {}.",
                        display, sd_roll, dc,
                    );
                }
                hp_after = Some(new_hp);
                effect_applied = true;
            }
        }
        "rally" => {
            // A2: Rally (PHB p.74) — spend a superiority die; an ally within
            // 30 ft gains temp HP equal to the roll + CHA mod.
            let target_id = body.target_id.ok_or(AppError::BadRequest(
                "target_id required for rally".into(),
            ))?;
            let chid = character_id.ok_or(AppError::BadRequest(
                "rally requires a linked character".into(),
            ))?;
            let fighter_level: i32 = sqlx::query_scalar(
                r#"select coalesce(sum((elem->>'level')::int), 0)
                   from characters, jsonb_array_elements(sheet->'classes') as elem
                   where id = $1 and lower(elem->>'name') = 'fighter'"#,
            )
            .bind(chid)
            .fetch_one(&s.db)
            .await?;
            if fighter_level < 3 {
                return Err(AppError::BadRequest("maneuvers require fighter level 3+".into()));
            }
            let mut tx = s.db.begin().await?;
            let sd = consume_superiority_die(&mut *tx, chid, fighter_level).await?;
            let cha_mod: i32 = sqlx::query_scalar(
                "select ((sheet->'abilities'->>'cha')::int - 10) / 2 from characters where id = $1",
            )
            .bind(chid)
            .fetch_one(&mut *tx)
            .await?;
            let temp = (sd + cha_mod).max(0);
            let (_, _, temp_hp): (i32, i32, i32) = sqlx::query_as(
                "select hp_current, hp_max, temp_hp from combatants where id = $1",
            )
            .bind(target_id)
            .fetch_one(&mut *tx)
            .await?;
            // Temp HP: highest-wins (PHB p.198).
            let new_temp = temp_hp.max(temp);
            sqlx::query("update combatants set temp_hp = $1 where id = $2")
                .bind(new_temp)
                .bind(target_id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            message = format!("Rally! {} gains {} temp HP (d{} + {} CHA).", target_id, temp, if fighter_level >= 18 { 12 } else if fighter_level >= 10 { 10 } else { 8 }, cha_mod);
            effect_applied = true;
        }
        "countercharm" => {
            // A6: Countercharm (PHB p.53) — Bard 6+ uses an ACTION; ally
            // creatures within 30 ft gain advantage on charm/frightened
            // saves until the start of your next turn. Approximation:
            // save_advantage applies to ALL saves (per-save targeting not
            // available in the effect model).
            let chid = character_id.ok_or(AppError::BadRequest(
                "Countercharm requires a linked character".into(),
            ))?;
            let bard_level: i32 = sqlx::query_scalar(
                r#"select coalesce(sum((elem->>'level')::int), 0)
                   from characters, jsonb_array_elements(sheet->'classes') as elem
                   where id = $1 and lower(elem->>'name') = 'bard'"#,
            )
            .bind(chid)
            .fetch_one(&s.db)
            .await?;
            if bard_level < 6 {
                return Err(AppError::BadRequest(
                    "Countercharm requires bard level 6+".into(),
                ));
            }
            let (ax, ay): (Option<f32>, Option<f32>) = sqlx::query_as(
                "select token_x, token_y from combatants where id = $1",
            )
            .bind(id)
            .fetch_one(&s.db)
            .await?;
            // Allies: character-bound combatants + NPCs that aren't hostile.
            let allies: Vec<Uuid> = sqlx::query_scalar(
                "select c.id from combatants c
                 left join npcs n on n.id = c.npc_id
                 where c.encounter_id = $1 and c.id != $2 and c.hp_current > 0
                   and (c.character_id is not null or coalesce(lower(n.faction), '') <> 'hostile')",
            )
            .bind(id_encounter)
            .bind(id)
            .fetch_all(&s.db)
            .await?;
            let mut tx = s.db.begin().await?;
            // Consume the action (Countercharm is an action, PHB p.53).
            let action_consumed: Option<Uuid> = sqlx::query_scalar(
                "update combatants set action_used = true where id = $1 and action_used = false returning id",
            )
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
            if action_consumed.is_none() {
                return Err(AppError::BadRequest("action already used".into()));
            }
            let mut affected = 0i32;
            for ally in allies {
                let (txp, typ): (Option<f32>, Option<f32>) = sqlx::query_as(
                    "select token_x, token_y from combatants where id = $1",
                )
                .bind(ally)
                .fetch_one(&mut *tx)
                .await?;
                match (ax, ay, txp, typ) {
                    (Some(ax), Some(ay), Some(txp), Some(typ)) => {
                        let dx = txp - ax;
                        let dy = typ - ay;
                        // 30 ft = 120 percent units (5 ft = 20%).
                        if (dx * dx + dy * dy).sqrt() > 120.0 {
                            continue;
                        }
                    }
                    _ => {}
                }
                sqlx::query(
                    r#"insert into combatant_effects
                       (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
                        concentration, active, modifiers, source_type, applied_at_round, applied_at_turn_index)
                       values ($1, 'Countercharm', 'buff', 'music', 'rounds', 1, 1, 'caster_turn_start',
                               false, true, '{"save_advantage": true}', 'ability', $2, $3)"#,
                )
                .bind(ally)
                .bind(enc_round)
                .bind(enc_turn_index)
                .execute(&mut *tx)
                .await?;
                affected += 1;
            }
            tx.commit().await?;
            message = format!(
                "Countercharm! {} ally(s) within 30 ft gain save advantage until your next turn.",
                affected
            );
            effect_applied = true;
        }
        "turn_undead" => {
            let chid = character_id.ok_or(AppError::BadRequest(
                "Turn Undead requires a linked character".into(),
            ))?;
            let cleric_level: i32 = sqlx::query_scalar(
                r#"select coalesce(sum((elem->>'level')::int), 0)
                   from characters, jsonb_array_elements(sheet->'classes') as elem
                   where id = $1 and lower(elem->>'name') = 'cleric'"#,
            )
            .bind(chid)
            .fetch_one(&s.db)
            .await?;
            if cleric_level < 2 {
                return Err(AppError::BadRequest(
                    "Turn Undead requires cleric level 2+".into(),
                ));
            }
            // Compute spell save DC from character sheet
            let pb = combat_engine::proficiency_from_level(cleric_level);
            let wis_mod: i32 = sqlx::query_scalar(
                "select ((sheet->'abilities'->>'wis')::int - 10) / 2 from characters where id = $1",
            )
            .bind(chid)
            .fetch_optional(&s.db)
            .await?
            .flatten()
            .unwrap_or(0);
            let dc = 8 + pb + wis_mod;
            let mut tx = s.db.begin().await?;
            // Find all undead combatants in the same encounter
            let undead: Vec<(Uuid, String)> = sqlx::query_as(
                r#"select c.id, c.display_name
                   from combatants c
                   left join npcs n on n.id = c.npc_id
                   left join characters ch on ch.id = c.character_id
                   where c.encounter_id = $1
                     and (lower(n.stats->>'creature_type') = 'undead'
                          or lower(ch.sheet->>'creature_type') = 'undead')
                     and c.hp_current > 0"#,
            )
            .bind(id_encounter)
            .fetch_all(&mut *tx)
            .await?;
            let mut turned = 0i32;
            let mut destroyed = 0i32;
            // A3: Destroy Undead (PHB p.59) — cleric 5+ instantly destroys
            // undead of CR ≤ threshold on a failed save.
            let destroy_cr: f32 = if cleric_level >= 17 {
                4.0
            } else if cleric_level >= 14 {
                3.0
            } else if cleric_level >= 11 {
                2.0
            } else if cleric_level >= 8 {
                1.0
            } else if cleric_level >= 5 {
                0.5
            } else {
                0.0
            };
            for (uid, _name) in &undead {
                // Compute WIS save from either NPC stats or character sheet
                let wis_mod: i32 = sqlx::query_scalar(
                    r#"select coalesce(
                        (select ((n.stats->'abilities'->>'wis')::int - 10) / 2
                         from combatants c2 join npcs n on n.id = c2.npc_id where c2.id = $1),
                        (select ((ch.sheet->'abilities'->>'wis')::int - 10) / 2
                         from combatants c2 join characters ch on ch.id = c2.character_id where c2.id = $1),
                        0
                    )"#,
                )
                .bind(uid)
                .fetch_one(&mut *tx)
                .await?;
                let mut rng = rand::rngs::StdRng::from_os_rng();
                let roll = crate::dice::roll(&format!("1d20+{}", wis_mod), &mut rng)
                    .map_err(|e| AppError::BadRequest(e.to_string()))?;
                if roll.total < dc {
                    turned += 1;
                    // A3: destroy if the undead's CR is within the cleric's
                    // threshold (NPC stats cr field, "1/4" or "2").
                    if destroy_cr > 0.0 {
                        let cr_str: Option<String> = sqlx::query_scalar(
                            "select n.stats->>'cr' from combatants c2 join npcs n on n.id = c2.npc_id where c2.id = $1",
                        )
                        .bind(uid)
                        .fetch_optional(&mut *tx)
                        .await?
                        .flatten();
                        if let Some(cr_str) = cr_str {
                            let cr_float: f32 = if let Some(pos) = cr_str.find('/') {
                                let num: f32 = cr_str[..pos].trim().parse().unwrap_or(0.0);
                                let den: f32 = cr_str[pos + 1..].trim().parse().unwrap_or(1.0);
                                if den == 0.0 { 0.0 } else { num / den }
                            } else {
                                cr_str.trim().parse().unwrap_or(0.0)
                            };
                            if cr_float <= destroy_cr {
                                destroyed += 1;
                                sqlx::query("update combatants set hp_current = 0 where id = $1")
                                    .bind(uid)
                                    .execute(&mut *tx)
                                    .await?;
                                continue;
                            }
                        }
                    }
                    // Apply turned effect (frightened + fleeing, 1 minute = 10 rounds)
                    let mut conditions: Vec<String> = sqlx::query_scalar(
                        "select conditions from combatants where id = $1",
                    )
                    .bind(uid)
                    .fetch_optional(&mut *tx)
                    .await?
                    .unwrap_or_default();
                    if !conditions.iter().any(|c| c.split(':').next().unwrap_or(c) == "frightened") {
                        conditions.push(format!("frightened:{}", 10));
                        sqlx::query("update combatants set conditions = $1 where id = $2")
                            .bind(&conditions)
                            .bind(uid)
                            .execute(&mut *tx)
                            .await?;
                    }
                }
            }
            // Consume Channel Divinity resource (resource named "Channel Divinity")
            let cd_idx: i32 = sqlx::query_scalar(
                r#"select position - 1
                   from characters, jsonb_array_elements(sheet->'resources') with ordinality as t(elem, position)
                   where id = $1 and lower(t.elem->>'name') like '%channel%divinity%'
                   limit 1"#,
            )
            .bind(chid)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::BadRequest(
                "Channel Divinity resource not found on character sheet".into(),
            ))?;
             let cd_cur: i32 = sqlx::query_scalar(
                r#"select (elem->>'current')::int
                   from characters, jsonb_array_elements(sheet->'resources') as elem
                   where id = $1 and lower(elem->>'name') like '%channel%divinity%'
                   limit 1"#,
            )
            .bind(chid)
            .fetch_optional(&mut *tx)
            .await?
            .flatten()
            .ok_or(AppError::BadRequest(
                "Channel Divinity resource not found on character sheet".into(),
            ))?;
            if cd_cur <= 0 {
                return Err(AppError::BadRequest(
                    "Channel Divinity depleted".into(),
                ));
            }
            sqlx::query(
                r#"update characters set sheet = jsonb_set(
                     sheet, ('{resources,' || $2 || ',current}')::text[],
                     to_jsonb($3::int)
                   ) where id = $1"#,
            )
            .bind(chid)
            .bind(cd_idx)
            .bind(cd_cur - 1)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            message = if destroyed > 0 {
                format!(
                    "Turn Undead! {} destroyed, {} turned (DC {}, WIS save).",
                    destroyed, turned, dc
                )
            } else {
                format!(
                    "Turn Undead! {} undead turned (DC {}, WIS save).",
                    turned, dc
                )
            };
            effect_applied = true;
        }
        "wild_shape" => {
            let npc_id = body.target_id.ok_or(AppError::BadRequest(
                "target_id (npc_id) required for Wild Shape".into(),
            ))?;
            let chid = character_id.ok_or(AppError::BadRequest(
                "Wild Shape requires a linked character".into(),
            ))?;
            let druid_level: i32 = sqlx::query_scalar(
                r#"select coalesce(sum((elem->>'level')::int), 0)
                   from characters, jsonb_array_elements(sheet->'classes') as elem
                   where id = $1 and lower(elem->>'name') = 'druid'"#,
            )
            .bind(chid)
            .fetch_one(&s.db)
            .await?;
            if druid_level < 2 {
                return Err(AppError::BadRequest("Wild Shape requires druid level 2+".into()));
            }
            // Fetch beast NPC stats and verify creature_type = beast
            let beast: Option<(String, serde_json::Value)> = sqlx::query_as(
                "select name, stats from npcs where id = $1 and campaign_id = $2",
            )
            .bind(npc_id)
            .bind(campaign_id)
            .fetch_optional(&s.db)
            .await?;
            let (beast_name, beast_stats) = beast.ok_or(AppError::BadRequest("NPC not found".into()))?;
            let creature_type = beast_stats.get("creature_type").and_then(|v| v.as_str()).unwrap_or("");
            if creature_type.to_lowercase() != "beast" {
                return Err(AppError::BadRequest("Wild Shape target must have creature_type 'beast'".into()));
            }
            // CR validation (handle fraction strings like "1/4")
            let cr_str = beast_stats.get("cr").and_then(|v| v.as_str()).unwrap_or("0");
            let cr_float: f32 = if let Some(pos) = cr_str.find('/') {
                let num: f32 = cr_str[..pos].parse().unwrap_or(0.0);
                let den: f32 = cr_str[pos+1..].parse().unwrap_or(1.0);
                if den == 0.0 { 0.0 } else { num / den }
            } else {
                cr_str.parse().unwrap_or(0.0)
            };
            let max_cr = if druid_level >= 8 { 1.0 } else if druid_level >= 4 { 0.5 } else { 0.25 };
            if cr_float > max_cr {
                return Err(AppError::BadRequest(format!(
                    "beast CR {} exceeds max CR {} for druid level {}", cr_float, max_cr, druid_level
                )));
            }
            // No fly/swim restriction for MVP (L8+ can fly anyway)

            let mut tx = s.db.begin().await?;
            // Lock character for resource consumption
            sqlx::query("select id from characters where id = $1 for update")
                .bind(chid)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(AppError::NotFound)?;

            // Consume a Wild Shape use
            let ws_idx: i32 = sqlx::query_scalar(
                r#"select position - 1
                   from characters, jsonb_array_elements(sheet->'resources') with ordinality as t(elem, position)
                   where id = $1 and lower(t.elem->>'name') like '%wild%shape%'
                   limit 1"#,
            )
            .bind(chid)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::BadRequest(
                "Wild Shape resource not found on character sheet".into(),
            ))?;
            let ws_cur: i32 = sqlx::query_scalar(
                r#"select (elem->>'current')::int
                   from characters, jsonb_array_elements(sheet->'resources') as elem
                   where id = $1 and lower(elem->>'name') like '%wild%shape%'
                   limit 1"#,
            )
            .bind(chid)
            .fetch_optional(&mut *tx)
            .await?
            .flatten()
            .ok_or(AppError::BadRequest(
                "Wild Shape resource not found on character sheet".into(),
            ))?;
            if ws_cur <= 0 {
                return Err(AppError::BadRequest(
                    "Wild Shape depleted".into(),
                ));
            }
            sqlx::query(
                r#"update characters set sheet = jsonb_set(
                     sheet, ('{resources,' || $2 || ',current}')::text[],
                     to_jsonb($3::int)
                   ) where id = $1"#,
            )
            .bind(chid)
            .bind(ws_idx)
            .bind(ws_cur - 1)
            .execute(&mut *tx)
            .await?;
            // Store original combatant stats + beast starting HP
            let orig: (i32, i32, i32) = sqlx::query_as(
                "select hp_current, hp_max, ac from combatants where id = $1",
            )
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
            // Read beast stats
            let beast_hp_max = beast_stats.get("hp").and_then(|h| h.get("max")).and_then(|v| v.as_i64()).unwrap_or(1) as i32;
            let beast_hp_cur = beast_stats.get("hp").and_then(|h| h.get("current")).and_then(|v| v.as_i64()).unwrap_or(beast_hp_max as i64) as i32;
            let beast_ac = beast_stats.get("ac").and_then(|v| v.as_i64()).unwrap_or(10) as i32;
            // Save originals and apply beast stats
            sqlx::query(
                "update combatants set
                 wild_shape_original = jsonb_build_object('hp_current', $2, 'hp_max', $3, 'ac', $4, 'beast_starting_hp', $5),
                 hp_current = $6, hp_max = $7, ac = $8
                 where id = $1",
            )
            .bind(id)
            .bind(orig.0)   // original hp_current
            .bind(orig.1)   // original hp_max
            .bind(orig.2)   // original ac
            .bind(beast_hp_cur)  // beast_starting_hp
            .bind(beast_hp_cur)
            .bind(beast_hp_max)
            .bind(beast_ac)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            message = format!(
                "Wild Shape! Transformed into {} (HP: {}/{}, AC: {}).",
                beast_name, beast_hp_cur, beast_hp_max, beast_ac,
            );
            effect_applied = true;
        }
        "revert_wild_shape" => {
            let mut tx = s.db.begin().await?;
            let orig: Option<(serde_json::Value,)> = sqlx::query_as(
                "select wild_shape_original from combatants where id = $1 and wild_shape_original is not null",
            )
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
            let orig = orig.ok_or(AppError::BadRequest("not in wild shape".into()))?;
            let hp_cur = orig.0.get("hp_current").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let hp_max = orig.0.get("hp_max").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let ac = orig.0.get("ac").and_then(|v| v.as_i64()).unwrap_or(10) as i32;
            let beast_starting_hp = orig.0.get("beast_starting_hp").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            // Carry over beast damage to original form: excess = starting - current
            let beast_hp_cur: i32 = sqlx::query_scalar("select hp_current from combatants where id = $1")
                .bind(id).fetch_one(&mut *tx).await?;
            let beast_damage = (beast_starting_hp - beast_hp_cur).max(0);
            let restored_hp = (hp_cur - beast_damage).max(0);
            sqlx::query(
                "update combatants set
                 wild_shape_original = null,
                 hp_current = $2, hp_max = $3, ac = $4
                 where id = $1",
            )
            .bind(id)
            .bind(restored_hp)
            .bind(hp_max)
            .bind(ac)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            message = format!(
                "Reverted from Wild Shape (HP: {}/{}, damage carried over: {}).",
                restored_hp, hp_max, beast_damage,
            );
            hp_after = Some(restored_hp);
            effect_applied = true;
        }
        _ => {
            return Err(AppError::BadRequest(format!(
                "unknown class feature: {}",
                body.feature
            )));
        }
    }

    // M-WS3: strip `message` from the public event. The message often leaks
    // class feature details (e.g. "Rage! +2 damage, BPS resistance, STR
    // advantage" reveals the barbarian's class features to all members).
    // The feature NAME is still public (master wants to see "X used Rage"),
    // and the actor gets the full message via the HTTP response.
    ws::publish_persist(
        &s.db,
        campaign_id,
        json!({
            "type": "combatant_uses_class_feature",
            "combatant_id": id,
            "feature": feature,
            // MED-12: drop hp_after (M12 visibility leak). HP broadcasts go
            // through list_combatants with is_visible mask. Per-feature payload
            // is now feature-only; damage fields (smite_damage, smite_extra_undead,
            // smite_slot_consumed) still published as they don't leak HP.
        }),
    )
    .await;

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


/// A2: Battle Master — roll + consume one superiority die (fighter 3+).
/// Returns the rolled value; decrements sheet.resources Superiority Dice.
/// Shared with the attack handler (Precision Attack) and reactions (Parry).
pub(crate) async fn consume_superiority_die(
    tx: &mut sqlx::PgConnection,
    chid: uuid::Uuid,
    fighter_level: i32,
) -> Result<i32, AppError> {
    let sd_size = if fighter_level >= 18 { 12 } else if fighter_level >= 10 { 10 } else { 8 };
    sqlx::query("select id from characters where id = $1 for update")
        .bind(chid)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AppError::NotFound)?;
    let sd_idx: i32 = sqlx::query_scalar(
        r#"select position - 1
           from characters, jsonb_array_elements(sheet->'resources') with ordinality as t(elem, position)
           where id = $1 and lower(t.elem->>'name') like '%superiority%dice%'
           limit 1"#,
    )
    .bind(chid)
    .fetch_optional(&mut *tx)
    .await?
    .unwrap_or(-1);
    if sd_idx < 0 {
        return Err(AppError::BadRequest("no superiority dice resource found".into()));
    }
    let sd_current: i32 = sqlx::query_scalar(
        r#"select (elem->>'current')::int
           from characters, jsonb_array_elements(sheet->'resources') as elem
           where id = $1 and lower(elem->>'name') like '%superiority%dice%'
           limit 1"#,
    )
    .bind(chid)
    .fetch_optional(&mut *tx)
    .await?
    .flatten()
    .unwrap_or(0);
    if sd_current < 1 {
        return Err(AppError::BadRequest("no superiority dice remaining".into()));
    }
    sqlx::query(
        r#"update characters set sheet = jsonb_set(
             sheet, ('{resources,' || $2 || ',current}')::text[],
             to_jsonb($3::int)
           ) where id = $1"#,
    )
    .bind(chid)
    .bind(sd_idx)
    .bind(sd_current - 1)
    .execute(&mut *tx)
    .await?;
    let mut rng = rand::rngs::StdRng::from_os_rng();
    crate::dice::roll(&format!("d{sd_size}"), &mut rng)
        .map(|r| r.total)
        .map_err(|e| AppError::BadRequest(e.to_string()))
}
