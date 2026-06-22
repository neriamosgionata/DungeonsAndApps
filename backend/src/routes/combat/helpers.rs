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

#[cfg(test)]
mod tests {
    use super::{cond_name, has_condition, remove_condition};

    #[test]
    fn cond_name_strips_duration_suffix() {
        assert_eq!(cond_name("blinded:3"), "blinded");
        assert_eq!(cond_name("charmed"), "charmed");
        assert_eq!(cond_name(""), "");
    }

    #[test]
    fn has_condition_matches_bare_and_timed() {
        let conds = vec!["blinded".into(), "charmed:3".into()];
        assert!(has_condition(&conds, "blinded"));
        assert!(has_condition(&conds, "charmed"));
        assert!(!has_condition(&conds, "stunned"));
    }

    #[test]
    fn has_condition_case_insensitive() {
        let conds = vec!["Blinded".into(), "CHARMED:1".into()];
        assert!(has_condition(&conds, "blinded"));
        assert!(has_condition(&conds, "charmed"));
    }

    #[test]
    fn remove_condition_strips_bare() {
        let conds = vec!["blinded".into(), "charmed".into(), "stunned".into()];
        let out = remove_condition(conds, "blinded");
        assert_eq!(out, vec!["charmed".to_string(), "stunned".to_string()]);
    }

    #[test]
    fn remove_condition_strips_timed() {
        // remove_condition("charmed") should also drop "charmed:3"
        let conds = vec!["blinded:2".into(), "charmed:3".into(), "stunned".into()];
        let out = remove_condition(conds, "charmed");
        assert_eq!(out, vec!["blinded:2".to_string(), "stunned".to_string()]);
    }

    #[test]
    fn remove_condition_grapple_release_chain() {
        // Models the grapple-release-on-incapacitate logic:
        // grappler becomes incapacitated → remove their 'grappling' marker
        // → for every other combatant with 'grappled' marker, remove it too
        let grappler_conds = vec![
            "grappling".into(),
            "prone".into(),
        ];
        let after_grappler_release = remove_condition(grappler_conds, "grappling");
        assert_eq!(after_grappler_release, vec!["prone".to_string()]);

        // Each grappled target gets cleared
        let target_conds = vec!["grappled".into(), "prone".into()];
        let after_target_release = remove_condition(target_conds, "grappled");
        assert_eq!(after_target_release, vec!["prone".to_string()]);
    }

    #[test]
    fn remove_condition_not_present_is_noop() {
        let conds = vec!["blinded".into()];
        let out = remove_condition(conds, "stunned");
        assert_eq!(out, vec!["blinded".to_string()]);
    }
}
