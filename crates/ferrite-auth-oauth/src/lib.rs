//! Ferrite OAuth — minimal plug-and-play OAuth2 flow for Google, GitHub, Discord.
//!
//! This crate implements the common Nest-style `@nestjs/passport` pattern:
//!
//! * Configured entirely by env variables (one triplet per provider:
//!   `<PROVIDER>_OAUTH_CLIENT_ID`, `<PROVIDER>_OAUTH_CLIENT_SECRET`,
//!   `<PROVIDER>_OAUTH_REDIRECT_URI`).
//! * Anti-CSRF state signed with HMAC-SHA256 (secret = `JWT_SECRET` or
//!   `OAUTH_STATE_SECRET` or a dev default).
//! * Controller routes mounted at `/oauth/:provider/authorize` and
//!   `/oauth/:provider/callback`.
//! * `callback` returns an [`OAuthUser`] you can persist; use the optional
//!   helper [`OAuthService::sign_jwt`] to mint a Ferrite `ferrite-auth-jwt`
//!   compatible token (HS256 with same secret as `JWT_SECRET`).
//!
//! # Env setup example
//!
//! ```text
//! GOOGLE_OAUTH_CLIENT_ID=...
//! GOOGLE_OAUTH_CLIENT_SECRET=...
//! GOOGLE_OAUTH_REDIRECT_URI=http://localhost:3000/oauth/google/callback
//! JWT_SECRET=your_hmac_secret
//! ```
//!
//! # Example
//!
//! ```ignore
//! use ferrite_framework::module;
//! use ferrite_auth_oauth::OAuthModule;
//!
//! #[module(imports = [OAuthModule])]
//! pub struct AppModule;
//! ```

use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use ferrite_auth_jwt::{JwtClaims, JwtError, JwtService};
use ferrite_macros::{controller, impl_controller, inject, injectable, module};
use fr_config::ConfigService;
use hmac::{Hmac, Mac};
use rand::RngCore;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use thiserror::Error;
use url::Url;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Built-in OAuth providers shipped by Ferrite. Add new ones via environment
/// variables by implementing the same pattern if you need others (e.g.
/// Facebook, Microsoft) — the [`OAuthService`] methods are generic enough.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OAuthProvider {
    Google,
    Github,
    Discord,
}

impl std::fmt::Display for OAuthProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OAuthProvider::Google => f.write_str("google"),
            OAuthProvider::Github => f.write_str("github"),
            OAuthProvider::Discord => f.write_str("discord"),
        }
    }
}

impl std::str::FromStr for OAuthProvider {
    type Err = OAuthError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "google" => Ok(OAuthProvider::Google),
            "github" => Ok(OAuthProvider::Github),
            "discord" => Ok(OAuthProvider::Discord),
            other => Err(OAuthError::UnknownProvider(other.into())),
        }
    }
}

/// Provider-specific URLs and default scope lists.
struct ProviderProfile {
    auth_url: &'static str,
    token_url: &'static str,
    userinfo_url: &'static str,
    scopes: &'static [&'static str],
}

impl OAuthProvider {
    fn profile(self) -> ProviderProfile {
        match self {
            OAuthProvider::Google => ProviderProfile {
                auth_url: "https://accounts.google.com/o/oauth2/v2/auth",
                token_url: "https://oauth2.googleapis.com/token",
                userinfo_url: "https://www.googleapis.com/oauth2/v3/userinfo",
                scopes: &["openid", "email", "profile"],
            },
            OAuthProvider::Github => ProviderProfile {
                auth_url: "https://github.com/login/oauth/authorize",
                token_url: "https://github.com/login/oauth/access_token",
                userinfo_url: "https://api.github.com/user",
                scopes: &["read:user", "user:email"],
            },
            OAuthProvider::Discord => ProviderProfile {
                auth_url: "https://discord.com/oauth2/authorize",
                token_url: "https://discord.com/api/oauth2/token",
                userinfo_url: "https://discord.com/api/oauth2/@me",
                scopes: &["identify", "email"],
            },
        }
    }

    fn env_prefix(self) -> &'static str {
        match self {
            OAuthProvider::Google => "GOOGLE",
            OAuthProvider::Github => "GITHUB",
            OAuthProvider::Discord => "DISCORD",
        }
    }
}

/// Resolved OAuth provider config (client id, secret, redirect URI).
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

/// User information returned after a complete OAuth callback flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthUser {
    pub id: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub provider: String,
}

/// Any error surfaced by [`OAuthService`] or controllers.
#[derive(Debug, Error)]
pub enum OAuthError {
    #[error("unknown OAuth provider: `{0}`")]
    UnknownProvider(String),
    #[error("missing env `{prefix}_OAUTH_CLIENT_ID` or `{prefix}_OAUTH_CLIENT_SECRET` or `{prefix}_OAUTH_REDIRECT_URI`")]
    MissingConfig { prefix: String },
    #[error("failed to sign state: {0}")]
    StateSign(String),
    #[error("invalid or tampered oauth state")]
    InvalidState,
    #[error("provider HTTP request failed: {0}")]
    Http(String),
    #[error("provider returned a non-OK status: {0}")]
    ProviderStatus(String),
    #[error("could not decode provider response: {0}")]
    Decode(String),
    #[error("authorization error: missing code or error={0}")]
    Authorization(String),
    #[error("jwt: {0}")]
    Jwt(#[from] JwtError),
}

// ---------------------------------------------------------------------------
// HMAC-signed state (no session storage needed)
// ---------------------------------------------------------------------------

type HmacSha256 = Hmac<Sha256>;

fn state_secret(cfg: &ConfigService) -> Vec<u8> {
    cfg.get("JWT_SECRET")
        .or_else(|| cfg.get("OAUTH_STATE_SECRET"))
        .unwrap_or_else(|| "ferrite-oauth-dev-secret-change-me".into())
        .into_bytes()
}

fn sign_state(secret: &[u8], nonce: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC init ok");
    mac.update(nonce);
    let sig = mac.finalize().into_bytes();
    format!("{}.{}", B64.encode(nonce), B64.encode(sig))
}

fn generate_state(secret: &[u8]) -> String {
    let mut nonce = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut nonce);
    sign_state(secret, &nonce)
}

fn verify_state(secret: &[u8], state: &str) -> bool {
    let Some((nonce_b64, sig_b64)) = state.split_once('.') else {
        return false;
    };
    let Ok(nonce) = B64.decode(nonce_b64) else {
        return false;
    };
    let Ok(sig) = B64.decode(sig_b64) else {
        return false;
    };
    let mut mac = match HmacSha256::new_from_slice(secret) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(&nonce);
    mac.verify_slice(&sig).is_ok()
}

// ---------------------------------------------------------------------------
// Provider responses
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct GoogleUserInfo {
    sub: String,
    email: Option<String>,
    name: Option<String>,
    picture: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubUserInfo {
    id: i64,
    login: Option<String>,
    name: Option<String>,
    avatar_url: Option<String>,
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubEmail {
    email: String,
    primary: bool,
    verified: bool,
}

#[derive(Debug, Deserialize)]
struct DiscordUserInfo {
    id: String,
    username: Option<String>,
    email: Option<String>,
    avatar: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    #[serde(flatten)]
    _extra: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// Injectable provider coordinating the complete OAuth2 flow.
#[injectable]
pub struct OAuthService {
    config: ConfigService,
    jwt: Option<Arc<JwtService>>,
    http: HttpClient,
    _anchor: u8,
}

impl OAuthService {
    #[inject]
    pub fn new(config: ConfigService, jwt: Option<Arc<JwtService>>) -> Self {
        Self {
            config,
            jwt,
            http: Arc::new(
                HttpClient::builder()
                    .user_agent(concat!("ferrite-auth-oauth/", env!("CARGO_PKG_VERSION")))
                    .build()
                    .unwrap_or_default(),
            ),
            _anchor: Arc::new(0),
        }
    }

    /// Pull provider credentials from the env.
    pub fn provider_config(&self, provider: OAuthProvider) -> Result<ProviderConfig, OAuthError> {
        let prefix = provider.env_prefix();
        let id_var = format!("{prefix}_OAUTH_CLIENT_ID");
        let sec_var = format!("{prefix}_OAUTH_CLIENT_SECRET");
        let uri_var = format!("{prefix}_OAUTH_REDIRECT_URI");
        let cid = self.config.get(&id_var);
        let csec = self.config.get(&sec_var);
        let ruri = self.config.get(&uri_var);
        match (cid, csec, ruri) {
            (Some(client_id), Some(client_secret), Some(redirect_uri)) => Ok(ProviderConfig {
                client_id,
                client_secret,
                redirect_uri,
            }),
            _ => Err(OAuthError::MissingConfig {
                prefix: prefix.into(),
            }),
        }
    }

    /// Build a full authorization URL with signed state.
    pub fn authorize_url(&self, provider: OAuthProvider) -> Result<String, OAuthError> {
        let cfg = self.provider_config(provider)?;
        let profile = provider.profile();
        let state = generate_state(&state_secret(&self.config));
        let mut url =
            Url::parse(profile.auth_url).map_err(|e| OAuthError::StateSign(e.to_string()))?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("response_type", "code");
            pairs.append_pair("client_id", &cfg.client_id);
            pairs.append_pair("redirect_uri", &cfg.redirect_uri);
            pairs.append_pair("state", &state);
            pairs.append_pair("scope", &profile.scopes.join(" "));
            if matches!(provider, OAuthProvider::Google) {
                pairs.append_pair("access_type", "offline");
                pairs.append_pair("prompt", "select_account");
            }
        }
        Ok(url.into())
    }

    /// Verify signed state, exchange `code` for `access_token`, fetch the
    /// user's profile, and finally wrap it into an [`OAuthUser`].
    pub async fn callback(
        &self,
        provider: OAuthProvider,
        code: Option<String>,
        state: Option<String>,
        error: Option<String>,
    ) -> Result<OAuthUser, OAuthError> {
        if let Some(err) = error {
            return Err(OAuthError::Authorization(err));
        }
        let state = state.ok_or(OAuthError::InvalidState)?;
        if !verify_state(&state_secret(&self.config), &state) {
            return Err(OAuthError::InvalidState);
        }
        let code = code.ok_or_else(|| OAuthError::Authorization("missing code".into()))?;
        let access_token = self.exchange_code(provider, &code).await?;
        self.fetch_user(provider, &access_token).await
    }

    async fn exchange_code(
        &self,
        provider: OAuthProvider,
        code: &str,
    ) -> Result<String, OAuthError> {
        let cfg = self.provider_config(provider)?;
        let profile = provider.profile();
        let form = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", cfg.redirect_uri.as_str()),
            ("client_id", cfg.client_id.as_str()),
            ("client_secret", cfg.client_secret.as_str()),
        ];
        // GitHub is the odd one: it returns form-encoded by default, but we
        // want JSON. Set Accept header for it.
        let req = self
            .http
            .post(profile.token_url)
            .header("Accept", "application/json");
        let res = req
            .form(&form)
            .send()
            .await
            .map_err(|e| OAuthError::Http(e.to_string()))?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(OAuthError::ProviderStatus(format!("{status}: {body}")));
        }
        let token: TokenResponse = res
            .json()
            .await
            .map_err(|e| OAuthError::Decode(e.to_string()))?;
        token
            .access_token
            .ok_or_else(|| OAuthError::Decode("missing access_token in response".into()))
    }

    async fn fetch_user(
        &self,
        provider: OAuthProvider,
        access_token: &str,
    ) -> Result<OAuthUser, OAuthError> {
        let profile = provider.profile();
        let bearer = format!("Bearer {access_token}");
        let req = self
            .http
            .get(profile.userinfo_url)
            .header("Authorization", &bearer)
            .header("Accept", "application/json");
        let res = req
            .send()
            .await
            .map_err(|e| OAuthError::Http(e.to_string()))?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(OAuthError::ProviderStatus(format!("{status}: {body}")));
        }
        match provider {
            OAuthProvider::Google => {
                let info: GoogleUserInfo = res
                    .json()
                    .await
                    .map_err(|e| OAuthError::Decode(e.to_string()))?;
                Ok(OAuthUser {
                    id: info.sub,
                    email: info.email,
                    name: info.name,
                    avatar_url: info.picture,
                    provider: "google".into(),
                })
            }
            OAuthProvider::Github => {
                let info: GithubUserInfo = res
                    .json()
                    .await
                    .map_err(|e| OAuthError::Decode(e.to_string()))?;
                // GitHub often leaves `email` null even with `user:email` scope;
                // fetch it from the /user/emails endpoint.
                let primary_email = if info.email.is_none() {
                    let emails: Vec<GithubEmail> = self
                        .http
                        .get("https://api.github.com/user/emails")
                        .header("Authorization", &bearer)
                        .header("Accept", "application/json")
                        .send()
                        .await
                        .map_err(|e| OAuthError::Http(e.to_string()))?
                        .json()
                        .await
                        .unwrap_or_default();
                    emails
                        .into_iter()
                        .filter(|e| e.primary && e.verified)
                        .map(|e| e.email)
                        .next()
                } else {
                    None
                };
                Ok(OAuthUser {
                    id: info.id.to_string(),
                    email: info.email.or(primary_email),
                    name: info.name.or(info.login),
                    avatar_url: info.avatar_url,
                    provider: "github".into(),
                })
            }
            OAuthProvider::Discord => {
                let info: DiscordUserInfo = res
                    .json()
                    .await
                    .map_err(|e| OAuthError::Decode(e.to_string()))?;
                let avatar = match (&info.id, &info.avatar) {
                    (id, Some(av)) => {
                        Some(format!("https://cdn.discordapp.com/avatars/{id}/{av}.png"))
                    }
                    _ => None,
                };
                Ok(OAuthUser {
                    id: info.id,
                    email: info.email,
                    name: info.username,
                    avatar_url: avatar,
                    provider: "discord".into(),
                })
            }
        }
    }

    /// Mint a HS256 JWT for `user`. Falls back to an inline encoder using
    /// `JWT_SECRET` if the optional [`JwtService`] was not available via DI.
    pub fn sign_jwt(&self, user: &OAuthUser) -> Result<String, OAuthError> {
        let sub = user.id.parse::<i64>().unwrap_or_default();
        let email = user
            .email
            .clone()
            .unwrap_or_else(|| format!("{id}@{provider}", id = user.id, provider = user.provider));
        if let Some(jwt) = self.jwt.as_ref() {
            Ok(jwt.sign(sub, email)?)
        } else {
            // Fallback encoder: reuse `jsonwebtoken` crate directly so the
            // tokens are compatible with `ferrite-auth-jwt::JwtService::verify`.
            let ttl = 86400usize;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as usize)
                .unwrap_or(0);
            let exp = now.checked_add(ttl).unwrap_or(now);
            let claims = JwtClaims {
                sub,
                email,
                exp,
                iss: self.config.get("JWT_ISSUER"),
            };
            let secret = self
                .config
                .get_or("JWT_SECRET", "ferrite-dev-secret-change-me");
            if secret.is_empty() {
                return Err(JwtError::MissingSecret.into());
            }
            use jsonwebtoken::{encode, EncodingKey, Header};
            let ek = EncodingKey::from_secret(secret.as_bytes());
            encode(&Header::default(), &claims, &ek)
                .map_err(|e| JwtError::InvalidToken(e.to_string()).into())
        }
    }
}

// ---------------------------------------------------------------------------
// Controller
// ---------------------------------------------------------------------------

/// Mounted routes:
///
/// * `GET /oauth/:provider/authorize` — redirect the browser to the provider
/// * `GET /oauth/:provider/callback?code=&state=` — exchange + redirect
#[controller("/oauth")]
pub struct OAuthController {
    service: OAuthService,
    config: ConfigService,
}

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AuthorizeQuery {
    pub redirect: Option<String>,
}

#[impl_controller]
impl OAuthController {
    #[inject]
    pub fn new(service: OAuthService, config: ConfigService) -> Self {
        Self { service, config }
    }

    #[ferrite_macros::get("/:provider/authorize")]
    pub async fn authorize(
        &self,
        ferrite_framework::extract::Path(provider): ferrite_framework::extract::Path<String>,
    ) -> Result<axum::response::Redirect, ferrite_framework::HttpError> {
        let provider: OAuthProvider = provider
            .parse()
            .map_err(|e: OAuthError| ferrite_framework::HttpError::bad_request(e.to_string()))?;
        let url = self
            .service
            .authorize_url(provider)
            .map_err(|e| ferrite_framework::HttpError::internal(e.to_string()))?;
        Ok(axum::response::Redirect::to(&url))
    }

    #[ferrite_macros::get("/:provider/callback")]
    pub async fn callback(
        &self,
        ferrite_framework::extract::Path(provider): ferrite_framework::extract::Path<String>,
        ferrite_framework::extract::Query(q): ferrite_framework::extract::Query<CallbackQuery>,
    ) -> Result<axum::response::Html<String>, ferrite_framework::HttpError> {
        let provider: OAuthProvider = provider
            .parse()
            .map_err(|e: OAuthError| ferrite_framework::HttpError::bad_request(e.to_string()))?;
        match self
            .service
            .callback(provider, q.code, q.state, q.error)
            .await
        {
            Ok(user) => {
                let jwt = self
                    .service
                    .sign_jwt(&user)
                    .map_err(|e| ferrite_framework::HttpError::internal(e.to_string()))?;
                let success = self.config.get("OAUTH_CLIENT_SUCCESS_URL");
                if let Some(redirect) = success {
                    let with_token = if redirect.contains('?') {
                        format!("{redirect}&token={jwt}")
                    } else {
                        format!("{redirect}?token={jwt}")
                    };
                    let html = format!(
                        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Signing in…</title>\n\
                        <meta http-equiv=\"refresh\" content=\"0; url={}\"></head>\n\
                        <body>Redirecting… <a href=\"{}\">click here</a></body></html>",
                        with_token, with_token
                    );
                    return Ok(axum::response::Html(html));
                }
                let json = serde_json::json!({ "user": user, "token": jwt });
                let html = format!(
                    "<!doctype html><html><head><meta charset=\"utf-8\"><title>OAuth success</title>\n\
                    <style>body{{font-family:system-ui,Segoe UI,Arial;background:#f7f7f9;margin:0;padding:40px}}\n\
                    pre{{background:#111;color:#9cffb0;padding:16px;border-radius:8px;overflow:auto}}</style>\n\
                    </head><body><h2>Ferrite OAuth — success</h2>\n\
                    <pre>{}</pre></body></html>",
                    serde_json::to_string_pretty(&json).unwrap_or_default()
                );
                Ok(axum::response::Html(html))
            }
            Err(err) => {
                let html = format!(
                    "<!doctype html><html><head><meta charset=\"utf-8\"><title>OAuth error</title>\n\
                    <style>body{{font-family:system-ui,Segoe UI,Arial;background:#fff1f1;color:#7a1d1d;margin:0;padding:40px}}</style>\n\
                    </head><body><h2>OAuth failed</h2><p>{err}</p></body></html>"
                );
                Ok(axum::response::Html(html))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

#[module(
    controllers = [OAuthController],
    providers = [OAuthService],
)]
pub struct OAuthModule;

// ---------------------------------------------------------------------------
// Tests (state only — no network)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn with_test_env(env: &str, f: impl FnOnce(&Path)) {
        let dir: PathBuf = std::env::temp_dir().join(format!(
            "ferrite-oauth-test-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".env"), env).unwrap();
        f(&dir);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn state_roundtrip_and_tamper_check() {
        let sec = b"test-secret";
        let state = generate_state(sec);
        assert!(verify_state(sec, &state));
        // Tamper the signature suffix: flip the very last byte of the signature
        // so the HMAC cannot match the expected digest.
        let (prefix, suffix) = state.split_once('.').expect("state format is random.sig");
        let bad_suffix = {
            let bytes = suffix.to_string();
            let mut chars: Vec<char> = bytes.chars().collect();
            if let Some(last) = chars.last_mut() {
                *last = if *last == 'A' { 'B' } else { 'A' };
            }
            chars.into_iter().collect::<String>()
        };
        let bad = format!("{}.{}", prefix, bad_suffix);
        assert_ne!(bad, state);
        assert!(!verify_state(sec, &bad));
        assert!(!verify_state(sec, "not.a.state"));
    }

    #[test]
    fn provider_fromstr_unknown_errors() {
        assert!(matches!(
            "google".parse::<OAuthProvider>(),
            Ok(OAuthProvider::Google)
        ));
        assert!(matches!(
            "facebook".parse::<OAuthProvider>(),
            Err(OAuthError::UnknownProvider(_))
        ));
    }

    #[test]
    fn missing_config_surfaces_prefix() {
        with_test_env("JWT_SECRET=dev-secret\n", |dir| {
            let cfg = ConfigService::load_from(dir);
            let svc = OAuthService::new(Arc::new(cfg), Arc::new(None));
            let err = svc.provider_config(OAuthProvider::Google).unwrap_err();
            assert!(matches!(err, OAuthError::MissingConfig { prefix } if prefix == "GOOGLE"));
        });
    }

    #[test]
    fn authorize_url_contains_state_and_scope() {
        with_test_env(
            "GOOGLE_OAUTH_CLIENT_ID=cid\n\
             GOOGLE_OAUTH_CLIENT_SECRET=csec\n\
             GOOGLE_OAUTH_REDIRECT_URI=http://localhost/oauth/google/callback\n\
             JWT_SECRET=dev\n",
            |dir| {
                let cfg = ConfigService::load_from(dir);
                let svc = OAuthService::new(Arc::new(cfg), Arc::new(None));
                let url = svc.authorize_url(OAuthProvider::Google).unwrap();
                assert!(url.contains("accounts.google.com"));
                assert!(url.contains("client_id=cid"));
                assert!(url.contains("scope=openid+email+profile"));
                assert!(url.contains("state="));
            },
        );
    }
}
