use crate::config::AppConfig;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use reqwest::header::CACHE_CONTROL;
use serde::Deserialize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

const CF_ACCESS_JWT_HEADER: &str = "cf-access-jwt-assertion";

#[derive(Clone, Debug)]
pub struct AuthContext {
    pub email: String,
}

#[derive(Clone)]
pub struct AuthManager {
    enabled: bool,
    jwks_url: Option<String>,
    audience: Option<String>,
    issuer: Option<String>,
    dev_user_email: String,
    cache_ttl: Duration,
    client: reqwest::Client,
    cache: Arc<RwLock<JwksCache>>,
}

impl AuthManager {
    pub fn new(config: &AppConfig) -> anyhow::Result<Self> {
        let enabled = config.cf_access_enabled;
        let jwks_url = config.cf_access_jwks_url.clone();
        if enabled && jwks_url.is_none() {
            return Err(anyhow::anyhow!(
                "CF_ACCESS_ENABLED=true but no JWKS URL is configured"
            ));
        }
        if enabled && config.cf_access_audience.is_none() {
            tracing::warn!("CF_ACCESS_AUD not set; JWT audience will not be validated");
        }
        if enabled && config.cf_access_issuer.is_none() {
            tracing::warn!("CF_ACCESS_ISSUER not set; JWT issuer will not be validated");
        }
        if enabled {
            tracing::debug!(
                jwks_url = ?jwks_url,
                audience = ?config.cf_access_audience,
                issuer = ?config.cf_access_issuer,
                cache_ttl_secs = config.cf_access_jwks_cache_ttl_secs,
                "cloudflare access auth enabled"
            );
        } else {
            tracing::debug!("cloudflare access auth disabled");
        }

        Ok(Self {
            enabled,
            jwks_url,
            audience: config.cf_access_audience.clone(),
            issuer: config.cf_access_issuer.clone(),
            dev_user_email: config.cf_access_dev_user_email.clone(),
            cache_ttl: Duration::from_secs(config.cf_access_jwks_cache_ttl_secs),
            client: reqwest::Client::new(),
            cache: Arc::new(RwLock::new(JwksCache::default())),
        })
    }

    pub async fn authenticate(&self, headers: &HeaderMap) -> Result<AuthContext, AuthError> {
        if !self.enabled {
            return Ok(AuthContext {
                email: self.dev_user_email.clone(),
            });
        }

        let token = headers
            .get(CF_ACCESS_JWT_HEADER)
            .and_then(|value| value.to_str().ok())
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| AuthError::unauthorized("missing cf-access-jwt-assertion header"))?;

        let header = decode_header(token).map_err(|err| {
            tracing::debug!(?err, "invalid jwt header");
            AuthError::unauthorized("invalid jwt header")
        })?;
        if header.alg != Algorithm::RS256 {
            return Err(AuthError::unauthorized("unexpected jwt algorithm"));
        }
        let kid = header
            .kid
            .ok_or_else(|| AuthError::unauthorized("missing jwt kid"))?;

        let decoding_key = self.decoding_key(&kid).await?;
        let mut validation = Validation::new(Algorithm::RS256);
        if let Some(audience) = &self.audience {
            validation.set_audience(&[audience.as_str()]);
        }
        if let Some(issuer) = &self.issuer {
            validation.set_issuer(&[issuer.as_str()]);
        }

        let data =
            decode::<serde_json::Value>(token, &decoding_key, &validation).map_err(|err| {
                tracing::debug!(?err, "jwt validation failed");
                AuthError::unauthorized("invalid cf-access-jwt-assertion token")
            })?;

        let email = data
            .claims
            .get("email")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string());

        let email = email.ok_or_else(|| AuthError::unauthorized("missing user email"))?;

        Ok(AuthContext { email })
    }

    async fn decoding_key(&self, kid: &str) -> Result<DecodingKey, AuthError> {
        let jwks = self.get_jwks(false).await?;
        if let Some(jwk) = jwks.key(kid) {
            return jwk.to_decoding_key();
        }
        let jwks = self.get_jwks(true).await?;
        jwks.key(kid)
            .ok_or_else(|| AuthError::unauthorized("unknown jwt key id"))?
            .to_decoding_key()
    }

    async fn get_jwks(&self, force_refresh: bool) -> Result<Arc<Jwks>, AuthError> {
        if !force_refresh {
            if let Some(jwks) = self.cache.read().await.fresh() {
                return Ok(jwks);
            }
        }

        let stale = self.cache.read().await.jwks.clone();
        let result = self.fetch_jwks().await;

        match result {
            Ok((jwks, ttl)) => {
                let mut cache = self.cache.write().await;
                cache.jwks = Some(jwks.clone());
                cache.expires_at = Some(Instant::now() + ttl);
                Ok(jwks)
            }
            Err(error) => {
                if let Some(stale) = stale {
                    tracing::warn!(?error, "failed to refresh JWKS, using cached keys");
                    Ok(stale)
                } else {
                    Err(error)
                }
            }
        }
    }

    async fn fetch_jwks(&self) -> Result<(Arc<Jwks>, Duration), AuthError> {
        let url = self
            .jwks_url
            .as_ref()
            .ok_or_else(|| AuthError::unauthorized("jwks url not configured"))?;

        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|_| AuthError::unauthorized("failed to fetch jwks"))?;
        if !response.status().is_success() {
            return Err(AuthError::unauthorized("jwks fetch failed"));
        }
        let headers = response.headers().clone();
        let jwks = response
            .json::<Jwks>()
            .await
            .map_err(|_| AuthError::unauthorized("invalid jwks response"))?;

        let ttl = cache_ttl_from_headers(&headers, self.cache_ttl);
        Ok((Arc::new(jwks), ttl))
    }
}

#[derive(Debug)]
pub struct AuthError {
    status: StatusCode,
    message: String,
}

impl AuthError {
    fn unauthorized(message: &str) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.to_string(),
        }
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        (self.status, self.message).into_response()
    }
}

#[derive(Clone, Default)]
struct JwksCache {
    jwks: Option<Arc<Jwks>>,
    expires_at: Option<Instant>,
}

impl JwksCache {
    fn fresh(&self) -> Option<Arc<Jwks>> {
        let expires_at = self.expires_at?;
        if Instant::now() < expires_at {
            return self.jwks.clone();
        }
        None
    }
}

#[derive(Clone, Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

impl Jwks {
    fn key(&self, kid: &str) -> Option<&Jwk> {
        self.keys.iter().find(|key| key.kid == kid)
    }
}

#[derive(Clone, Deserialize)]
struct Jwk {
    kid: String,
    kty: String,
    n: String,
    e: String,
}

impl Jwk {
    fn to_decoding_key(&self) -> Result<DecodingKey, AuthError> {
        if self.kty != "RSA" {
            return Err(AuthError::unauthorized("unsupported jwk key type"));
        }
        DecodingKey::from_rsa_components(&self.n, &self.e)
            .map_err(|_| AuthError::unauthorized("invalid jwk key"))
    }
}

fn cache_ttl_from_headers(headers: &HeaderMap, default_ttl: Duration) -> Duration {
    let mut max_age: Option<u64> = None;
    if let Some(value) = headers
        .get(CACHE_CONTROL)
        .and_then(|value| value.to_str().ok())
    {
        for part in value.split(',') {
            let part = part.trim();
            if let Some(age) = part.strip_prefix("s-maxage=") {
                max_age = age.parse::<u64>().ok();
                continue;
            }
            if let Some(age) = part.strip_prefix("max-age=") {
                if max_age.is_none() {
                    max_age = age.parse::<u64>().ok();
                }
            }
        }
    }
    match max_age {
        Some(age) if age > 0 => Duration::from_secs(age),
        _ => default_ttl,
    }
}
