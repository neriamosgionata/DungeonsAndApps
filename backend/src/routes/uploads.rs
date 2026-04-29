use crate::{
    AppState,
    error::{AppError, AppResult},
    extract::AuthUser,
};
use aws_credential_types::Credentials;
use aws_sdk_s3::{
    Client, Config as S3Cfg,
    presigning::PresigningConfig,
    primitives::ByteStream,
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Multipart, State},
    routing::post,
};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use std::sync::Mutex;
use std::collections::HashMap;
use uuid::Uuid;
use validator::Validate;
use once_cell::sync::Lazy;

// Simple per-user token bucket: max N uploads per window.
// Keeps runaway clients from exhausting disk. Resets per-process restart.
const UPLOAD_WINDOW_SECS: u64 = 60;
const UPLOAD_MAX_PER_WINDOW: usize = 20;
static UPLOAD_BUCKETS: Lazy<Mutex<HashMap<Uuid, Vec<Instant>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn check_rate(uid: Uuid) -> AppResult<()> {
    let mut map = UPLOAD_BUCKETS.lock().unwrap();
    let now = Instant::now();
    let window = Duration::from_secs(UPLOAD_WINDOW_SECS);
    let entry = map.entry(uid).or_default();
    entry.retain(|t| now.duration_since(*t) < window);
    if entry.len() >= UPLOAD_MAX_PER_WINDOW {
        return Err(AppError::BadRequest(
            format!("upload rate limit: max {UPLOAD_MAX_PER_WINDOW} per {UPLOAD_WINDOW_SECS}s")));
    }
    entry.push(now);
    Ok(())
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/uploads", post(presign))
        .route("/uploads/file", post(upload_proxy))
        // raise body limit on upload routes (default axum = 2MB; we accept up to MAX_BYTES)
        .layer(DefaultBodyLimit::max(MAX_BYTES + 1024 * 1024))
}

// Build an S3/MinIO client from app config.
fn client(s: &AppState) -> AppResult<(Client, String)> {
    let cfg = s.cfg.s3.as_ref()
        .ok_or_else(|| AppError::BadRequest("S3 not configured — set S3_ENDPOINT/BUCKET/ACCESS_KEY/SECRET_KEY".into()))?;
    let creds = Credentials::new(&cfg.access_key, &cfg.secret_key, None, None, "cinghialapp");
    let s3cfg = S3Cfg::builder()
        .region(aws_sdk_s3::config::Region::new(cfg.region.clone()))
        .endpoint_url(&cfg.endpoint)
        .credentials_provider(creds)
        .force_path_style(true)
        .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
        .build();
    Ok((Client::from_conf(s3cfg), cfg.bucket.clone()))
}

// ---------- presigned PUT (client uploads directly) ---------------------

#[derive(Debug, Deserialize, Validate)]
pub struct PresignReq {
    #[validate(length(min = 1, max = 120))]
    pub kind: String,
    #[validate(length(min = 1, max = 255))]
    pub filename: String,
    #[validate(length(min = 1, max = 120))]
    pub content_type: String,
}

#[derive(Debug, Serialize)]
pub struct PresignRes {
    pub url: String,
    pub key: String,
    pub expires_in_sec: u64,
    pub public_url: String,
}

async fn presign(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Json(body): Json<PresignReq>,
) -> AppResult<Json<PresignRes>> {
    body.validate()?;
    check_rate(uid)?;
    let (cli, bucket) = client(&s)?;

    let ext = std::path::Path::new(&body.filename)
        .extension().and_then(|e| e.to_str()).unwrap_or("bin");
    let safe_ext = ext.chars().filter(|c| c.is_ascii_alphanumeric()).take(8).collect::<String>();
    let key = format!("{}/{}.{}", sanitize_kind(&body.kind), Uuid::new_v4(), safe_ext);

    let presign_cfg = PresigningConfig::expires_in(Duration::from_secs(900))
        .map_err(|e| AppError::Other(anyhow::anyhow!(e)))?;
    let req = cli.put_object()
        .bucket(&bucket).key(&key)
        .content_type(&body.content_type)
        .presigned(presign_cfg).await
        .map_err(|e| AppError::Other(anyhow::anyhow!(e.to_string())))?;

    Ok(Json(PresignRes {
        url: req.uri().to_string(),
        public_url: public_url(&s, &key),
        key,
        expires_in_sec: 900,
    }))
}

// ---------- server-side proxy multipart upload --------------------------
// Browser CORS against MinIO is a pain; this endpoint takes `file` multipart
// and PUTs to S3 for the client. Returns the public URL to embed.

const MAX_BYTES: usize = 32 * 1024 * 1024; // 32 MB (maps can be large JPEGs)
const ALLOWED_MIME: &[&str] = &[
    "image/png", "image/jpeg", "image/webp", "image/gif", "image/svg+xml",
];

#[derive(Debug, Serialize)]
pub struct UploadRes {
    pub url: String,
    pub key: String,
    pub size: usize,
    pub content_type: String,
}

async fn upload_proxy(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    mut mp: Multipart,
) -> AppResult<Json<UploadRes>> {
    check_rate(uid)?;
    let (cli, bucket) = client(&s)?;
    let mut kind: String = "misc".into();

    while let Some(field) = mp.next_field().await.map_err(|e| AppError::BadRequest(e.to_string()))? {
        let name = field.name().unwrap_or("").to_string();

        if name == "kind" {
            kind = field.text().await.map_err(|e| AppError::BadRequest(e.to_string()))?;
            continue;
        }

        if name != "file" { continue; }

        let orig = field.file_name().unwrap_or("upload.bin").to_string();
        let ct = field.content_type().unwrap_or("application/octet-stream").to_string();
        if !ALLOWED_MIME.contains(&ct.as_str()) {
            return Err(AppError::BadRequest(format!("unsupported content type: {ct}")));
        }

        let bytes = field.bytes().await.map_err(|e| AppError::BadRequest(e.to_string()))?;
        if bytes.len() > MAX_BYTES {
            return Err(AppError::BadRequest("file too large (max 32MB)".into()));
        }

        let ext = std::path::Path::new(&orig)
            .extension().and_then(|e| e.to_str()).unwrap_or_else(|| match ct.as_str() {
                "image/png" => "png", "image/jpeg" => "jpg", "image/webp" => "webp",
                "image/gif" => "gif", "image/svg+xml" => "svg", _ => "bin",
            })
            .to_string();
        let safe_ext = ext.chars().filter(|c| c.is_ascii_alphanumeric()).take(8).collect::<String>();
        let key = format!("{}/{}.{}", sanitize_kind(&kind), Uuid::new_v4(), safe_ext);
        let size = bytes.len();

        cli.put_object()
            .bucket(&bucket)
            .key(&key)
            .content_type(ct.clone())
            .body(ByteStream::from(bytes.to_vec()))
            .send().await
            .map_err(|e| AppError::Other(anyhow::anyhow!(e.to_string())))?;

        return Ok(Json(UploadRes {
            url: public_url(&s, &key),
            key,
            size,
            content_type: ct,
        }));
    }

    Err(AppError::BadRequest("missing 'file' field".into()))
}

fn sanitize_kind(k: &str) -> String {
    k.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(40).collect()
}

fn public_url(s: &AppState, key: &str) -> String {
    // path-style URL against the endpoint: {endpoint}/{bucket}/{key}
    let cfg = s.cfg.s3.as_ref().expect("s3 config present");
    let ep = cfg.endpoint.trim_end_matches('/');
    format!("{ep}/{}/{}", cfg.bucket, key)
}
