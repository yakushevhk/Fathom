//! API key authentication and per-client rate limiting middleware.

use crate::AppState;
use axum::{
    extract::{ConnectInfo, Request, State},
    http::{header, HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Environment variable holding comma-separated API keys. When unset or
/// empty, authentication is disabled (open access).
pub const API_KEYS_ENV: &str = "PARALLEL_RESEARCH_API_KEYS";

/// Metadata about a registered API key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyInfo {
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Registry of valid API keys.
///
/// When no keys are registered, authentication is disabled and every
/// request is treated as `anonymous`.
#[derive(Debug, Clone, Default)]
pub struct ApiKeyAuth {
    keys: HashMap<String, ApiKeyInfo>,
}

impl ApiKeyAuth {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load keys from the [`API_KEYS_ENV`] environment variable
    /// (comma-separated). Empty entries are skipped.
    pub fn from_env() -> Self {
        let mut auth = Self::new();
        if let Ok(raw) = std::env::var(API_KEYS_ENV) {
            for (i, key) in raw.split(',').enumerate() {
                let key = key.trim();
                if key.is_empty() {
                    continue;
                }
                auth = auth.with_key(key, format!("key-{i}"));
            }
        }
        auth
    }

    /// Register a key with a human-readable name.
    pub fn with_key(mut self, key: impl Into<String>, name: impl Into<String>) -> Self {
        self.keys.insert(
            key.into(),
            ApiKeyInfo {
                name: name.into(),
                created_at: chrono::Utc::now(),
            },
        );
        self
    }

    /// Whether authentication is enforced (at least one key registered).
    pub fn is_enabled(&self) -> bool {
        !self.keys.is_empty()
    }

    /// Validate a key, returning its info if it is registered.
    pub fn validate(&self, key: &str) -> Option<&ApiKeyInfo> {
        self.keys.get(key)
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

/// Extract an API key from request headers.
///
/// Supports `Authorization: Bearer <key>` and `X-Api-Key: <key>`.
pub fn extract_api_key(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        let value = value.trim();
        if let Some(key) = value
            .strip_prefix("Bearer ")
            .or_else(|| value.strip_prefix("bearer "))
        {
            let key = key.trim();
            if !key.is_empty() {
                return Some(key.to_string());
            }
        }
    }
    headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Identity attached to a request after it passes authentication.
///
/// Holds the API key name (or `anonymous` when auth is disabled). Used as
/// the rate-limiting key.
#[derive(Debug, Clone)]
pub struct AuthPrincipal(pub String);

/// Authentication middleware.
///
/// Returns `401 Unauthorized` when keys are configured and the request does
/// not carry a valid key. On success, inserts an [`AuthPrincipal`] into the
/// request extensions for downstream middleware.
pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let principal = if state.auth.is_enabled() {
        match extract_api_key(&headers)
            .as_deref()
            .and_then(|key| state.auth.validate(key))
        {
            Some(info) => info.name.clone(),
            None => return Err(StatusCode::UNAUTHORIZED),
        }
    } else {
        "anonymous".to_string()
    };

    request.extensions_mut().insert(AuthPrincipal(principal));
    Ok(next.run(request).await)
}

/// Sliding-window rate limiter keyed by client identity.
#[derive(Debug)]
pub struct RateLimiter {
    requests: HashMap<String, Vec<Instant>>,
    limit: usize,
    window: Duration,
}

impl RateLimiter {
    pub fn new(limit: usize, window: Duration) -> Self {
        Self {
            requests: HashMap::new(),
            limit: limit.max(1),
            window,
        }
    }

    pub fn limit(&self) -> usize {
        self.limit
    }

    pub fn window(&self) -> Duration {
        self.window
    }

    /// Record a request for `key` at `now` and report whether it is allowed.
    ///
    /// Requests older than the window are evicted; if fewer than `limit`
    /// requests remain, the new one is accepted.
    pub fn check(&mut self, key: &str, now: Instant) -> bool {
        let window = self.window;
        let entry = self.requests.entry(key.to_string()).or_default();
        entry.retain(|t| now.duration_since(*t) < window);
        if entry.len() < self.limit {
            entry.push(now);
            true
        } else {
            false
        }
    }
}

/// Rate-limiting middleware.
///
/// Should run after [`auth_middleware`] so an [`AuthPrincipal`] is present.
/// Falls back to the client IP, then `"anonymous"`, when no principal is
/// attached. Returns `429 Too Many Requests` when the window is exhausted.
pub async fn rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let key = request
        .extensions()
        .get::<AuthPrincipal>()
        .map(|p| p.0.clone())
        .or_else(|| {
            request
                .extensions()
                .get::<ConnectInfo<SocketAddr>>()
                .map(|c| c.0.ip().to_string())
        })
        .unwrap_or_else(|| "anonymous".to_string());

    let allowed = state
        .rate_limiter
        .lock()
        .map(|mut limiter| limiter.check(&key, Instant::now()))
        .unwrap_or(true);

    if !allowed {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers_with(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (k, v) in pairs {
            map.insert(
                header::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                header::HeaderValue::from_str(v).unwrap(),
            );
        }
        map
    }

    #[test]
    fn extracts_bearer_token() {
        let h = headers_with(&[("authorization", "Bearer secret-key")]);
        assert_eq!(extract_api_key(&h).as_deref(), Some("secret-key"));
    }

    #[test]
    fn extracts_x_api_key_header() {
        let h = headers_with(&[("x-api-key", "secret-key")]);
        assert_eq!(extract_api_key(&h).as_deref(), Some("secret-key"));
    }

    #[test]
    fn bearer_takes_precedence_over_x_api_key() {
        let h = headers_with(&[
            ("authorization", "Bearer first"),
            ("x-api-key", "second"),
        ]);
        assert_eq!(extract_api_key(&h).as_deref(), Some("first"));
    }

    #[test]
    fn no_key_returns_none() {
        let h = headers_with(&[]);
        assert_eq!(extract_api_key(&h), None);

        let h = headers_with(&[("authorization", "Basic abc")]);
        assert_eq!(extract_api_key(&h), None);

        let h = headers_with(&[("x-api-key", "   ")]);
        assert_eq!(extract_api_key(&h), None);
    }

    #[test]
    fn api_key_auth_validates_registered_keys() {
        let auth = ApiKeyAuth::new().with_key("k1", "alice");
        assert!(auth.is_enabled());
        assert_eq!(auth.len(), 1);
        assert_eq!(auth.validate("k1").unwrap().name, "alice");
        assert!(auth.validate("k2").is_none());
    }

    #[test]
    fn empty_registry_disables_auth() {
        let auth = ApiKeyAuth::new();
        assert!(!auth.is_enabled());
        assert!(auth.is_empty());
        assert!(auth.validate("anything").is_none());
    }

    #[test]
    fn rate_limiter_allows_up_to_limit_per_window() {
        let mut limiter = RateLimiter::new(3, Duration::from_secs(60));
        let start = Instant::now();

        for i in 0..3 {
            assert!(
                limiter.check("client", start + Duration::from_millis(i)),
                "request {i} should be allowed"
            );
        }
        // Fourth request inside the window is rejected.
        assert!(!limiter.check("client", start + Duration::from_millis(10)));

        // Other clients have their own budget.
        assert!(limiter.check("other", start));

        // After the window elapses the client can make requests again.
        assert!(limiter.check("client", start + Duration::from_secs(61)));
    }

    #[test]
    fn rate_limiter_minimum_limit_is_one() {
        let limiter = RateLimiter::new(0, Duration::from_secs(60));
        assert_eq!(limiter.limit(), 1);
    }
}
