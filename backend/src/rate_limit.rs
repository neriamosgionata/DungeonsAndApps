use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};

const WINDOW: Duration = Duration::from_secs(60);
const MAX_REQUESTS: u32 = 600; // 60 req/s sustained — high for LAN; tighten in prod

static HTTP_RATE: Lazy<Arc<DashMap<String, (u32, Instant)>>> =
    Lazy::new(|| Arc::new(DashMap::new()));

pub async fn http_rate_limit(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let key = addr.ip().to_string();
    let now = Instant::now();
    let mut blocked = false;

    HTTP_RATE
        .entry(key.clone())
        .and_modify(|(count, first)| {
            if now.duration_since(*first) > WINDOW {
                *count = 1;
                *first = now;
            } else if *count >= MAX_REQUESTS {
                blocked = true;
            } else {
                *count += 1;
            }
        })
        .or_insert((1, now));

    if blocked {
        tracing::warn!(ip = %key, "HTTP rate limit exceeded");
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    // Periodic cleanup (~1% chance per request)
    use rand::Rng;
    if rand::rng().random_range(0..100u8) == 0 {
        HTTP_RATE.retain(|_, (_, first)| now.duration_since(*first) <= WINDOW * 2);
    }

    Ok(next.run(req).await)
}
