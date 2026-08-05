// Reaction handlers (Shield, Counterspell, Ready Action) and auto-trigger.
// Extracted from actions.rs to keep the route handler file under the 500-line
// guideline (per AGENTS.md §1.4). Public re-exports preserve call-site compatibility.
use super::*;
use crate::AppState;
use crate::error::AppResult;
use crate::extract::AuthUser;
use axum::Json;
use axum::extract::{Path, State};
use rand::SeedableRng;

#[derive(Debug, Deserialize)]
pub struct ReactBody {
    pub reaction_type: String, // shield | counterspell | deflect_missiles | parry | interception | opportunity_attack | custom
    pub label: Option<String>,
    /// Counterspell: which caster's spell to counter. None = legacy LIMIT 1 behavior.
    pub target_caster_id: Option<Uuid>,
    /// Counterspell: slot level used to cast. Drives auto-success check.
    pub slot_level: Option<i32>,
    /// Counterspell: if slot < target_spell_level, client rolls ability check
    /// and passes the total here. Backend validates vs DC = 10 + target_spell_level.
    pub ability_check_total: Option<i32>,
    /// Interception: the ally combatant whose hit is being reduced.
    pub target_combatant_id: Option<Uuid>,
}

pub async fn react(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ReactBody>,
) -> AppResult<Json<super::super::combatants::Combatant>> {
    let auth = require_action_auth(&s.db, uid, id).await?;
    let campaign_id = auth.campaign_id;
    let encounter_id = auth.encounter_id;
    let mut tx = s.db.begin().await?;

    // Atomic reaction consumption
    let c: super::super::combatants::Combatant = sqlx::query_as::<_, super::super::combatants::Combatant>(
        r#"update combatants set reaction_used = true where id = $1 and reaction_used = false and not ('surprised' = any(conditions))
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, spell_being_cast, level_override, vision_range, faction, pending_hits, mounted_on"#,
    )
    .bind(id)
    .fetch_optional(&mut *tx).await?
    .ok_or(AppError::BadRequest("reaction already used this round".into()))?;

    // M-WS2: shield_blocked_hit is no longer published to the campaign
    // (it was intel about whether the hit landed). The outcome is observable
    // downstream via combatant_attacks and combatant_damages events.
    match body.reaction_type.as_str() {
        "shield" => {
            let row: (serde_json::Value, Option<i32>, i32) =
                sqlx::query_as("select pending_hits, hp_max, ac from combatants where id = $1")
                    .bind(id)
                    .fetch_one(&mut *tx)
                    .await?;
            let (pending_hits_raw, hp_max_col_opt, ac) = row;
            let mut hits: Vec<serde_json::Value> =
                pending_hits_raw.as_array().cloned().unwrap_or_default();
            let hit = hits.last().cloned().ok_or_else(|| {
                AppError::BadRequest(
                    "Shield can only be used when you have been hit (no pending hit this round)"
                        .into(),
                )
            })?;
            let atk_total = hit
                .get("attack_total")
                .and_then(|v| v.as_i64())
                .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32);
            let pending_dmg = hit
                .get("damage")
                .and_then(|v| v.as_i64())
                .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32);
            hits.pop();
            let new_pending = serde_json::Value::Array(hits);

            // In-tx AC read (HIGH-3 fix). Previous implementation called
            // combat_engine::load_snapshot(&s.db, id) outside the tx, which
            // could read a stale AC if a parallel writer changed it between
            // this read and the in-tx hp_max_reduction read. The Shield save
            // decision (`attack_total < ac_with_shield`) used the out-of-tx
            // value; the AC wasn't published to the client so the practical
            // impact was nil, but consistency is cheap.
            let ac_with_shield = ac + 5;
            let attack_total = atk_total.unwrap_or(0);

            sqlx::query(
                r#"insert into combatant_effects
                   (combatant_id, name, kind, duration_unit, duration_value, remaining, tick_trigger,
                    concentration, active, modifiers, source_type, applied_at_round, applied_at_turn_index)
                   values ($1, 'Shield (Reaction)', 'buff', 'rounds', 1, 1, 'caster_turn_start',
                           false, true, '{"ac_bonus": 5}', 'spell', $2, $3)"#,
            )
            .bind(id).bind(auth.round).bind(auth.turn_index).execute(&mut *tx).await?;

            if attack_total < ac_with_shield {
                // Restore exactly what the hit actually cost (hp_before - hp_after),
                // so temp HP absorption isn't over-restored. hp_max column is already
                // the effective max (reduction applied at sheet→combatant sync).
                let dmg_to_restore = hit
                    .get("hp_before")
                    .and_then(|v| v.as_i64())
                    .zip(hit.get("hp_after").and_then(|v| v.as_i64()))
                    .map(|(b, a)| (b - a).max(0) as i32)
                    .unwrap_or(pending_dmg.unwrap_or(0));
                let (current_hp,): (i32,) = sqlx::query_as(
                    "select hp_current from combatants where id = $1")
                    .bind(id).fetch_one(&mut *tx).await?;
                let hp_max_col = hp_max_col_opt.unwrap_or(0);
                let effective_max = hp_max_col.max(1);
                let new_hp = (current_hp + dmg_to_restore).min(effective_max);
                sqlx::query("update combatants set hp_current = $1, last_hit_attack_total = null, last_hit_damage = null, pending_hits = $2 where id = $3")
                    .bind(new_hp).bind(&new_pending).bind(id).execute(&mut *tx).await?;
                // H-7: the hit never happened — unwind death saves,
                // concentration, temp loss and re-sync the sheet.
                reverse_negated_hit(&mut tx, id, &hit).await?;
                // M-WS2: shield_blocked_hit removed — the HP restoration
                // here is the actual outcome. See combatant_attacks /
                // combatant_damages events downstream for the campaign
                // to see the final state.
            } else {
                sqlx::query("update combatants set last_hit_attack_total = null, last_hit_damage = null, pending_hits = $2 where id = $1")
                    .bind(id).bind(&new_pending).execute(&mut *tx).await?;
            }
        }
        "deflect_missiles" => {
            // A9: Deflect Missiles (PHB p.78) — Monk 3+ reduces an incoming
            // ranged hit by 1d10 + DEX mod + monk level. Ranged-only per
            // PHB; pending hits don't record weapon type, so the reduction
            // applies to any pending hit (documented approximation).
            let chid: Option<Uuid> =
                sqlx::query_scalar("select character_id from combatants where id = $1")
                    .bind(id)
                    .fetch_one(&mut *tx)
                    .await?;
            let monk_level: i32 = if let Some(chid) = chid {
                sqlx::query_scalar(
                    r#"select coalesce(sum((elem->>'level')::int), 0)
                       from characters, jsonb_array_elements(sheet->'classes') as elem
                       where id = $1 and lower(elem->>'name') = 'monk'"#,
                )
                .bind(chid)
                .fetch_one(&mut *tx)
                .await?
            } else {
                0
            };
            if monk_level < 3 {
                return Err(AppError::BadRequest(
                    "Deflect Missiles requires monk level 3+".into(),
                ));
            }
            let row: (serde_json::Value, i32) =
                sqlx::query_as("select pending_hits, hp_max from combatants where id = $1")
                    .bind(id)
                    .fetch_one(&mut *tx)
                    .await?;
            let (pending_hits_raw, hp_max_col) = row;
            let mut hits: Vec<serde_json::Value> =
                pending_hits_raw.as_array().cloned().unwrap_or_default();
            let hit = hits.last().cloned().ok_or_else(|| {
                AppError::BadRequest(
                    "Deflect Missiles requires a pending hit this round".into(),
                )
            })?;
            let dmg = hit
                .get("hp_before")
                .and_then(|v| v.as_i64())
                .zip(hit.get("hp_after").and_then(|v| v.as_i64()))
                .map(|(b, a)| (b - a).max(0) as i32)
                .unwrap_or(0);
            hits.pop();
            let new_pending = serde_json::Value::Array(hits);
            let dex_mod: i32 = sqlx::query_scalar(
                r#"select coalesce(
                    (select ((n.stats->'abilities'->>'dex')::int - 10) / 2
                     from combatants c2 join npcs n on n.id = c2.npc_id where c2.id = $1),
                    (select ((ch.sheet->'abilities'->>'dex')::int - 10) / 2
                     from combatants c2 join characters ch on ch.id = c2.character_id where c2.id = $1),
                    0
                )"#,
            )
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
            let mut rng = rand::rngs::StdRng::from_os_rng();
            let reduce = crate::dice::roll(&format!("1d10"), &mut rng)
                .map_err(|e| AppError::BadRequest(e.to_string()))?
                .total
                + dex_mod
                + monk_level;
            let remaining = (dmg - reduce).max(0);
            let restored = (dmg - remaining).max(0);
            let (current_hp,): (i32,) = sqlx::query_as("select hp_current from combatants where id = $1")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
            let new_hp = (current_hp + restored).min(hp_max_col.max(1));
            sqlx::query(
                "update combatants set hp_current = $1, pending_hits = $2 where id = $3",
            )
            .bind(new_hp)
            .bind(&new_pending)
            .bind(id)
            .execute(&mut *tx)
            .await?;
            // H-7: fully reduced (remaining == 0) = the hit never landed.
            if remaining == 0 {
                reverse_negated_hit(&mut tx, id, &hit).await?;
            }
            // Catching (Monk 5+, damage reduced to 0) allows a ranged
            // throw-back — exposed as a follow-up attack by the client.
            sqlx::query(
                "insert into combat_events (encounter_id, round, actor_combatant, target_combatant, action, delta_hp, note) values ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(auth.encounter_id)
            .bind(auth.round)
            .bind(id)
            .bind(hit.get("attacker_id").and_then(|v| v.as_str()).map(String::from).unwrap_or_default())
            .bind("Deflect Missiles")
            .bind(restored)
            .bind(Some(format!(
                "reduced {} damage by {} (1d10+{} DEX+{} monk)",
                dmg, reduce, dex_mod, monk_level
            )))
            .execute(&mut *tx)
            .await?;
        }
        "parry" => {
            // A2: Parry (PHB p.74) — Battle Master 3+; when hit, spend a
            // superiority die to add it to AC against the pending hit. If
            // the total beats the attack, the hit is negated (HP restored).
            let chid: Option<Uuid> =
                sqlx::query_scalar("select character_id from combatants where id = $1")
                    .bind(id)
                    .fetch_one(&mut *tx)
                    .await?;
            let fighter_level: i32 = if let Some(chid) = chid {
                sqlx::query_scalar(
                    r#"select coalesce(sum((elem->>'level')::int), 0)
                       from characters, jsonb_array_elements(sheet->'classes') as elem
                       where id = $1 and lower(elem->>'name') = 'fighter'"#,
                )
                .bind(chid)
                .fetch_one(&mut *tx)
                .await?
            } else {
                0
            };
            if fighter_level < 3 {
                return Err(AppError::BadRequest(
                    "Parry requires fighter level 3+ (Battle Master)".into(),
                ));
            }
            let chid = chid.ok_or(AppError::BadRequest(
                "Parry requires a linked character".into(),
            ))?;
            let sd = crate::routes::combat::special::consume_superiority_die(
                &mut *tx, chid, fighter_level,
            )
            .await?;
            let row: (serde_json::Value, Option<i32>, i32) =
                sqlx::query_as("select pending_hits, hp_max, ac from combatants where id = $1")
                    .bind(id)
                    .fetch_one(&mut *tx)
                    .await?;
            let (pending_hits_raw, hp_max_col, ac) = row;
            let mut hits: Vec<serde_json::Value> =
                pending_hits_raw.as_array().cloned().unwrap_or_default();
            let hit = hits.last().cloned().ok_or_else(|| {
                AppError::BadRequest("Parry requires a pending hit this round".into())
            })?;
            let atk_total = hit
                .get("attack_total")
                .and_then(|v| v.as_i64())
                .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
                .unwrap_or(0);
            hits.pop();
            let new_pending = serde_json::Value::Array(hits);
            if atk_total < ac + sd {
                let dmg = hit
                    .get("hp_before")
                    .and_then(|v| v.as_i64())
                    .zip(hit.get("hp_after").and_then(|v| v.as_i64()))
                    .map(|(b, a)| (b - a).max(0) as i32)
                    .unwrap_or(0);
                let (current_hp,): (i32,) =
                    sqlx::query_as("select hp_current from combatants where id = $1")
                        .bind(id)
                        .fetch_one(&mut *tx)
                        .await?;
                let new_hp = (current_hp + dmg).min(hp_max_col.unwrap_or(0).max(1));
                sqlx::query(
                    "update combatants set hp_current = $1, pending_hits = $2 where id = $3",
                )
                .bind(new_hp)
                .bind(&new_pending)
                .bind(id)
                .execute(&mut *tx)
                .await?;
                // H-7: full negation — unwind side effects + sheet sync.
                reverse_negated_hit(&mut tx, id, &hit).await?;
                sqlx::query(
                    "insert into combat_events (encounter_id, round, actor_combatant, target_combatant, action, delta_hp, note) values ($1, $2, $3, $4, $5, $6, $7)",
                )
                .bind(auth.encounter_id)
                .bind(auth.round)
                .bind(id)
                .bind(hit.get("attacker_id").and_then(|v| v.as_str()).map(String::from).unwrap_or_default())
                .bind("Parry")
                .bind(dmg)
                .bind(Some(format!("+{} AC negated the hit", sd)))
                .execute(&mut *tx)
                .await?;
            } else {
                sqlx::query(
                    "update combatants set pending_hits = $1 where id = $2",
                )
                .bind(&new_pending)
                .bind(id)
                .execute(&mut *tx)
                .await?;
            }
        }
        "interception" => {
            // A11: Interception fighting style (TCoE) — reaction: when an
            // ally within 5 ft is hit, reduce the damage by 1d10 + prof.
            let ally_id = body.target_combatant_id.ok_or(AppError::BadRequest(
                "Interception requires target_combatant_id (the ally being hit)".into(),
            ))?;
            let row: (serde_json::Value, i32) =
                sqlx::query_as("select pending_hits, hp_max from combatants where id = $1")
                    .bind(ally_id)
                    .fetch_one(&mut *tx)
                    .await?;
            let (pending_hits_raw, hp_max_col) = row;
            let mut hits: Vec<serde_json::Value> =
                pending_hits_raw.as_array().cloned().unwrap_or_default();
            let hit = hits.last().cloned().ok_or_else(|| {
                AppError::BadRequest(
                    "Interception requires the ally to have a pending hit this round".into(),
                )
            })?;
            let dmg = hit
                .get("hp_before")
                .and_then(|v| v.as_i64())
                .zip(hit.get("hp_after").and_then(|v| v.as_i64()))
                .map(|(b, a)| (b - a).max(0) as i32)
                .unwrap_or(0);
            hits.pop();
            let new_pending = serde_json::Value::Array(hits);
            // Protector's proficiency bonus from the combatant level_override
            // or linked character level.
            let prof: i32 = sqlx::query_scalar(
                "select 2 + (greatest(coalesce(c.level_override, 0), 0) - 1) / 4 from combatants c where c.id = $1",
            )
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
            let mut rng = rand::rngs::StdRng::from_os_rng();
            let reduce = crate::dice::roll(&format!("1d10+{}", prof), &mut rng)
                .map_err(|e| AppError::BadRequest(e.to_string()))?
                .total;
            let restored = dmg.min(reduce);
            let (ally_hp,): (i32,) =
                sqlx::query_as("select hp_current from combatants where id = $1")
                    .bind(ally_id)
                    .fetch_one(&mut *tx)
                    .await?;
            let new_hp = (ally_hp + restored).min(hp_max_col.max(1));
            sqlx::query(
                "update combatants set hp_current = $1, pending_hits = $2 where id = $3",
            )
            .bind(new_hp)
            .bind(&new_pending)
            .bind(ally_id)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "insert into combat_events (encounter_id, round, actor_combatant, target_combatant, action, delta_hp, note) values ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(auth.encounter_id)
            .bind(auth.round)
            .bind(id)
            .bind(ally_id)
            .bind("Interception")
            .bind(restored)
            .bind(Some(format!("reduced {} damage by {}", dmg, reduce)))
            .execute(&mut *tx)
            .await?;
        }
        "protection" => {
            // A11: Protection fighting style (PHB p.84) — reaction: when a
            // creature you can see within 5 ft is attacked, impose
            // disadvantage by REROLLING the attack (the roll already
            // resolved; the pending hit stores natural_roll + bonus so the
            // reroll keeps the same modifiers).
            let ally_id = body.target_combatant_id.ok_or(AppError::BadRequest(
                "Protection requires target_combatant_id (the ally being attacked)".into(),
            ))?;
            let row: (serde_json::Value, i32, i32) = sqlx::query_as(
                "select pending_hits, hp_max, ac from combatants where id = $1",
            )
            .bind(ally_id)
            .fetch_one(&mut *tx)
            .await?;
            let (pending_hits_raw, hp_max_col, ac) = row;
            let mut hits: Vec<serde_json::Value> =
                pending_hits_raw.as_array().cloned().unwrap_or_default();
            let hit = hits.last().cloned().ok_or_else(|| {
                AppError::BadRequest(
                    "Protection requires the ally to have a pending hit this round".into(),
                )
            })?;
            let bonus = hit
                .get("bonus")
                .and_then(|v| v.as_i64())
                .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
                .unwrap_or(0);
            hits.pop();
            let new_pending = serde_json::Value::Array(hits);
            let mut rng = rand::rngs::StdRng::from_os_rng();
            // Reroll with disadvantage (2d20 keep lowest).
            let reroll = crate::dice::roll(&format!("2d20kl1+{}", bonus), &mut rng)
                .map_err(|e| AppError::BadRequest(e.to_string()))?;
            if reroll.total < ac {
                let dmg = hit
                    .get("hp_before")
                    .and_then(|v| v.as_i64())
                    .zip(hit.get("hp_after").and_then(|v| v.as_i64()))
                    .map(|(b, a)| (b - a).max(0) as i32)
                    .unwrap_or(0);
                let (ally_hp,): (i32,) =
                    sqlx::query_as("select hp_current from combatants where id = $1")
                        .bind(ally_id)
                        .fetch_one(&mut *tx)
                        .await?;
                let new_hp = (ally_hp + dmg).min(hp_max_col.max(1));
                sqlx::query(
                    "update combatants set hp_current = $1, pending_hits = $2 where id = $3",
                )
                .bind(new_hp)
                .bind(&new_pending)
                .bind(ally_id)
                .execute(&mut *tx)
                .await?;
                // H-7: the reroll missed — the hit never landed on the ally.
                reverse_negated_hit(&mut tx, ally_id, &hit).await?;
                sqlx::query(
                    "insert into combat_events (encounter_id, round, actor_combatant, target_combatant, action, delta_hp, note) values ($1, $2, $3, $4, $5, $6, $7)",
                )
                .bind(auth.encounter_id)
                .bind(auth.round)
                .bind(id)
                .bind(ally_id)
                .bind("Protection")
                .bind(dmg)
                .bind(Some(format!("reroll {} < AC {} — attack negated", reroll.total, ac)))
                .execute(&mut *tx)
                .await?;
            } else {
                sqlx::query(
                    "update combatants set pending_hits = $1 where id = $2",
                )
                .bind(&new_pending)
                .bind(ally_id)
                .execute(&mut *tx)
                .await?;
            }
        }
        "counterspell" => {
            let (caster_id, target_spell_level): (Uuid, i32) = if let Some(target_id) =
                body.target_caster_id
            {
                let row: Option<(Uuid, String)> = sqlx::query_as(
                    r#"select id, spell_being_cast from combatants
                       where id = $1 and encounter_id = $2 and spell_being_cast is not null"#,
                )
                .bind(target_id)
                .bind(encounter_id)
                .fetch_optional(&mut *tx)
                .await?;
                let (cid, slug) = row.ok_or_else(|| AppError::BadRequest(
                    "Counterspell target is not currently casting a spell (or not in this encounter)".into()
                ))?;
                (cid, spell_level_of(&s.db, &slug, campaign_id).await?)
            } else {
                let row: Option<(Uuid, String)> = sqlx::query_as(
                    r#"select id, spell_being_cast from combatants
                       where encounter_id = $1 and spell_being_cast is not null
                       limit 1"#,
                )
                .bind(encounter_id)
                .fetch_optional(&mut *tx)
                .await?;
                if row.is_none() {
                    return Err(AppError::BadRequest(
                        "Counterspell can only be used when a spell is being cast".into(),
                    ));
                }
                let (cid, slug) = row.unwrap();
                (cid, spell_level_of(&s.db, &slug, campaign_id).await?)
            };

            // H-12: counterspell consumes a real spell slot (PHB p.228).
            // NPCs (no sheet slots) are GM-controlled — approximation, no
            // consumption. `spell_being_cast` is the slug only; the level
            // resolved here is the spell's BASE level (upcast tracking is a
            // known approximation — see COMBAT_AUDIT.md).
            let slot = body.slot_level.ok_or(AppError::BadRequest(
                "Counterspell requires slot_level".into(),
            ))?;
            let reactor_chid: Option<Uuid> =
                sqlx::query_scalar("select character_id from combatants where id = $1")
                    .bind(id)
                    .fetch_optional(&mut *tx)
                    .await?
                    .flatten();
            if let Some(chid) = reactor_chid {
                sqlx::query("select id from characters where id = $1 for update")
                    .bind(chid)
                    .fetch_optional(&mut *tx)
                    .await?
                    .ok_or(AppError::NotFound)?;
                let slot_key = format!("{}", slot);
                let slot_current: Option<i32> = sqlx::query_scalar(
                    "select (sheet->'slots'->$1->>'current')::int from characters where id = $2",
                )
                .bind(&slot_key)
                .bind(chid)
                .fetch_optional(&mut *tx)
                .await?
                .flatten();
                let cur = slot_current.ok_or(AppError::BadRequest(
                    "spell slot not found on character sheet".into(),
                ))?;
                if cur <= 0 {
                    return Err(AppError::BadRequest(
                        "spell slot depleted — cannot counterspell".into(),
                    ));
                }
                sqlx::query(
                    "update characters set sheet = jsonb_set(sheet, array['slots', $1, 'current'], to_jsonb($2::int)) where id = $3",
                )
                .bind(&slot_key)
                .bind(cur - 1)
                .bind(chid)
                .execute(&mut *tx)
                .await?;
            }

            if slot < target_spell_level {
                // H-12: server-rolled ability check (was client-supplied).
                // 1d20 + PB + the reactor's best spellcasting ability mod
                // (INT/WIS/CHA — matches the cast path's spell attack math).
                let dc = 10 + target_spell_level;
                let (pb, cast_mod): (i32, i32) = sqlx::query_as(
                    r#"select
                        coalesce(
                          (select (coalesce((n.stats->>'pb')::int, 2)) from combatants c2 join npcs n on n.id = c2.npc_id where c2.id = $1),
                          (select 2 + (greatest(coalesce((ch.sheet->>'level_total')::int, 1), 1) - 1) / 4 from combatants c2 join characters ch on ch.id = c2.character_id where c2.id = $1),
                          2),
                        coalesce(
                          (select greatest(((n.stats->'abilities'->>'int')::int - 10) / 2,
                                            ((n.stats->'abilities'->>'wis')::int - 10) / 2,
                                            ((n.stats->'abilities'->>'cha')::int - 10) / 2)
                           from combatants c2 join npcs n on n.id = c2.npc_id where c2.id = $1),
                          (select greatest(((ch.sheet->'abilities'->>'int')::int - 10) / 2,
                                            ((ch.sheet->'abilities'->>'wis')::int - 10) / 2,
                                            ((ch.sheet->'abilities'->>'cha')::int - 10) / 2)
                           from combatants c2 join characters ch on ch.id = c2.character_id where c2.id = $1),
                          0)"#,
                )
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
                let mut rng = rand::rngs::StdRng::from_os_rng();
                let total = crate::dice::roll(&format!("1d20+{}", pb + cast_mod), &mut rng)
                    .map_err(|e| AppError::BadRequest(e.to_string()))?
                    .total;
                if total < dc {
                    return Err(AppError::BadRequest(format!(
                        "Counterspell failed: ability check {} < DC {}",
                        total, dc
                    )));
                }
            }

            sqlx::query("update combatants set spell_being_cast = null where id = $1")
                .bind(caster_id)
                .execute(&mut *tx)
                .await?;
        }
        _ => {}
    }

    tx.commit().await?;

    let label = body.label.unwrap_or_else(|| body.reaction_type.clone());
    // M-WS2: drop shield_blocked_hit from the public event. It's intel —
    // "did the hit land or did Shield cancel it?" — that other players
    // shouldn't see. The reactor (target of the hit, user of the reaction)
    // already gets the outcome via the combat events log + the resulting
    // combatant_attacks / combatant_damages events.
    ws::publish_persist(
        &s.db,
        campaign_id,
        json!({
            "type": "combatant_reacts",
            "combatant_id": id,
            "reaction_type": body.reaction_type,
            "label": label,
        }),
    )
    .await;

    emit_campaign(
        &s.db,
        campaign_id,
        None,
        "combat.reaction",
        &format!("{} used reaction: {}", c.display_name, label),
        None,
        Some("encounter"),
        Some(encounter_id),
    )
    .await;

    Ok(Json(c))
}

pub async fn auto_trigger_ready_actions_for_event(
    db: &sqlx::PgPool,
    campaign_id: Uuid,
    encounter_id: Uuid,
    event_type: &str,
    actor_id: Uuid,
    subject_id: Uuid,
) {
    // C-P1: replace per-row correlated subquery + per-row UPDATE + per-row WS
    // with: 1 grid_size query + 1 readied query (no correlated subquery) + 1
    // subject position query + 1 batched UPDATE + 1 batched WS event.
    // For 10 readied triggered by 1 attack: 30 round-trips + 10 WS frames → 4 round-trips + 1 WS frame.

    // Pre-fetch encounter grid_size once (eliminates correlated subquery per row).
    let _grid_size: Option<i32> = sqlx::query_scalar(
        "select map_grid_size from encounters where id = $1",
    )
    .bind(encounter_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    // Fetch all readied combatants for this encounter in 1 query.
    let readied: Vec<(Uuid, serde_json::Value, Option<f32>, Option<f32>)> = match sqlx::query_as(
        r#"select id, readied_action, token_x, token_y
           from combatants
           where encounter_id = $1 and readied_action is not null and reaction_used = false"#,
    )
    .bind(encounter_id)
    .fetch_all(db)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!(encounter_id = %encounter_id, "auto_trigger_ready: readied query failed: {e}");
            return;
        }
    };

    // Pre-fetch subject position (for target_enters_range distance check).
    let subject_pos: Option<(Option<f32>, Option<f32>)> = sqlx::query_as(
        "select token_x, token_y from combatants where id = $1",
    )
    .bind(subject_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    // Filter in memory: which readied actions match this event.
    let mut triggered: Vec<(Uuid, serde_json::Value, serde_json::Value)> = Vec::new();
    for (cid, action_json, r_x, r_y) in readied {
        if cid == actor_id {
            continue;
        }

        let trigger_event = action_json
            .get("trigger_event")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if trigger_event != event_type {
            continue;
        }

        let watch_target = action_json
            .get("watch_target_id")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<Uuid>().ok());

        if let Some(wid) = watch_target {
            if wid != subject_id {
                continue;
            }
        }

        if trigger_event == "target_enters_range" {
            let watch_ft: f32 = action_json
                .get("watch_distance_ft")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32)
                .unwrap_or(5.0);
            // HIGH-4: 1 cell = 5ft = 20% of map → dist_pct × 0.25 = feet.
            let dist_ft = match (
                r_x,
                r_y,
                subject_pos.as_ref().and_then(|p| p.0),
                subject_pos.as_ref().and_then(|p| p.1),
            ) {
                (Some(rx), Some(ry), Some(sx), Some(sy)) => {
                    let dx = (rx - sx) as f32;
                    let dy = (ry - sy) as f32;
                    ((dx * dx + dy * dy).sqrt()) * 0.25
                }
                _ => f32::MAX,
            };
            if dist_ft > watch_ft {
                continue;
            }
        }

        // Build dispatch hint (client dispatches the actual effect).
        let action_kind = action_json
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let target_id = action_json
            .get("target_id")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<Uuid>().ok());
        let dispatch = match (action_kind, target_id) {
            ("attack", Some(tid)) => json!({
                "endpoint": "attack",
                "payload": { "target_id": tid }
            }),
            ("cast spell", _) => json!({
                "endpoint": "cast_spell",
                "payload": { "target_id": target_id }
            }),
            _ => json!({"endpoint": "noop"}),
        };
        triggered.push((cid, action_json, dispatch));
    }

    if triggered.is_empty() {
        return;
    }

    // Batched atomic UPDATE: consume reaction + clear readied_action for all triggered.
    let ids: Vec<Uuid> = triggered.iter().map(|(cid, _, _)| *cid).collect();
    let updated_ids: Vec<Uuid> = match sqlx::query_scalar(
        "update combatants set reaction_used = true, readied_action = null, action_used = false
         where id = ANY($1::uuid[]) and reaction_used = false
         returning id",
    )
    .bind(&ids)
    .fetch_all(db)
    .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(encounter_id = %encounter_id, "auto_trigger_ready: batched reaction consume failed: {e}");
            return;
        }
    };

    // Build single batched WS event for the actually-consumed set.
    let updates: Vec<serde_json::Value> = triggered
        .into_iter()
        .filter(|(cid, _, _)| updated_ids.contains(cid))
        .map(|(cid, action_json, dispatch)| {
            tracing::info!(
                combatant_id = %cid,
                trigger_event = %event_type,
                action = %action_json.get("action").and_then(|v| v.as_str()).unwrap_or(""),
                "readied action auto-triggered"
            );
            json!({
                "combatant_id": cid,
                "trigger_event": event_type,
                "triggered_by": actor_id,
                "readied_action": action_json,
                "dispatch": dispatch,
            })
        })
        .collect();

    if !updates.is_empty() {
        ws::publish_persist(
            db,
            campaign_id,
            json!({
                "type": "combatant_triggers_readied_actions",
                "triggers": updates,
            }),
        )
        .await;
    }
}

#[derive(Debug, Deserialize)]
pub struct ReadyBody {
    pub trigger: String,
    pub action: String,
    pub _target_id: Option<Uuid>,
    pub trigger_event: Option<String>,
    pub watch_target_id: Option<Uuid>,
}

pub async fn ready_action(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ReadyBody>,
) -> AppResult<Json<super::super::combatants::Combatant>> {
    let auth = require_action_auth(&s.db, uid, id).await?;
    let campaign_id = auth.campaign_id;
    let current_round = auth.round;

    let readied = json!({
        "trigger": body.trigger,
        "action": body.action,
        "target_id": body._target_id,
        "trigger_event": body.trigger_event,
        "watch_target_id": body.watch_target_id,
        "set_at_round": current_round,
        "expires_at_round": current_round + 1,
    });

    let mut tx = s.db.begin().await?;
    let c: Option<super::super::combatants::Combatant> = sqlx::query_as::<_, super::super::combatants::Combatant>(
        r#"update combatants set action_used = true, readied_action = $2
           where id = $1 and action_used = false
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, spell_being_cast, level_override, vision_range, faction, pending_hits, mounted_on"#,
    )
    .bind(id)
    .bind(readied)
    .fetch_optional(&mut *tx).await?;

    let c = c.ok_or_else(|| AppError::BadRequest("action already used this turn".into()))?;
    tx.commit().await?;

    ws::publish_persist(
        &s.db,
        campaign_id,
        json!({
            "type": "combatant_readies",
            "id": id,
            "trigger": body.trigger,
            "action": body.action,
        }),
    )
    .await;

    Ok(Json(c))
}

/// H-7: a fully-negated hit (Shield / Parry / Deflect-to-0 / Protection
/// reroll) "never happened" — unwind the side effects the attack path
/// committed and re-sync the character sheet. The pending_hits entry now
/// records temp delta, death-save failures, instant-death and concentration
/// break (see attack_apply.rs).
async fn reverse_negated_hit(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_id: Uuid,
    hit: &serde_json::Value,
) -> AppResult<()> {
    // M-21: restore temp HP the negated hit absorbed.
    if let (Some(tb), Some(ta)) = (
        hit.get("temp_before").and_then(|v| v.as_i64()),
        hit.get("temp_after").and_then(|v| v.as_i64()),
    ) {
        let diff = (tb - ta).max(0) as i32;
        if diff > 0 {
            sqlx::query("update combatants set temp_hp = temp_hp + $2 where id = $1")
                .bind(target_id)
                .bind(diff)
                .execute(&mut **tx)
                .await?;
        }
    }
    // Re-activate concentration effects broken by the negated hit.
    if hit
        .get("concentration_broken")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        sqlx::query(
            "update combatant_effects set active = true
             where combatant_id = $1 and concentration = true and active = false",
        )
        .bind(target_id)
        .execute(&mut **tx)
        .await?;
    }
    let chid: Option<Uuid> =
        sqlx::query_scalar("select character_id from combatants where id = $1")
            .bind(target_id)
            .fetch_optional(&mut **tx)
            .await?
            .flatten();
    if let Some(chid) = chid {
        // Unwind death-save failures recorded by the negated hit.
        let failures = hit
            .get("death_failures")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;
        let alive_set_false = hit
            .get("alive_set_false")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if failures > 0 && !alive_set_false {
            sqlx::query(
                r#"update characters set sheet =
                     coalesce(sheet, '{}'::jsonb)
                     || jsonb_build_object('death_saves', jsonb_build_object(
                          'successes', coalesce((sheet->'death_saves'->>'successes')::int, 0),
                          'failures', greatest(0, coalesce((sheet->'death_saves'->>'failures')::int, 0) - $2)
                     ))
                   where id = $1"#,
            )
            .bind(chid)
            .bind(failures)
            .execute(&mut **tx)
            .await?;
        }
        // Instant death is reversed by the sheet sync below (alive=true +
        // death-save reset when the restored HP is > 0).
    }
    // Re-sync the restored HP (and alive/death_saves) back to the sheet.
    let (hp, temp): (i32, i32) =
        sqlx::query_as("select hp_current, temp_hp from combatants where id = $1")
            .bind(target_id)
            .fetch_one(&mut **tx)
            .await?;
    sync_combatant_hp_to_sheet_tx(&mut **tx, target_id, hp, temp).await
}

/// H-11: resolve a spell's level from the SRD table, falling back to the
/// campaign's homebrew spells (campaign_spells) — the old `fetch_one` on
/// `spells` only 500'd for homebrew slugs.
async fn spell_level_of(db: &sqlx::PgPool, slug: &str, campaign_id: Uuid) -> AppResult<i32> {
    let srd: Option<i32> = sqlx::query_scalar("select level::int from spells where slug = $1")
        .bind(slug)
        .fetch_optional(db)
        .await?;
    if let Some(lvl) = srd {
        return Ok(lvl);
    }
    let homebrew: Option<i32> = sqlx::query_scalar(
        "select level::int from campaign_spells where slug = $1 and campaign_id = $2",
    )
    .bind(slug)
    .bind(campaign_id)
    .fetch_optional(db)
    .await?;
    homebrew.ok_or_else(|| AppError::BadRequest(format!("unknown spell slug: {slug}")))
}
