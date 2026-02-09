//! Simple in-memory per-IP token bucket rate limiter.
//!
//! Implements an axum middleware that rejects requests with HTTP 429
//! when a client exceeds the allowed request rate.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::ConnectInfo,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use tokio::sync::Mutex;
use tracing::warn;

// ---------------------------------------------------------------------------
// Token bucket
// ---------------------------------------------------------------------------

/// A single token bucket for one IP address.
#[derive(Debug, Clone)]
struct TokenBucket {
    /// Remaining tokens.
    tokens: f64,
    /// Last time we refilled.
    last_refill: Instant,
}

impl TokenBucket {
    fn new(capacity: f64) -> Self {
        Self {
            tokens: capacity,
            last_refill: Instant::now(),
        }
    }

    /// Refill tokens based on elapsed time, then try to consume one.
    fn try_consume(&mut self, rate: f64, capacity: f64) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.last_refill = now;

        // Add tokens based on elapsed time, capped at capacity.
        self.tokens = (self.tokens + elapsed * rate).min(capacity);

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Rate limiter state
// ---------------------------------------------------------------------------

/// Shared rate limiter state.
#[derive(Clone)]
pub struct RateLimiter {
    /// Per-IP buckets.
    buckets: Arc<Mutex<HashMap<IpAddr, TokenBucket>>>,
    /// Tokens added per second.
    rate: f64,
    /// Maximum burst (bucket capacity).
    capacity: f64,
}

impl RateLimiter {
    /// Create a new rate limiter.
    ///
    /// # Arguments
    ///
    /// * `rate` - Tokens per second (sustained request rate).
    /// * `capacity` - Maximum burst size.
    pub fn new(rate: f64, capacity: f64) -> Self {
        Self {
            buckets: Arc::new(Mutex::new(HashMap::new())),
            rate,
            capacity,
        }
    }

    /// Check whether a request from `ip` is allowed.
    pub async fn check(&self, ip: IpAddr) -> bool {
        let mut buckets = self.buckets.lock().await;
        let bucket = buckets
            .entry(ip)
            .or_insert_with(|| TokenBucket::new(self.capacity));
        bucket.try_consume(self.rate, self.capacity)
    }

    /// Remove stale entries that have been idle for a long time.
    ///
    /// Call this periodically (e.g. every 5 minutes) to bound memory.
    pub async fn purge_stale(&self, max_idle_secs: f64) {
        let mut buckets = self.buckets.lock().await;
        let now = Instant::now();
        buckets.retain(|_, bucket| {
            now.duration_since(bucket.last_refill).as_secs_f64() < max_idle_secs
        });
    }
}

impl Default for RateLimiter {
    /// Default: 10 requests/second, burst of 30.
    fn default() -> Self {
        Self::new(10.0, 30.0)
    }
}

// ---------------------------------------------------------------------------
// Axum middleware
// ---------------------------------------------------------------------------

/// Axum middleware function that applies rate limiting based on the
/// client's IP address.
///
/// Usage with axum:
/// ```ignore
/// let limiter = RateLimiter::default();
/// let app = Router::new()
///     .route("/api", get(handler))
///     .layer(axum::middleware::from_fn_with_state(
///         limiter,
///         rate_limit_middleware,
///     ));
/// ```
pub async fn rate_limit_middleware(
    axum::extract::State(limiter): axum::extract::State<RateLimiter>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Try to extract the peer IP from ConnectInfo, or fall back to
    // a header-based approach for proxied setups.
    let ip = extract_client_ip(&req);

    if let Some(ip) = ip {
        if !limiter.check(ip).await {
            warn!(ip = %ip, "Rate limit exceeded");
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
    }

    Ok(next.run(req).await)
}

/// Extract the client IP from the request.
///
/// Checks `ConnectInfo` first, then falls back to `X-Forwarded-For`
/// and `X-Real-IP` headers (useful behind a reverse proxy).
fn extract_client_ip<B>(req: &Request<B>) -> Option<IpAddr> {
    // 1. ConnectInfo (set by axum when using `into_make_service_with_connect_info`)
    if let Some(connect_info) = req.extensions().get::<ConnectInfo<std::net::SocketAddr>>() {
        return Some(connect_info.0.ip());
    }

    // 2. X-Forwarded-For header (first IP)
    if let Some(forwarded) = req.headers().get("x-forwarded-for") {
        if let Ok(value) = forwarded.to_str() {
            if let Some(first) = value.split(',').next() {
                if let Ok(ip) = first.trim().parse::<IpAddr>() {
                    return Some(ip);
                }
            }
        }
    }

    // 3. X-Real-IP header
    if let Some(real_ip) = req.headers().get("x-real-ip") {
        if let Ok(value) = real_ip.to_str() {
            if let Ok(ip) = value.trim().parse::<IpAddr>() {
                return Some(ip);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_allows_burst() {
        let limiter = RateLimiter::new(10.0, 5.0);
        let ip: IpAddr = "127.0.0.1".parse().unwrap();

        // First 5 requests should be allowed (burst capacity).
        for _ in 0..5 {
            assert!(limiter.check(ip).await);
        }

        // 6th request should be rejected (bucket exhausted).
        assert!(!limiter.check(ip).await);
    }

    #[tokio::test]
    async fn test_rate_limiter_different_ips() {
        let limiter = RateLimiter::new(10.0, 2.0);
        let ip1: IpAddr = "10.0.0.1".parse().unwrap();
        let ip2: IpAddr = "10.0.0.2".parse().unwrap();

        assert!(limiter.check(ip1).await);
        assert!(limiter.check(ip1).await);
        assert!(!limiter.check(ip1).await);

        // Different IP should have its own bucket.
        assert!(limiter.check(ip2).await);
    }

    #[tokio::test]
    async fn test_purge_stale() {
        let limiter = RateLimiter::new(10.0, 5.0);
        let ip: IpAddr = "192.168.1.1".parse().unwrap();
        assert!(limiter.check(ip).await);

        // Purge with max_idle = 0 should remove everything.
        limiter.purge_stale(0.0).await;

        let buckets = limiter.buckets.lock().await;
        assert!(buckets.is_empty());
    }
}
