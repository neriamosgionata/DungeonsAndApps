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
use tokio::io::AsyncWriteExt;
use futures::{StreamExt, TryStreamExt};
use uuid::Uuid;
use validator::Validate;
use once_cell::sync::Lazy;

// Simple per-user token bucket: max N uploads per window with bounded memory.
// Uses LRU-style eviction to prevent unbounded growth.
const UPLOAD_WINDOW_SECS: u64 = 60;
const UPLOAD_MAX_PER_WINDOW: usize = 20;
const MAX_TRACKED_USERS: usize = 10000; // Prevent unbounded memory growth

static UPLOAD_BUCKETS: Lazy<Mutex<HashMap<Uuid, UploadBucket>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Debug)]
struct UploadBucket {
    timestamps: Vec<Instant>,
    last_access: Instant,
}

fn check_rate(uid: Uuid) -> AppResult<()> {
    let mut map = UPLOAD_BUCKETS.lock().unwrap();
    let now = Instant::now();
    let window = Duration::from_secs(UPLOAD_WINDOW_SECS);
    
    // Cleanup: if map is getting large, remove stale entries
    if map.len() > MAX_TRACKED_USERS {
        let stale_threshold = now - (window * 2);
        let stale_keys: Vec<Uuid> = map
            .iter()
            .filter(|(_, v)| v.last_access < stale_threshold)
            .map(|(k, _)| *k)
            .take(MAX_TRACKED_USERS / 2)
            .collect();
        for key in stale_keys {
            map.remove(&key);
        }
    }
    
    let bucket = map.entry(uid).or_insert(UploadBucket {
        timestamps: Vec::with_capacity(4),
        last_access: now,
    });
    
    // Remove attempts outside the window
    bucket.timestamps.retain(|t| now.duration_since(*t) < window);
    bucket.last_access = now;
    
    if bucket.timestamps.len() >= UPLOAD_MAX_PER_WINDOW {
        return Err(AppError::BadRequest(
            format!("upload rate limit: max {UPLOAD_MAX_PER_WINDOW} per {UPLOAD_WINDOW_SECS}s")));
    }
    bucket.timestamps.push(now);
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
        public_url: public_url(&s, &key)?,
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

        // Stream chunks to a temp file to avoid loading large uploads into memory.
        let temp_path = std::env::temp_dir().join(format!("cinghialapp-upload-{}", Uuid::new_v4()));
        let mut file = tokio::fs::File::create(&temp_path).await
            .map_err(|e| AppError::Other(anyhow::anyhow!(e)))?;
        let mut total: usize = 0;
        let mut chunk_stream = field.into_stream();
        while let Some(chunk) = chunk_stream.next().await {
            let data = chunk.map_err(|e| AppError::BadRequest(e.to_string()))?;
            total += data.len();
            if total > MAX_BYTES {
                let _ = tokio::fs::remove_file(&temp_path).await;
                return Err(AppError::BadRequest("file too large (max 32MB)".into()));
            }
            file.write_all(&data).await.map_err(|e| AppError::Other(anyhow::anyhow!(e)))?;
        }
        file.shutdown().await.map_err(|e| AppError::Other(anyhow::anyhow!(e)))?;
        drop(file);

        let ext = std::path::Path::new(&orig)
            .extension().and_then(|e| e.to_str()).unwrap_or_else(|| match ct.as_str() {
                "image/png" => "png", "image/jpeg" => "jpg", "image/webp" => "webp",
                "image/gif" => "gif", "image/svg+xml" => "svg", _ => "bin",
            })
            .to_string();
        let safe_ext = ext.chars().filter(|c| c.is_ascii_alphanumeric()).take(8).collect::<String>();
        let key = format!("{}/{}.{}" , sanitize_kind(&kind), Uuid::new_v4(), safe_ext);

        let upload_res = async {
            let body = ByteStream::from_path(&temp_path).await
                .map_err(|e| AppError::Other(anyhow::anyhow!(e)))?;
            cli.put_object()
                .bucket(&bucket)
                .key(&key)
                .content_type(ct.clone())
                .body(body)
                .send().await
                .map_err(|e| AppError::Other(anyhow::anyhow!(e.to_string())))
        }.await;
        // Clean up temp file regardless of success/failure
        let _ = tokio::fs::remove_file(&temp_path).await;
        upload_res?;

        return Ok(Json(UploadRes {
            url: public_url(&s, &key)?,
            key,
            size: total,
            content_type: ct,
        }));
    }

    Err(AppError::BadRequest("missing 'file' field".into()))
}

const ALLOWED_KINDS: &[&str] = &["avatars", "maps", "portraits", "tokens", "npcs", "misc", "map", "npc", "pin", "campaign", "character"];

fn sanitize_kind(k: &str) -> String {
    let filtered: String = k.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(40).collect();
    // Validate against whitelist; fallback to misc if not allowed
    if ALLOWED_KINDS.contains(&filtered.as_str()) {
        filtered
    } else {
        "misc".to_string()
    }
}

fn public_url(s: &AppState, key: &str) -> AppResult<String> {
    let cfg = s.cfg.s3.as_ref()
        .ok_or_else(|| AppError::BadRequest("S3 not configured".into()))?;
    let ep = cfg.endpoint.trim_end_matches('/');
    Ok(format!("{ep}/{}/{}", cfg.bucket, key))
}
