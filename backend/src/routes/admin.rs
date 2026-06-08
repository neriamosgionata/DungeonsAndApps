use crate::{AppState, error::{AppError, AppResult}, extract::AuthUser};
use axum::{Json, Router, extract::{Path, State}, http::StatusCode, routing::{delete, get, post}};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/stats", get(stats))
        .route("/admin/campaigns", get(list_campaigns))
        .route("/admin/campaigns/{id}", delete(delete_campaign))
        .route("/admin/backup", get(create_backup))
        .route("/admin/restore", post(restore_backup))
}

async fn require_admin(db: &sqlx::PgPool, uid: Uuid) -> AppResult<()> {
    let role: String = sqlx::query_scalar("select role::text from users where id = $1")
        .bind(uid).fetch_optional(db).await?.ok_or(AppError::Unauthorized)?;
    if role != "admin" { return Err(AppError::Forbidden); }
    Ok(())
}

#[derive(Serialize)]
pub struct Stats {
    pub users: i64,
    pub campaigns: i64,
    pub characters: i64,
    pub messages: i64,
    pub encounters: i64,
    pub spells: i64,
}

async fn stats(State(s): State<AppState>, AuthUser(uid): AuthUser) -> AppResult<Json<Stats>> {
    require_admin(&s.db, uid).await?;
    let (users, campaigns, characters, messages, encounters, spells): (i64,i64,i64,i64,i64,i64) = tokio::try_join!(
        sqlx::query_scalar("select count(*) from users").fetch_one(&s.db),
        sqlx::query_scalar("select count(*) from campaigns").fetch_one(&s.db),
        sqlx::query_scalar("select count(*) from characters").fetch_one(&s.db),
        sqlx::query_scalar("select count(*) from messages").fetch_one(&s.db),
        sqlx::query_scalar("select count(*) from encounters").fetch_one(&s.db),
        sqlx::query_scalar("select count(*) from spells").fetch_one(&s.db),
    )?;
    Ok(Json(Stats { users, campaigns, characters, messages, encounters, spells }))
}

#[derive(Serialize)]
pub struct CampaignRow {
    pub id: Uuid,
    pub name: String,
    pub owner_name: String,
    pub member_count: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

async fn list_campaigns(State(s): State<AppState>, AuthUser(uid): AuthUser) -> AppResult<Json<Vec<CampaignRow>>> {
    require_admin(&s.db, uid).await?;
    let rows = sqlx::query_as::<_, (Uuid, String, String, i64, OffsetDateTime)>(
        r#"select c.id,
                  c.name,
                  coalesce(
                    (select u.display_name from memberships ms
                     join users u on u.id = ms.user_id
                     where ms.campaign_id = c.id and ms.role = 'master'
                     limit 1),
                    'Unknown'
                  ) as owner_name,
                  (select count(*) from memberships m where m.campaign_id = c.id) as member_count,
                  c.created_at
           from campaigns c
           order by c.created_at desc"#,
    )
    .fetch_all(&s.db)
    .await?
    .into_iter()
    .map(|(id, name, owner_name, member_count, created_at)| CampaignRow { id, name, owner_name, member_count, created_at })
    .collect();
    Ok(Json(rows))
}

async fn delete_campaign(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    require_admin(&s.db, uid).await?;
    let res = sqlx::query("delete from campaigns where id = $1")
        .bind(id).execute(&s.db).await?;
    if res.rows_affected() == 0 { return Err(AppError::NotFound); }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, Deserialize)]
pub struct BackupData {
    pub version: i32,
    pub exported_at: String,
    pub tables: BackupTables,
}

#[derive(Serialize, Deserialize)]
pub struct BackupTables {
    pub users: Vec<serde_json::Value>,
    pub campaigns: Vec<serde_json::Value>,
    pub memberships: Vec<serde_json::Value>,
    pub characters: Vec<serde_json::Value>,
    pub character_spells: Vec<serde_json::Value>,
    pub sessions: Vec<serde_json::Value>,
    pub maps: Vec<serde_json::Value>,
    pub map_pins: Vec<serde_json::Value>,
    pub npcs: Vec<serde_json::Value>,
    pub factions: Vec<serde_json::Value>,
    pub lore: Vec<serde_json::Value>,
    pub news: Vec<serde_json::Value>,
    pub quests: Vec<serde_json::Value>,
    pub quest_npcs: Vec<serde_json::Value>,
    pub party_data: Vec<serde_json::Value>,
    pub loot: Vec<serde_json::Value>,
    pub encounters: Vec<serde_json::Value>,
    pub combatants: Vec<serde_json::Value>,
    pub combatant_effects: Vec<serde_json::Value>,
    pub encounter_overlays: Vec<serde_json::Value>,
    pub messages: Vec<serde_json::Value>,
    pub notifications: Vec<serde_json::Value>,
    pub invitations: Vec<serde_json::Value>,
    pub dice_rolls: Vec<serde_json::Value>,
    pub spells: Vec<serde_json::Value>,
    pub combat_events: Vec<serde_json::Value>,
    pub sessions_auth: Vec<serde_json::Value>,
    pub conditions: Vec<serde_json::Value>,
}

async fn create_backup(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
) -> AppResult<(StatusCode, Json<BackupData>)> {
    require_admin(&s.db, uid).await?;

    let tables = BackupTables {
        users: fetch_table(&s.db, "users").await?,
        campaigns: fetch_table(&s.db, "campaigns").await?,
        memberships: fetch_table(&s.db, "memberships").await?,
        characters: fetch_table(&s.db, "characters").await?,
        character_spells: fetch_table(&s.db, "character_spells").await?,
        sessions: fetch_table(&s.db, "campaign_sessions").await?,
        maps: fetch_table(&s.db, "maps").await?,
        map_pins: fetch_table(&s.db, "map_pins").await?,
        npcs: fetch_table(&s.db, "npcs").await?,
        factions: fetch_table(&s.db, "factions").await?,
        lore: fetch_table(&s.db, "lore_entries").await?,
        news: fetch_table(&s.db, "news_entries").await?,
        quests: fetch_table(&s.db, "quests").await?,
        quest_npcs: fetch_table(&s.db, "quest_npcs").await?,
        party_data: fetch_table(&s.db, "parties").await?,
        loot: fetch_table(&s.db, "loot_items").await?,
        encounters: fetch_table(&s.db, "encounters").await?,
        combatants: fetch_table(&s.db, "combatants").await?,
        combatant_effects: fetch_table(&s.db, "combatant_effects").await?,
        encounter_overlays: fetch_table(&s.db, "encounter_overlays").await?,
        messages: fetch_table(&s.db, "messages").await?,
        notifications: fetch_table(&s.db, "notifications").await?,
        invitations: fetch_table(&s.db, "campaign_invitations").await?,
        dice_rolls: fetch_table(&s.db, "dice_rolls").await?,
        spells: fetch_table(&s.db, "spells").await?,
        combat_events: fetch_table(&s.db, "combat_events").await?,
        sessions_auth: fetch_table(&s.db, "sessions_auth").await?,
        conditions: fetch_table(&s.db, "conditions").await?,
    };

    let backup = BackupData {
        version: 1,
        exported_at: time::OffsetDateTime::now_utc().to_string(),
        tables,
    };

    Ok((StatusCode::OK, Json(backup)))
}

async fn fetch_table(db: &sqlx::PgPool, table: &str) -> AppResult<Vec<serde_json::Value>> {
    let query = format!("SELECT to_jsonb(t.*) as row FROM {} t", table);
    let rows: Vec<serde_json::Value> = sqlx::query_scalar(&query)
        .fetch_all(db)
        .await?;
    Ok(rows)
}

#[derive(Deserialize)]
pub struct RestoreRequest {
    pub backup: BackupData,
}

async fn restore_backup(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Json(body): Json<RestoreRequest>,
) -> AppResult<StatusCode> {
    require_admin(&s.db, uid).await?;

    let mut tx = s.db.begin().await?;

    // parent-first INSERT order; DELETE runs reverse (children-first)
    let table_order: Vec<(&str, &Vec<serde_json::Value>)> = vec![
        ("users", &body.backup.tables.users),
        ("spells", &body.backup.tables.spells),
        ("conditions", &body.backup.tables.conditions),
        ("campaigns", &body.backup.tables.campaigns),
        ("sessions_auth", &body.backup.tables.sessions_auth),
        ("memberships", &body.backup.tables.memberships),
        ("characters", &body.backup.tables.characters),
        ("campaign_sessions", &body.backup.tables.sessions),
        ("maps", &body.backup.tables.maps),
        ("factions", &body.backup.tables.factions),
        ("map_pins", &body.backup.tables.map_pins),
        ("npcs", &body.backup.tables.npcs),
        ("lore_entries", &body.backup.tables.lore),
        ("news_entries", &body.backup.tables.news),
        ("parties", &body.backup.tables.party_data),
        ("loot_items", &body.backup.tables.loot),
        ("quests", &body.backup.tables.quests),
        ("quest_npcs", &body.backup.tables.quest_npcs),
        ("messages", &body.backup.tables.messages),
        ("dice_rolls", &body.backup.tables.dice_rolls),
        ("notifications", &body.backup.tables.notifications),
        ("campaign_invitations", &body.backup.tables.invitations),
        ("character_spells", &body.backup.tables.character_spells),
        ("encounters", &body.backup.tables.encounters),
        ("combatants", &body.backup.tables.combatants),
        ("combatant_effects", &body.backup.tables.combatant_effects),
        ("encounter_overlays", &body.backup.tables.encounter_overlays),
        ("combat_events", &body.backup.tables.combat_events),
    ];

    // Pass 1: DELETE all tables, children-first (reverse of insert order)
    for (table, data) in table_order.iter().rev() {
        if data.is_empty() {
            continue;
        }
        sqlx::query(&format!("DELETE FROM {}", table))
            .execute(&mut *tx)
            .await?;
    }

    // Pass 2: INSERT all tables, parents-first (forward order)
    for (table, data) in &table_order {
        if data.is_empty() {
            continue;
        }
        for row in *data {
            let obj = match row.as_object() {
                Some(o) => o,
                None => continue,
            };

            let columns: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
            if columns.is_empty() {
                continue;
            }

            // Validate column names against SQL injection
            for col in &columns {
                if !col.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    || col.starts_with(|c: char| c.is_ascii_digit())
                {
                    return Err(AppError::BadRequest(
                        format!("invalid column name in backup: {col}")
                    ));
                }
            }

            let col_list = columns.join(", ");
            let placeholders: Vec<String> = (1..=columns.len())
                .map(|i| format!("${}", i))
                .collect();
            let ph_list = placeholders.join(", ");

            let query_str = format!("INSERT INTO {} ({}) VALUES ({})", table, col_list, ph_list);
            let mut q = sqlx::query(&query_str);

            for col in &columns {
                let val = obj.get(*col).unwrap_or(&serde_json::Value::Null);
                q = bind_json_value(q, val);
            }

            q.execute(&mut *tx).await?;
        }
    }

    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

fn bind_json_value<'a>(
    q: sqlx::query::Query<'a, sqlx::Postgres, sqlx::postgres::PgArguments>,
    val: &serde_json::Value,
) -> sqlx::query::Query<'a, sqlx::Postgres, sqlx::postgres::PgArguments> {
    match val {
        serde_json::Value::Null => q.bind::<Option<String>>(None),
        serde_json::Value::Bool(b) => q.bind(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                q.bind(i)
            } else if let Some(f) = n.as_f64() {
                q.bind(f)
            } else {
                q.bind(n.to_string())
            }
        }
        serde_json::Value::String(s) => q.bind(s.clone()),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => q.bind(val.to_string()),
    }
}
