//! User self-management tests (update_me, change_password)
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

#[tokio::test]
async fn update_me_changes_display_name() {
    let (router, _db) = skip_no_db!();
    let (token, _user) = register(&router, "alice@test.com").await;

    let (s, body) = json_req(
        &router, "PATCH", "/api/v1/users/me",
        Some(&token),
        Some(json!({ "display_name": "Alice Updated" })),
    ).await;
    assert_eq!(s, 200);
    assert_eq!(body["display_name"], "Alice Updated");

    let (s2, body2) = json_req(&router, "GET", "/api/v1/auth/me", Some(&token), None).await;
    assert_eq!(s2, 200);
    assert_eq!(body2["display_name"], "Alice Updated");
}

#[tokio::test]
async fn update_me_changes_language() {
    let (router, _db) = skip_no_db!();
    let (token, _user) = register(&router, "bob@test.com").await;

    let (s, body) = json_req(
        &router, "PATCH", "/api/v1/users/me",
        Some(&token),
        Some(json!({ "language": "it" })),
    ).await;
    assert_eq!(s, 200);
    assert_eq!(body["language"], "it");
}

#[tokio::test]
async fn update_me_changes_both_fields() {
    let (router, _db) = skip_no_db!();
    let (token, _user) = register(&router, "carol@test.com").await;

    let (s, body) = json_req(
        &router, "PATCH", "/api/v1/users/me",
        Some(&token),
        Some(json!({ "display_name": "Carol IT", "language": "it" })),
    ).await;
    assert_eq!(s, 200);
    assert_eq!(body["display_name"], "Carol IT");
    assert_eq!(body["language"], "it");
}

#[tokio::test]
async fn update_me_rejects_empty_display_name() {
    let (router, _db) = skip_no_db!();
    let (token, _user) = register(&router, "dave@test.com").await;

    let (s, _body) = json_req(
        &router, "PATCH", "/api/v1/users/me",
        Some(&token),
        Some(json!({ "display_name": "" })),
    ).await;
    assert!(s == 422, "expected 422, got {s}");
}

#[tokio::test]
async fn update_me_rejects_long_display_name() {
    let (router, _db) = skip_no_db!();
    let (token, _user) = register(&router, "eve@test.com").await;

    let (s, _body) = json_req(
        &router, "PATCH", "/api/v1/users/me",
        Some(&token),
        Some(json!({ "display_name": "a".repeat(65) })),
    ).await;
    assert!(s == 422, "expected 422, got {s}");
}

#[tokio::test]
async fn update_me_rejects_invalid_language() {
    let (router, _db) = skip_no_db!();
    let (token, _user) = register(&router, "frank@test.com").await;

    let (s, _body) = json_req(
        &router, "PATCH", "/api/v1/users/me",
        Some(&token),
        Some(json!({ "language": "fr" })),
    ).await;
    assert!(s == 400, "expected 400, got {s}");
}

#[tokio::test]
async fn update_me_rejects_unauthenticated() {
    let (router, _db) = skip_no_db!();

    let (s, _body) = json_req(
        &router, "PATCH", "/api/v1/users/me",
        None,
        Some(json!({ "display_name": "Anonymous" })),
    ).await;
    assert_eq!(s, 401);
}

#[tokio::test]
async fn change_password_succeeds_with_correct_current() {
    let (router, _db) = skip_no_db!();
    let (token, _user) = register(&router, "grace@test.com").await;

    let (s, _body) = json_req(
        &router, "POST", "/api/v1/users/me/change-password",
        Some(&token),
        Some(json!({ "current_password": TEST_PASSWORD, "new_password": "NewSecure1!Password" })),
    ).await;
    assert_eq!(s, 204);

    // old password should fail now
    let (s2, _body2) = json_req(&router, "POST", "/api/v1/auth/login",
        None,
        Some(json!({ "email": "grace@test.com", "password": TEST_PASSWORD })),
    ).await;
    assert_eq!(s2, 401);

    // new password should work
    let (s3, _body3) = json_req(&router, "POST", "/api/v1/auth/login",
        None,
        Some(json!({ "email": "grace@test.com", "password": "NewSecure1!Password" })),
    ).await;
    assert_eq!(s3, 200);
}

#[tokio::test]
async fn change_password_rejects_wrong_current() {
    let (router, _db) = skip_no_db!();
    let (token, _user) = register(&router, "hank@test.com").await;

    let (s, _body) = json_req(
        &router, "POST", "/api/v1/users/me/change-password",
        Some(&token),
        Some(json!({ "current_password": "WrongPassword1!", "new_password": "NewSecure1!Xyz" })),
    ).await;
    assert_eq!(s, 403);
}

#[tokio::test]
async fn change_password_rejects_weak_new_password() {
    let (router, _db) = skip_no_db!();
    let (token, _user) = register(&router, "ivy@test.com").await;

    let (s, _body) = json_req(
        &router, "POST", "/api/v1/users/me/change-password",
        Some(&token),
        Some(json!({ "current_password": TEST_PASSWORD, "new_password": "short" })),
    ).await;
    assert!(s == 422, "expected 422, got {s}");
}

#[tokio::test]
async fn change_password_rejects_unauthenticated() {
    let (router, _db) = skip_no_db!();

    let (s, _body) = json_req(
        &router, "POST", "/api/v1/users/me/change-password",
        None,
        Some(json!({ "current_password": "x", "new_password": "y" })),
    ).await;
    assert_eq!(s, 401);
}

#[tokio::test]
async fn update_me_no_fields_is_noop() {
    let (router, _db) = skip_no_db!();
    let (token, user) = register(&router, "jack@test.com").await;

    let (s, body) = json_req(
        &router, "PATCH", "/api/v1/users/me",
        Some(&token),
        Some(json!({})),
    ).await;
    assert_eq!(s, 200);
    assert_eq!(body["display_name"], user["display_name"]);
    assert_eq!(body["language"], user["language"]);
}
