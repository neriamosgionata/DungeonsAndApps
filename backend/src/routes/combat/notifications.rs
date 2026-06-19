// Turn-change notifications: round/turn broadcasts + per-user "your turn" alert.
use super::Encounter;
use crate::{
    AppState,
    routes::notifications::{NewNotif, emit, emit_campaign},
};
use uuid::Uuid;

pub async fn notify_turn(s: &AppState, e: &Encounter, prev_round: i32) {
    let row: Option<(String, Option<Uuid>, Uuid)> = sqlx::query_as(
        r#"select c.display_name, ch.owner_id, c.id
           from combatants c
           left join characters ch on ch.id = c.character_id
           where c.encounter_id = $1
           order by c.turn_order asc
           offset $2 limit 1"#,
    )
    .bind(e.id)
    .bind(e.turn_index as i64)
    .fetch_optional(&s.db)
    .await
    .ok()
    .flatten();
    if let Some((name, owner, _cid)) = row {
        if e.round > prev_round {
            emit_campaign(
                &s.db,
                e.campaign_id,
                None,
                "combat.round",
                &format!("Round {} — {}", e.round, name),
                None,
                Some("encounter"),
                Some(e.id),
            )
            .await;
        }
        if let Some(o) = owner {
            emit(
                &s.db,
                NewNotif {
                    user_id: o,
                    campaign_id: Some(e.campaign_id),
                    kind: "combat.your_turn",
                    title: "It's your turn!",
                    body: Some(&format!("{} — round {}", name, e.round)),
                    ref_kind: Some("encounter"),
                    ref_id: Some(e.id),
                },
            )
            .await;
        }
    }
}
