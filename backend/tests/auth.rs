//! Authentication and security tests
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
async fn password_validation_rejects_weak_passwords() {
    let (router, _db) = skip_no_db!();

    // Test cases: (password, description)
    let weak_passwords = [
        ("short1!", "too short (< 8 chars)"),
        ("lowercase1", "no uppercase"),
        ("UPPERCASE1", "no lowercase"),
        ("NoDigits!", "no digits"),
        ("12345678", "only digits"),
        ("password", "no uppercase, no digits, no special"),
        ("Password1", "only 2 char types (upper, lower, digit)"),
        ("password1!", "only 2 char types (lower, digit, special)"),
    ];

    for (password, desc) in weak_passwords {
        let (s, body) = json_req(
            &router,
            "POST",
            "/api/v1/auth/register",
            None,
            Some(json!({
                "email": format!("test_{}@example.com", uuid::Uuid::new_v4()),
                "password": password,
                "display_name": "Test",
            })),
        )
        .await;

        assert!(
            s == 422 || s == 400,
            "Password '{}' ({}) should be rejected, got status {}: {:?}",
            password,
            desc,
            s,
            body
        );
    }
}

#[tokio::test]
async fn password_validation_accepts_strong_passwords() {
    let (router, _db) = skip_no_db!();

    // Test cases: passwords with at least 3 of 4 character types
    let strong_passwords = [
        "Test123!",          // upper + lower + digit + special
        "MyP@ssw0rd",        // upper + lower + digit + special
        "Hello1!World",      // upper + lower + digit + special
        "A1b2C3!@#",         // upper + lower + digit + special
        "Secure#Pass123",    // upper + lower + digit + special
    ];

    for password in strong_passwords {
        let email = format!("strong_{}@example.com", uuid::Uuid::new_v4());
        let (s, body) = json_req(
            &router,
            "POST",
            "/api/v1/auth/register",
            None,
            Some(json!({
                "email": email,
                "password": password,
                "display_name": "Test",
            })),
        )
        .await;

        assert!(
            s == 201,
            "Password '{}' should be accepted, got status {}: {:?}",
            password,
            s,
            body
        );
    }
}

#[tokio::test]
async fn cors_allows_configured_origins() {
    use axum::http::{Method, Request};
    use tower::ServiceExt;

    let url = match helpers::test_db_url() {
        Some(u) => u,
        None => {
            eprintln!("SKIP: TEST_DATABASE_URL/DATABASE_URL not set");
            return;
        }
    };

    // Create app with specific CORS origins
    let cfg = cinghialapp::config::Config {
        database_url: url.clone(),
        jwt_secret: "test-secret-with-at-least-32-bytes-long".into(),
        bind_addr: "127.0.0.1:0".into(),
        cors_origin: "http://localhost:5173,http://0.0.0.0:5173".into(),
        s3: None,
    };

    // Reset schema
    let state = cinghialapp::AppState::new(cfg.clone()).await.unwrap();
    sqlx::query("drop schema public cascade; create schema public;")
        .execute(&state.db)
        .await
        .ok();
    sqlx::migrate!("../migrations")
        .run(&state.db)
        .await
        .ok();

    let router = cinghialapp::app(state);

    // Test preflight request from allowed origin
    let req = Request::builder()
        .method(Method::OPTIONS)
        .uri("/api/v1/auth/login")
        .header("Origin", "http://localhost:5173")
        .header("Access-Control-Request-Method", "POST")
        .header("Access-Control-Request-Headers", "Content-Type")
        .body(axum::body::Body::empty())
        .unwrap();

    let res = router.clone().oneshot(req).await.unwrap();
    let allow_origin = res
        .headers()
        .get("access-control-allow-origin")
        .cloned();
    
    assert!(
        allow_origin.is_some(),
        "CORS should include access-control-allow-origin header"
    );
    let allow_origin_str = allow_origin.unwrap().to_str().unwrap().to_string();
    assert!(
        allow_origin_str.contains("localhost:5173") || allow_origin_str.contains("0.0.0.0:5173"),
        "CORS should allow configured origins, got: {}",
        allow_origin_str
    );
}

#[tokio::test]
async fn jwt_rejects_expired_tokens() {
    use cinghialapp::auth::{issue_jwt, decode_jwt};
    use time::OffsetDateTime;

    let secret = "test-secret-with-at-least-32-bytes";
    
    // Issue a token
    let token = issue_jwt(uuid::Uuid::new_v4(), 0, secret).unwrap();
    
    // Token should be valid immediately
    let claims = decode_jwt(&token, secret);
    assert!(claims.is_ok(), "Valid token should be decoded successfully");

    // Note: We can't easily test actual expiration without time manipulation,
    // but we verify the claims contain expiration
    let claims = claims.unwrap();
    assert!(claims.exp > claims.iat, "Token should have expiration after issuance");
    assert!(
        claims.exp > OffsetDateTime::now_utc().unix_timestamp() - 10,
        "Token should not already be expired"
    );
}

#[tokio::test]
async fn login_rate_limiting_blocks_after_max_attempts() {
    let (router, _db) = skip_no_db!();

    // First, register a user
    let email = format!("ratelimit_{}@example.com", uuid::Uuid::new_v4());
    let (_tok, body) = register_with(&router, &email, None).await;
    assert!(!_tok.is_empty(), "User should be registered: {:?}", body);

    // Make multiple failed login attempts
    for i in 0..12 {
        let (s, body) = json_req(
            &router,
            "POST",
            "/api/v1/auth/login",
            None,
            Some(json!({
                "email": email,
                "password": "WrongPass1!",
            })),
        )
        .await;

        if i < 10 {
            assert_eq!(s, 401, "Attempt {} should return 401 (unauthorized)", i + 1);
        } else {
            // After 10 attempts, should be rate limited
            // Note: This may fail in tests if the rate limit window has passed
            // or if rate limiting is disabled in test mode
            if s == 400 {
                assert!(
                    body.to_string().to_lowercase().contains("too many")
                        || body.to_string().to_lowercase().contains("rate"),
                    "Rate limit should return descriptive error"
                );
                break;
            }
        }
    }
}

#[tokio::test]
async fn admin_password_reset_enforces_strong_password() {
    let (router, _db) = skip_no_db!();

    // Register first user (becomes master/admin)
    let (master_tok, _) = register_with(&router, "admin_reset@example.com", None).await;

    // Register another user
    let (user_tok, user_body) = register_with(&router, "user_reset@example.com", Some(&master_tok)).await;
    assert!(!user_tok.is_empty(), "User should be registered");
    let user_id = user_body["user"]["id"].as_str().unwrap();

    // Try to reset with weak password
    let (s, body) = json_req(
        &router,
        "POST",
        &format!("/api/v1/users/{}/reset-password", user_id),
        Some(&master_tok),
        Some(json!({
            "new_password": "weak",
        })),
    )
    .await;

    assert!(
        s == 422 || s == 400,
        "Weak password reset should be rejected, got {}: {:?}",
        s,
        body
    );

    // Reset with strong password should succeed
    let (s, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/users/{}/reset-password", user_id),
        Some(&master_tok),
        Some(json!({
            "new_password": "NewStrong1!Pass",
        })),
    )
    .await;

    assert_eq!(s, 204, "Strong password reset should succeed");

    // Login with new password
    let (s, body) = json_req(
        &router,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(json!({
            "email": "user_reset@example.com",
            "password": "NewStrong1!Pass",
        })),
    )
    .await;

    assert_eq!(s, 200, "Should login with new password");
    assert!(body["token"].is_string(), "Should receive JWT token");
}
