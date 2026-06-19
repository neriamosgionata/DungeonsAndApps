pub mod actions;
pub mod combatants;
pub mod encounters;
pub mod events;
pub mod special;
pub mod spells;
pub mod tactical;

mod helpers;
mod notifications;
mod tick;

use crate::{
    AppState,
    error::AppError,
    routes::notifications::emit_campaign,
    ws,
};
use axum::{Router, extract::DefaultBodyLimit, routing::{get, patch, post}};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use self::actions::*;
use self::combatants::{
    add_combatant, bulk_add_combatants, delete_combatant, list_combatants, move_combatant,
    update_combatant, use_action,
};
use self::encounters::{
    create, delete, end_encounter, goto_turn, list, next_turn, prev_turn, read, set_initiative,
    start, update,
};
use self::events::{delete_event, list_events, patch_effects};
use self::special::{
    class_feature, grapple, grapple_escape, lair_action, legendary_action, multiattack,
    parse_multiattack, shove, stand_up, trigger_ready,
};
use self::spells::cast_spell;
use self::tactical::{
    add_condition, calculate_cover, check_flanking, create_overlay, delete_overlay,
    encounter_difficulty, list_overlays, overlay_damage, surprise_auto, surprise_round,
};

pub use self::encounters::types::Encounter;
pub use self::helpers::{cond_name, fetch, has_condition, remove_condition};
pub(crate) use self::notifications::notify_turn;
pub(crate) use self::tick::tick_effects;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/campaigns/{id}/encounters", get(list).post(create))
        .route("/encounters/{id}", get(read).patch(update).delete(delete))
        .route(
            "/encounters/{id}/combatants",
            get(list_combatants).post(add_combatant),
        )
        .route(
            "/encounters/{id}/combatants/bulk",
            post(bulk_add_combatants),
        )
        .route(
            "/combatants/{id}",
            axum::routing::patch(update_combatant).delete(delete_combatant),
        )
        .route("/combatants/{id}/move", post(move_combatant))
        .route("/combatants/{id}/use-action", post(use_action))
        .route("/encounters/{id}/next-turn", post(next_turn))
        .route("/encounters/{id}/prev-turn", post(prev_turn))
        .route("/encounters/{id}/goto-turn", post(goto_turn))
        .route("/encounters/{id}/start", post(start))
        .route("/encounters/{id}/end", post(end_encounter))
        .route("/encounters/{id}/set-initiative", post(set_initiative))
        .route(
            "/encounters/{id}/overlays",
            get(list_overlays).post(create_overlay),
        )
        .route(
            "/encounters/{id}/overlays/{overlay_id}",
            axum::routing::delete(delete_overlay),
        )
        .route("/combatants/{id}/attack", post(attack))
        .route("/combatants/{id}/damage", post(deal_damage))
        .route("/combatants/{id}/save", post(roll_save))
        .route("/combatants/{id}/computed-stats", get(computed_stats))
        .route("/combatants/{id}/react", post(react))
        .route("/combatants/{id}/cast-spell", post(cast_spell))
        .route("/combatants/{id}/dodge", post(dodge))
        .route("/combatants/{id}/disengage", post(disengage))
        .route("/combatants/{id}/help", post(help_action))
        .route(
            "/combatants/{id}/opportunity-attack",
            post(opportunity_attack),
        )
        .route("/combatants/{id}/ready", post(ready_action))
        .route("/combatants/{id}/delay", post(delay_turn))
        .route("/combatants/{id}/grapple", post(grapple))
        .route("/combatants/{id}/grapple-escape", post(grapple_escape))
        .route("/combatants/{id}/shove", post(shove))
        .route("/combatants/{id}/stand-up", post(stand_up))
        .route("/combatants/{id}/heal", post(heal))
        .route("/combatants/{id}/death-save", post(death_save))
        .route("/combatants/{id}/skill-check", post(skill_check))
        .route("/encounters/{id}/lair-action", post(lair_action))
        .route("/combatants/{id}/legendary-action", post(legendary_action))
        .route("/combatants/{id}/multiattack", post(multiattack))
        .route("/combatants/{id}/parse-multiattack", get(parse_multiattack))
        .route("/combatants/{id}/trigger-ready", post(trigger_ready))
        .route("/combatants/{id}/class-feature", post(class_feature))
        .route("/combatants/{id}/two-weapon-fight", post(two_weapon_fight))
        .route("/combatants/{id}/dash", post(dash))
        .route("/combatants/{id}/hide", post(hide))
        .route("/combatants/{id}/contested-hide", post(contested_hide))
        .route("/combatants/{id}/search", post(search_action))
        .route("/combatants/{id}/use-object", post(use_object))
        .route("/combatants/{id}/conditions", post(add_condition))
        .route("/encounters/{id}/effects", patch(patch_effects))
        .route("/encounters/{id}/overlay-damage", post(overlay_damage))
        .route("/encounters/{id}/surprise", post(surprise_round))
        .route("/encounters/{id}/surprise-auto", post(surprise_auto))
        .route("/encounters/{id}/difficulty", get(encounter_difficulty))
        .route("/encounters/{id}/flanking", get(check_flanking))
        .route("/encounters/{id}/cover", get(calculate_cover))
        .route("/encounters/{id}/events", get(list_events))
        .route(
            "/combat-events/{event_id}",
            axum::routing::delete(delete_event),
        )
        // LOW-7: cap combat body at 512KB (axum default is 2MB). Single-action bodies
        // are typically <10KB; bulk_add_combatants is the largest at ~1KB/row, so 512KB
        // admits ~500 combatants which is well above any real encounter.
        .layer(DefaultBodyLimit::max(512 * 1024))
}
