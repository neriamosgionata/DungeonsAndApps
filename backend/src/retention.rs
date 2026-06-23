// Sprint 38 HIGH-5: retention cleanup for unbounded-growth tables.
// Sprint 32 introduced ws_events, combat_events, notifications, dice_rolls
// and similar tables that grow without bound. Without a retention job, a
// long-running server eventually OOMs or hits disk limits.
//
// Strategy: periodic background task that runs once per hour. Each tick
// deletes rows older than the configured retention window. Tables and
// windows are listed in [`retention_specs`] so adding more tables is a
// one-line change.
//
// Defaults: ws_events + combat_events 7 days, notifications 30 days,
// dice_rolls 90 days. These match the audit's stated windows and the
// longest realistic disconnect / cleanup window for each.

use crate::AppState;
use sqlx::PgPool;
use std::time::Duration;
use tokio::time::interval;
use tracing::{info, warn};

/// Per-table retention spec. `column` is the timestamp column used for
/// the age check (must be timestamptz / now()-comparable).
struct RetentionSpec {
    table: &'static str,
    column: &'static str,
    window: Duration,
}

const RETENTION_SPECS: &[RetentionSpec] = &[
    // ws_events replay window: 7 days. The replay API only needs to
    // cover the longest realistic disconnect; older rows are useless.
    RetentionSpec {
        table: "ws_events",
        column: "created_at",
        window: Duration::from_secs(7 * 24 * 60 * 60),
    },
    // combat_events: 7 days. Combat history is interesting for the
    // active campaign but not beyond a couple of sessions.
    RetentionSpec {
        table: "combat_events",
        column: "created_at",
        window: Duration::from_secs(7 * 24 * 60 * 60),
    },
    // notifications: 30 days. Users have time to read + dismiss.
    RetentionSpec {
        table: "notifications",
        column: "created_at",
        window: Duration::from_secs(30 * 24 * 60 * 60),
    },
    // dice_rolls: 90 days. Roll history is small + cheap; keep longer
    // for session recap.
    RetentionSpec {
        table: "dice_rolls",
        column: "rolled_at",
        window: Duration::from_secs(90 * 24 * 60 * 60),
    },
];

/// Start the retention cleanup task. Call once at app startup.
pub fn start_retention_task(state: AppState) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(60 * 60)); // 1 hour
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // First tick fires immediately — skip it so we don't run cleanup
        // before the app is fully ready.
        ticker.tick().await;
        loop {
            ticker.tick().await;
            if let Err(e) = run_retention(&state.db).await {
                warn!(error = %e, "retention cleanup failed");
            }
        }
    });
    info!(
        tables = RETENTION_SPECS.len(),
        "retention cleanup task started"
    );
}

/// Run one round of retention cleanup. Public for tests + manual cron.
pub async fn run_retention(db: &PgPool) -> Result<(), sqlx::Error> {
    for spec in RETENTION_SPECS {
        // PostgreSQL: now() - interval 'N seconds' gives a timestamptz.
        // Bind the seconds as i64 so we can use the Duration::as_secs().
        let secs = spec.window.as_secs() as i64;
        let sql = format!(
            "delete from {} where {} < now() - make_interval(secs => $1)",
            spec.table, spec.column
        );
        let res = sqlx::query(&sql).bind(secs as f64).execute(db).await?;
        if res.rows_affected() > 0 {
            info!(
                table = spec.table,
                deleted = res.rows_affected(),
                secs,
                "retention cleanup deleted rows"
            );
        }
    }
    Ok(())
}
