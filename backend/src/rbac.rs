use crate::error::{AppError, AppResult};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "membership_role", rename_all = "lowercase")]
pub enum Role {
    Player,
    Master,
}

pub async fn role_in_campaign(db: &PgPool, user_id: Uuid, campaign_id: Uuid) -> AppResult<Role> {
    let row: Option<Role> = sqlx::query_scalar(
        r#"select role as "role: _" from memberships where user_id = $1 and campaign_id = $2"#,
    )
    .bind(user_id)
    .bind(campaign_id)
    .fetch_optional(db)
    .await?;
    row.ok_or(AppError::Forbidden)
}

async fn is_app_admin(db: &PgPool, user_id: Uuid) -> bool {
    let role: Option<String> = sqlx::query_scalar("select role::text from users where id = $1")
        .bind(user_id)
        .fetch_optional(db)
        .await
        .ok()
        .flatten();
    role.as_deref() == Some("admin")
}

pub async fn require_master(db: &PgPool, user_id: Uuid, campaign_id: Uuid) -> AppResult<()> {
    if is_app_admin(db, user_id).await {
        return Ok(());
    }
    match role_in_campaign(db, user_id, campaign_id).await? {
        Role::Master => Ok(()),
        Role::Player => Err(AppError::Forbidden),
    }
}

pub async fn require_member(db: &PgPool, user_id: Uuid, campaign_id: Uuid) -> AppResult<Role> {
    if is_app_admin(db, user_id).await {
        return Ok(Role::Master);
    }
    role_in_campaign(db, user_id, campaign_id).await
}
