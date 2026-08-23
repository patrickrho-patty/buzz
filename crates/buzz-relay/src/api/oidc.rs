//! Keycloak OIDC authentication for workforce SSO.
//!
//! Grants Griddle (Buzz relay) login via the company Keycloak `internal`
//! realm. Employees sign in through the system browser (PKCE Authorization
//! Code flow); on success the relay:
//!
//! 1. validates the Keycloak `id_token` against the realm JWKS,
//! 2. looks up the employee's Nostr pubkey (minting one on first login,
//!    encrypted with a deployment master key and stored back into Keycloak
//!    as a user attribute),
//! 3. ensures the pubkey is a relay member (auto-admission),
//! 4. returns the *plaintext* Nostr private key to the desktop client so it
//!    can sign messages locally. The private key transits the wire exactly
//!    once, over TLS, inside the login response.
//!
//! Deprovisioning runs on a 30s sync loop: disabled or deleted Keycloak
//! users are removed from `relay_members`, and their live WebSockets are
//! closed on the next auth check.
//!
//! Routes:
//! - `GET  /auth/oidc/start`    — returns the authorization URL + PKCE verifier
//!                                state is held by the desktop client (not us).
//! - `POST /auth/oidc/complete` — { code, code_verifier, redirect_uri }
//!                                → exchanges code at Keycloak, validates,
//!                                mints/looks-up key, admits, returns keypair.

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::Json};
use serde::Deserialize;
use serde_json::Value;

use crate::state::AppState;

use super::{api_error, internal_error};

// ─────────────────────────────────────────────────────────────────────────────
// Config accessors
// ─────────────────────────────────────────────────────────────────────────────

fn issuer(state: &AppState) -> String {
    state.config.oidc.issuer.clone()
}

fn auth_url(state: &AppState) -> String {
    format!("{}/protocol/openid-connect/auth", issuer(state))
}

fn token_url(state: &AppState) -> String {
    format!("{}/protocol/openid-connect/token", issuer(state))
}

// ─────────────────────────────────────────────────────────────────────────────
// GET /auth/oidc/start
// ─────────────────────────────────────────────────────────────────────────────

/// Returns everything the desktop needs to open the system browser:
/// the authorization URL (with PKCE challenge baked in) and the verifier
/// the client must send back on completion.
pub async fn start(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let cfg = &state.config.oidc;
    if !cfg.enabled {
        return Err(api_error(StatusCode::NOT_FOUND, "OIDC is not configured"));
    }

    // PKCE: verifier 43-128 chars from url-safe alphabet, challenge = S256.
    let verifier = crate::oidc::generate_code_verifier();
    let challenge = crate::oidc::code_challenge_s256(&verifier);

    let state_param = crate::oidc::generate_state();
    let url = format!(
        "{}?client_id={}&response_type=code&scope=openid%20email%20profile&redirect_uri={}&state={}&code_challenge={}&code_challenge_method=S256",
        auth_url(&state),
        urlencoding::encode(&cfg.desktop_client_id),
        urlencoding::encode(&cfg.redirect_uri),
        state_param,
        challenge,
    );

    Ok(Json(serde_json::json!({
        "authorization_url": url,
        "state": state_param,
        "code_verifier": verifier,
    })))
}

// ─────────────────────────────────────────────────────────────────────────────
// POST /auth/oidc/complete
// ─────────────────────────────────────────────────────────────────────────────

/// Completion payload sent by the desktop client after the browser redirect.
#[derive(Deserialize)]
pub struct CompleteRequest {
    /// Authorization code returned by Keycloak to the redirect URI.
    pub code: String,
    /// PKCE verifier the client generated for this flow (returned by `/start`).
    pub code_verifier: String,
    /// Override for the registered redirect URI (dev deep links).
    pub redirect_uri: Option<String>,
}

/// Exchanges the OIDC authorization code, validates the user against Keycloak,
/// mints or recovers their Nostr keypair, and ensures relay membership.
/// Returns the plaintext private key to the caller (desktop stores it in the
/// OS keychain afterwards).
pub async fn complete(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CompleteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let cfg = state.config.oidc.clone();
    if !cfg.enabled {
        return Err(api_error(StatusCode::NOT_FOUND, "OIDC is not configured"));
    }

    let redirect_uri = body
        .redirect_uri
        .unwrap_or_else(|| cfg.redirect_uri.clone());

    // 1. Exchange the authorization code for tokens at Keycloak.
    let tokens = crate::oidc::exchange_code(
        &token_url(&state),
        &cfg.desktop_client_id,
        &body.code,
        &body.code_verifier,
        &redirect_uri,
    )
    .await
    .map_err(|e| {
        api_error(
            StatusCode::UNAUTHORIZED,
            &format!("code exchange failed: {e}"),
        )
    })?;

    let id_token = tokens
        .get("id_token")
        .and_then(Value::as_str)
        .ok_or_else(|| api_error(StatusCode::UNAUTHORIZED, "no id_token in Keycloak response"))?;

    // 2. Validate the id_token against the realm JWKS.
    let claims = crate::oidc::validate_id_token(&state, id_token)
        .await
        .map_err(|e| api_error(StatusCode::UNAUTHORIZED, &format!("id_token rejected: {e}")))?;

    let sub = claims.sub.clone();
    // Email resolution: prefer the id_token claim, then the userinfo endpoint
    // (Keycloak omits email from id_tokens when the client lacks the email
    // scope mapping — userinfo always carries it), then sub as a last resort.
    let email = match claims.email.clone() {
        Some(e) if e.contains('@') => e,
        _ => crate::oidc::fetch_userinfo_email(&state, tokens.get("access_token").and_then(|v| v.as_str()))
            .await
            .unwrap_or_else(|| sub.clone()),
    };
    let enabled = claims.enabled.unwrap_or(true);
    if !enabled {
        return Err(api_error(StatusCode::FORBIDDEN, "account is disabled"));
    }

    // 3. Mint or recover the Nostr keypair for this Keycloak `sub`.
    let keys = crate::oidc::keypair_for_user(&state, &sub, &email)
        .await
        .map_err(|e| {
            tracing::error!(user = %email, "OIDC key mint/lookup failed: {e}");
            internal_error("key mint failed")
        })?;

    // 4. Auto-admit: ensure the pubkey is a relay member of this deployment's
    //    community. Idempotent.
    let host = crate::tenant::relay_url_authority(&state.config.relay_url);
    let tenant = crate::tenant::bind_community(&state.db, &host)
        .await
        .map_err(|_| api_error(StatusCode::NOT_FOUND, "relay: community not configured"))?;

    let added = state
        .db
        .add_relay_member(tenant.community(), &keys.pubkey_hex, "member", Some("oidc"))
        .await
        .map_err(|e| internal_error(&format!("member admission failed: {e}")))?;
    if added {
        tracing::info!(user = %email, "OIDC auto-admitted member");
    }

    // 5. Hand the plaintext private key to the client (one-time, over TLS).
    //    `relay_url` tells the client which workspace to join — SSO users skip
    //    the generic community picker entirely.
    Ok(Json(serde_json::json!({
        "pubkey": keys.pubkey_hex,
        "private_key": keys.privkey_hex,
        "email": email,
        "name": claims.name,
        "relay_url": state.config.relay_url,
    })))
}

// ─────────────────────────────────────────────────────────────────────────────
// GET /auth/oidc/whoami — NIP-98 authed email lookup for the caller's pubkey
// ─────────────────────────────────────────────────────────────────────────────

/// Returns the workspace email bound to the caller's Nostr pubkey (via the
/// Keycloak `nostr_pubkey` user attribute). Lets the desktop resolve its SSO
/// username on any machine / after restarts without a session stash.
pub async fn whoami(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let cfg = state.config.oidc.clone();
    if !cfg.enabled {
        return Err(api_error(StatusCode::NOT_FOUND, "OIDC is not configured"));
    }

    // NIP-98 auth on this exact URL.
    let url = format!("{}/auth/oidc/whoami", state.config.relay_url.trim_end_matches('/'));
    let body: &[u8] = &[];
    let (_tenant, pubkey) = super::invites::authenticate(&state, &headers, &url, body)
        .await
        .map_err(|e| e)?;
    let pubkey_hex = pubkey.to_hex();

    // Reverse lookup across Keycloak users: find the one whose nostr_pubkey matches.
    let kc = crate::oidc::KeycloakAdmin::from_state(&state);
    let users = kc
        .list_users()
        .await
        .map_err(|e| internal_error(&format!("keycloak lookup: {e}")))?;

    let email = users
        .iter()
        .find(|u| u.enabled && u.nostr_pubkey() == Some(pubkey_hex.as_str()))
        .and_then(|u| u.email.clone())
        .unwrap_or_default();

    Ok(Json(serde_json::json!({ "email": email })))
}
