// Shared helpers used by encounters/turns, special/*, combatants/*, tactical/*.
use super::Encounter;
use crate::{
    AppState,
    error::{AppError, AppResult},
};
use uuid::Uuid;

pub async fn fetch(s: &AppState, id: Uuid) -> AppResult<Encounter> {
    sqlx::query_as::<_, Encounter>(
        "select id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at
         from encounters where id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)
}

pub fn cond_name(c: &str) -> &str {
    c.split(':').next().unwrap_or(c)
}

pub fn has_condition(conditions: &[String], name: &str) -> bool {
    conditions.iter().any(|c| cond_name(c).eq_ignore_ascii_case(name))
}

pub fn remove_condition(conditions: Vec<String>, name: &str) -> Vec<String> {
    conditions
        .into_iter()
        .filter(|c| !cond_name(c).eq_ignore_ascii_case(name))
        .collect()
}
