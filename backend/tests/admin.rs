//! Admin endpoints: users CRUD + admin stats + admin campaigns
mod helpers;
use helpers::*;
use serde_json::json;

macro_rules! skip_no_db {
    () => {
        match make_app().await {
            Some(x) => x,
            None => {
                eprintln!("SKIP: TEST_DATABASE_URL/DATABASE_URL not set");
                return;
            }
        }
    };
}

// Helper: bootstrap admin + register a non-admin user via admin API.
// Promotes the first registered user to admin via direct SQL (test-only).
async fn setup_admin_and_user(
    router: &axum::Router,
    db: &sqlx::PgPool,
) -> (String, String, String, String) {
    let (admin_tok, admin_body) = register(router, "admin@test.com").await;
    let admin_id = admin_body["user"]["id"].as_str().unwrap().to_string();

    sqlx::query("update users set role = 'admin' where email = $1")
        .bind("admin@test.com")
        .execute(db)
        .await
        .expect("promote admin user");

    let (s, body) = json_req(
        router,
        "POST",
        "/api/v1/users",
        Some(&admin_tok),
        Some(json!({
            "email": "player@test.com",
            "password": TEST_PASSWORD,
            "display_name": "Player",
            "role": "user",
            "language": "en",
        })),
    )
    .await;
    assert_eq!(s.as_u16(), 201, "create user: {body}");
    let user_id = body["id"].as_str().unwrap().to_string();
    (admin_tok, admin_id, String::new(), user_id)
}

#[tokio::test]
async fn admin_create_user_success() {
    let (router, db) = skip_no_db!();
    let (admin_tok, _, _, _) = setup_admin_and_user(&router, &db).await;

    // list confirms new user exists
    let (s, body) = json_req(&router, "GET", "/api/v1/users", Some(&admin_tok), None).await;
    assert_eq!(s.as_u16(), 200);
    let users = body.as_array().unwrap();
    assert!(users.iter().any(|u| u["email"] == "player@test.com"));
}

#[tokio::test]
async fn admin_create_user_rejects_non_admin() {
    let (router, db) = skip_no_db!();
    // bootstrap admin
    let (admin_tok, _, _, user_id) = setup_admin_and_user(&router, &db).await;

    // get a token for the plain user by logging in
    let (s, body) = json_req(
        &router,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(json!({ "email": "player@test.com", "password": TEST_PASSWORD })),
    )
    .await;
    assert_eq!(s.as_u16(), 200);
    let user_tok = body["token"].as_str().unwrap().to_string();

    // plain user tries to create a user — should be 403
    let (s, _) = json_req(
        &router,
        "POST",
        "/api/v1/users",
        Some(&user_tok),
        Some(json!({ "email": "x@test.com", "password": TEST_PASSWORD, "display_name": "X" })),
    )
    .await;
    assert_eq!(s.as_u16(), 403);

    // suppress unused warning
    let _ = user_id;
    let _ = admin_tok;
}

#[tokio::test]
async fn admin_create_user_duplicate_email_conflict() {
    let (router, db) = skip_no_db!();
    let (admin_tok, _, _, _) = setup_admin_and_user(&router, &db).await;

    let (s, _) = json_req(
        &router,
        "POST",
        "/api/v1/users",
        Some(&admin_tok),
        Some(
            json!({ "email": "player@test.com", "password": TEST_PASSWORD, "display_name": "Dup" }),
        ),
    )
    .await;
    assert_eq!(s.as_u16(), 409, "duplicate email should be 409");
}

#[tokio::test]
async fn admin_update_user_role() {
    let (router, db) = skip_no_db!();
    let (admin_tok, _, _, user_id) = setup_admin_and_user(&router, &db).await;

    let (s, body) = json_req(
        &router,
        "PATCH",
        &format!("/api/v1/users/{user_id}"),
        Some(&admin_tok),
        Some(json!({ "role": "admin" })),
    )
    .await;
    assert_eq!(s.as_u16(), 200);
    assert_eq!(body["role"], "admin");
}

#[tokio::test]
async fn admin_delete_user() {
    let (router, db) = skip_no_db!();
    let (admin_tok, _, _, user_id) = setup_admin_and_user(&router, &db).await;

    let (s, _) = json_req(
        &router,
        "DELETE",
        &format!("/api/v1/users/{user_id}"),
        Some(&admin_tok),
        None,
    )
    .await;
    assert_eq!(s.as_u16(), 204);

    // confirm gone
    let (s, body) = json_req(&router, "GET", "/api/v1/users", Some(&admin_tok), None).await;
    assert_eq!(s.as_u16(), 200);
    let users = body.as_array().unwrap();
    assert!(!users.iter().any(|u| u["id"] == user_id));
}

#[tokio::test]
async fn admin_reset_password() {
    let (router, db) = skip_no_db!();
    let (admin_tok, _, _, user_id) = setup_admin_and_user(&router, &db).await;

    let new_pw = "NewPass99!";
    let (s, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/users/{user_id}/reset-password"),
        Some(&admin_tok),
        Some(json!({ "new_password": new_pw })),
    )
    .await;
    assert_eq!(s.as_u16(), 204);

    // login with new password succeeds
    let (s, _) = json_req(
        &router,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(json!({ "email": "player@test.com", "password": new_pw })),
    )
    .await;
    assert_eq!(s.as_u16(), 200, "login with reset password should succeed");
}

#[tokio::test]
async fn admin_stats_returns_counts() {
    let (router, db) = skip_no_db!();
    let (admin_tok, _, _, _) = setup_admin_and_user(&router, &db).await;

    let (s, body) = json_req(
        &router,
        "GET",
        "/api/v1/admin/stats",
        Some(&admin_tok),
        None,
    )
    .await;
    assert_eq!(s.as_u16(), 200);
    assert!(body["users"].as_i64().unwrap() >= 2);
    assert!(body["campaigns"].as_i64().is_some());
    assert!(body["characters"].as_i64().is_some());
}

#[tokio::test]
async fn admin_stats_rejects_non_admin() {
    let (router, db) = skip_no_db!();
    let (admin_tok, _, _, _) = setup_admin_and_user(&router, &db).await;

    let (s, body) = json_req(
        &router,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(json!({ "email": "player@test.com", "password": TEST_PASSWORD })),
    )
    .await;
    let user_tok = body["token"].as_str().unwrap().to_string();

    let (s2, _) = json_req(&router, "GET", "/api/v1/admin/stats", Some(&user_tok), None).await;
    assert_eq!(s2.as_u16(), 403);

    let _ = (s, admin_tok);
}

#[tokio::test]
async fn admin_list_campaigns() {
    let (router, db) = skip_no_db!();
    let (admin_tok, _, _, _) = setup_admin_and_user(&router, &db).await;

    // create a campaign as admin
    let (s, _) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&admin_tok),
        Some(json!({ "name": "Test Campaign" })),
    )
    .await;
    assert_eq!(s.as_u16(), 201);

    let (s, body) = json_req(
        &router,
        "GET",
        "/api/v1/admin/campaigns",
        Some(&admin_tok),
        None,
    )
    .await;
    assert_eq!(s.as_u16(), 200);
    let arr = body.as_array().unwrap();
    assert!(arr.iter().any(|c| c["name"] == "Test Campaign"));
}

#[tokio::test]
async fn admin_backup_returns_data() {
    let (router, db) = skip_no_db!();
    let (admin_tok, _, _, _) = setup_admin_and_user(&router, &db).await;

    let (s, body) = json_req(
        &router,
        "GET",
        "/api/v1/admin/backup",
        Some(&admin_tok),
        None,
    )
    .await;
    assert_eq!(s.as_u16(), 200);
    assert_eq!(body["version"], 1);
    assert!(body["exported_at"].as_str().is_some());
    assert!(body["tables"]["users"].as_array().is_some());
    assert!(body["tables"]["campaigns"].as_array().is_some());
    assert!(body["tables"]["spells"].as_array().is_some());
    assert!(body["tables"]["sessions_auth"].as_array().is_some());
    assert!(body["tables"]["conditions"].as_array().is_some());
}

#[tokio::test]
async fn admin_backup_rejects_non_admin() {
    let (router, db) = skip_no_db!();
    let (_, _, user_tok, _) = setup_admin_and_user(&router, &db).await;

    let (s, _) = json_req(
        &router,
        "GET",
        "/api/v1/admin/backup",
        Some(&user_tok),
        None,
    )
    .await;
    assert_eq!(s.as_u16(), 403);
}

#[tokio::test]
async fn admin_restore_replaces_data() {
    let (router, db) = skip_no_db!();
    let (admin_tok, _, _, _) = setup_admin_and_user(&router, &db).await;

    // Create a campaign
    let (s, _camp_body) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&admin_tok),
        Some(json!({ "name": "Before Restore" })),
    )
    .await;
    assert_eq!(s.as_u16(), 201);

    // Get backup
    let (s, backup) = json_req(
        &router,
        "GET",
        "/api/v1/admin/backup",
        Some(&admin_tok),
        None,
    )
    .await;
    assert_eq!(s.as_u16(), 200);

    // Verify campaign exists before restore
    let (s, campaigns) =
        json_req(&router, "GET", "/api/v1/campaigns", Some(&admin_tok), None).await;
    assert_eq!(s.as_u16(), 200);
    assert!(
        campaigns
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["name"] == "Before Restore")
    );

    // Restore the backup (which should recreate the same data)
    let (s, _) = json_req(
        &router,
        "POST",
        "/api/v1/admin/restore",
        Some(&admin_tok),
        Some(json!({ "backup": backup })),
    )
    .await;
    assert_eq!(s.as_u16(), 204);

    // Verify campaign still exists after restore
    let (s, campaigns) =
        json_req(&router, "GET", "/api/v1/campaigns", Some(&admin_tok), None).await;
    assert_eq!(s.as_u16(), 200);
    assert!(
        campaigns
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["name"] == "Before Restore")
    );
}

#[tokio::test]
async fn admin_restore_rejects_non_admin() {
    let (router, db) = skip_no_db!();
    let (_, _, user_tok, _) = setup_admin_and_user(&router, &db).await;

    let (s, _) = json_req(
        &router,
        "POST",
        "/api/v1/admin/restore",
        Some(&user_tok),
        Some(json!({ "backup": { "version": 1, "exported_at": "2024-01-01", "tables": {} } })),
    )
    .await;
    assert_eq!(s.as_u16(), 403);
}

#[tokio::test]
async fn admin_restore_rejects_invalid_column_names() {
    let (router, db) = skip_no_db!();
    let (admin_tok, _, _, _) = setup_admin_and_user(&router, &db).await;

    let (s, _body) = json_req(
        &router,
        "POST",
        "/api/v1/admin/restore",
        Some(&admin_tok),
        Some(json!({
            "backup": {
                "version": 1,
                "exported_at": "2024-01-01",
                "tables": {
                    "users": [{
                        "id": "00000000-0000-0000-0000-000000000001",
                        "display_name": "ok",
                        "malicious; DROP TABLE users;--": "injected"
                    }]
                }
            }
        })),
    )
    .await;
    assert_eq!(s.as_u16(), 400);

    // Numeric-starting column names also rejected
    let (s2, _) = json_req(
        &router,
        "POST",
        "/api/v1/admin/restore",
        Some(&admin_tok),
        Some(json!({
            "backup": {
                "version": 1,
                "exported_at": "2024-01-01",
                "tables": {
                    "users": [{
                        "1invalid_start": "bad"
                    }]
                }
            }
        })),
    )
    .await;
    assert_eq!(s2.as_u16(), 400);
}
