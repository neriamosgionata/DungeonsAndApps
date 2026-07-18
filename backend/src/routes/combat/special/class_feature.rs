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
            // PHB: target takes half damage (floor). Apply halved damage to HP, drop the hit.
            let halve = (final_dmg / 2).max(0);
            let new_hp = (hp_cur - halve).max(0);
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
        "trip_attack" | "menacing_attack" => {
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
            // Compute superiority die size: d8 at L3, d10 at L10, d12 at L18
            let sd_size = if fighter_level >= 18 { 12 } else if fighter_level >= 10 { 10 } else { 8 };
            let mut tx = s.db.begin().await?;
            // Lock character row for atomic SD consumption
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
            // Roll superiority die
            let mut rng = rand::rngs::StdRng::from_os_rng();
            let sd_roll = crate::dice::roll(&format!("d{}", sd_size), &mut rng)
                .map_err(|e| AppError::BadRequest(e.to_string()))?;
            // Consume superiority die
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
            // Apply superiority die damage to target
            let (hp_cur, _hp_max, temp_hp): (i32, i32, i32) = sqlx::query_as(
                "select hp_current, hp_max, temp_hp from combatants where id = $1",
            )
            .bind(target_id)
            .fetch_one(&mut *tx)
            .await?;
            let (new_hp, new_temp) = combat_engine::apply_hp_damage(hp_cur, temp_hp, sd_roll.total);
            sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
                .bind(new_hp)
                .bind(new_temp)
                .bind(target_id)
                .execute(&mut *tx)
                .await?;
            // Force save based on maneuver type
            let maneuver = feature.as_str();
            let (save_ability, condition_name, condition_msg) = match maneuver {
                "trip_attack" => ("str", "prone", "knocked prone"),
                _ => ("wis", "frightened", "frightened"),
            };
            // Compute save DC: 8 + prof + STR or DEX mod (fighter's choice)
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
            tx.commit().await?;
            if save_failed {
                message = format!(
                    "{}! {} damage + target {} (save {:.1} vs DC {}).",
                    if maneuver == "trip_attack" { "Trip Attack" } else { "Menacing Attack" },
                    sd_roll.total, condition_msg, save_roll.total, dc,
                );
            } else {
                message = format!(
                    "{}! {} damage, target saved vs DC {}.",
                    if maneuver == "trip_attack" { "Trip Attack" } else { "Menacing Attack" },
                    sd_roll.total, dc,
                );
            }
            hp_after = Some(new_hp);
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
            message = format!(
                "Turn Undead! {} undead turned (DC {}, WIS save).",
                turned, dc
            );
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
