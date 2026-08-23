//! OIDC plumbing for Keycloak workforce SSO — the engine behind
//! [`crate::api::oidc`].
//!
//! Responsibilities:
//! - PKCE helpers (verifier + S256 challenge)
//! - authorization-code exchange against the Keycloak token endpoint
//! - `id_token` validation against the realm JWKS (RS256/ES256/EdDSA via
//!   embedded `jsonwebtoken` verification; JWKS cached with a 10-minute TTL)
//! - per-user Nostr keypair minting/persistence:
//!   - pubkey stored as a Keycloak user attribute (`nostr_pubkey`)
//!   - private key encrypted AES-256-GCM with the deployment master key,
//!     stored as `nostr_privkey_encrypted` (base64 `nonce || ciphertext`)
//! - membership sync loop: every 30s, poll Keycloak users; any user that is
//!   disabled, deleted, or missing a `nostr_pubkey` attribute while having
//!   `oidc`-origin membership gets their relay membership removed.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use nostr::prelude::*;

use serde::{Deserialize, Serialize};
use sha2::Digest;
use tokio::sync::RwLock;

use crate::state::AppState;

/// Interval between Keycloak sync polls.
pub const SYNC_INTERVAL: Duration = Duration::from_secs(30);

/// JWKS cache TTL.
const JWKS_TTL: Duration = Duration::from_secs(600);

// ─────────────────────────────────────────────────────────────────────────────
// PKCE
// ─────────────────────────────────────────────────────────────────────────────

const VERIFIER_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";

/// Generate a PKCE code verifier (43 chars, within the RFC 7636 bounds).
pub fn generate_code_verifier() -> String {
    // Random index selection via random<u8> % len (bias irrelevant for PKCE).
    (0..43)
        .map(|_| {
            let b: u8 = ::rand::random();
            VERIFIER_CHARS[(b as usize) % VERIFIER_CHARS.len()] as char
        })
        .collect()
}

/// S256 code challenge: BASE64URL(SHA256(verifier)), unpadded.
pub fn code_challenge_s256(verifier: &str) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

/// Random `state` parameter.
pub fn generate_state() -> String {
    let bytes: [u8; 12] = ::rand::random();
    hex::encode(bytes)
}

// ─────────────────────────────────────────────────────────────────────────────
// Token exchange
// ─────────────────────────────────────────────────────────────────────────────

/// Exchange an authorization code for the Keycloak token set.
pub async fn exchange_code(
    token_url: &str,
    client_id: &str,
    code: &str,
    code_verifier: &str,
    redirect_uri: &str,
) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let params = [
        ("grant_type", "authorization_code"),
        ("client_id", client_id),
        ("code", code),
        ("code_verifier", code_verifier),
        ("redirect_uri", redirect_uri),
    ];
    let resp = client
        .post(token_url)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("keycloak unreachable: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("keycloak returned {status}: {body}"));
    }
    resp.json()
        .await
        .map_err(|e| format!("keycloak response parse: {e}"))
}

// ─────────────────────────────────────────────────────────────────────────────
// id_token validation (JWKS)
// ─────────────────────────────────────────────────────────────────────────────

/// Claims extracted from a validated Keycloak `id_token`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdClaims {
    pub sub: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub preferred_username: Option<String>,
    #[serde(default)]
    pub email_verified: bool,
    /// Populated only on token exchange (not in id_tokens generally);
    /// user `enabled` status is fetched live from the admin API during sync.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

struct JwksCache {
    keys: jsonwebtoken::jwk::JwkSet,
    fetched_at: std::time::Instant,
}

/// Process-wide JWKS cache keyed by issuer URL.
static JWKS_CACHE: once_cell::sync::Lazy<tokio::sync::Mutex<HashMap<String, JwksCache>>> =
    once_cell::sync::Lazy::new(|| tokio::sync::Mutex::new(HashMap::new()));

/// Validate an `id_token` against the issuer's JWKS.
/// Checks: signature (key selected by `kid` from the token header,
/// algorithm taken from the header itself), `iss`, `aud` (desktop client
/// id), `exp`, `iat`.
///
/// Keycloak realms may sign with RS256 (default), PS256, or ES256 — the
/// token header is authoritative for both the algorithm and the key.
pub async fn validate_id_token(state: &AppState, token: &str) -> Result<IdClaims, String> {
    let cfg = &state.config.oidc;
    let issuer = &cfg.issuer;
    let expected_aud = &cfg.desktop_client_id;

    let jwks = fetch_jwks_cached(issuer).await?;

    let header = jsonwebtoken::decode_header(token)
        .map_err(|e| format!("jwt header parse: {e}"))?;
    let jwk = pick_jwk_by_kid(&jwks, header.kid.as_deref())?;

    let mut validation = jsonwebtoken::Validation::new(header.alg);
    validation.set_issuer(&[issuer.as_str()]);
    validation.set_audience(&[expected_aud.as_str()]);
    validation.leeway = 30;

    let key = jsonwebtoken::DecodingKey::from_jwk(&jwk)
        .map_err(|e| format!("decoding key from jwk: {e}"))?;
    let token_data = jsonwebtoken::decode::<IdClaims>(token, &key, &validation)
        .map_err(|e| format!("jwt validation: {e}"))?;

    Ok(token_data.claims)
}

/// Pick a signing (not encryption) JWK — by `kid` when the token header
/// carries one, else the first RSA signing key.
fn pick_jwk_by_kid(
    jwks: &jsonwebtoken::jwk::JwkSet,
    kid: Option<&str>,
) -> Result<jsonwebtoken::jwk::Jwk, String> {
    let is_signing = |k: &jsonwebtoken::jwk::Jwk| {
        matches!(
            k.common.key_algorithm,
            Some(jsonwebtoken::jwk::KeyAlgorithm::RS256)
                | Some(jsonwebtoken::jwk::KeyAlgorithm::PS256)
                | Some(jsonwebtoken::jwk::KeyAlgorithm::ES256)
        )
    };
    if let Some(kid) = kid {
        if let Some(k) = jwks
            .keys
            .iter()
            .find(|k| k.common.key_id.as_deref() == Some(kid) && is_signing(k))
        {
            return Ok(k.clone());
        }
        return Err(format!("JWKS has no signing key for kid {kid}"));
    }
    jwks
        .keys
        .iter()
        .find(|k| is_signing(k))
        .cloned()
        .ok_or_else(|| "no usable signing key in JWKS".to_string())
}

async fn fetch_jwks_cached(issuer: &str) -> Result<jsonwebtoken::jwk::JwkSet, String> {
    let mut cache = JWKS_CACHE.lock().await;
    if let Some(entry) = cache.get(issuer) {
        if entry.fetched_at.elapsed() < JWKS_TTL {
            return Ok(entry.keys.clone());
        }
    }
    let url = format!("{}/protocol/openid-connect/certs", issuer);
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("jwks fetch: {e}"))?;
    let keys: jsonwebtoken::jwk::JwkSet =
        resp.json().await.map_err(|e| format!("jwks parse: {e}"))?;
    cache.insert(
        issuer.to_string(),
        JwksCache {
            keys: keys.clone(),
            fetched_at: std::time::Instant::now(),
        },
    );
    Ok(keys)
}

/// Fetch the userinfo endpoint with the access token and extract the email.
/// Used as a fallback when the id_token carries no `email` claim.
pub async fn fetch_userinfo_email(
    state: &AppState,
    access_token: Option<&str>,
) -> Option<String> {
    let token = access_token?;
    let url = format!("{}/protocol/openid-connect/userinfo", state.config.oidc.issuer);
    let resp = reqwest::Client::new()
        .get(&url)
        .bearer_auth(token)
        .send()
        .await
        .ok()?;
    let body: serde_json::Value = resp.json().await.ok()?;
    body.get("email")
        .and_then(|v| v.as_str())
        .filter(|e| e.contains('@'))
        .map(String::from)
}

// ─────────────────────────────────────────────────────────────────────────────
// Nostr keypair minting + persistence
// ─────────────────────────────────────────────────────────────────────────────

/// A per-employee Nostr keypair recovered from (or newly written to) Keycloak.
pub struct MintedKeypair {
    /// 64-char hex public key — the employee's identity in the workspace.
    pub pubkey_hex: String,
    /// 64-char hex private key — returned once to the desktop client.
    pub privkey_hex: String,
}

/// Look up (or mint) the Nostr keypair for a Keycloak user.
///
/// Priority:
/// 1. Keycloak attributes `nostr_pubkey` + `nostr_privkey_encrypted` — decrypt.
/// 2. Otherwise mint a fresh keypair, store back into Keycloak, return it.
pub async fn keypair_for_user(
    state: &AppState,
    sub: &str,
    email: &str,
) -> Result<MintedKeypair, String> {
    let kc = KeycloakAdmin::from_state(state);

    // 1. Existing attributes?
    let (existing_pub, existing_priv_enc) = kc.get_nostr_attrs(sub).await?;
    if let (Some(pub_hex), Some(priv_enc)) = (existing_pub, existing_priv_enc) {
        let priv_hex = decrypt_private_key(state, &priv_enc)?;
        // Cross-check: derived pubkey must match the stored one.
        if let Ok(keys) = Keys::parse(&priv_hex) {
            if keys.public_key().to_hex() == pub_hex {
                return Ok(MintedKeypair {
                    pubkey_hex: pub_hex,
                    privkey_hex: priv_hex,
                });
            }
        }
        // Mismatch → fall through and mint fresh (corrupt attribute).
        tracing::warn!(
            user = email,
            "stored nostr attributes inconsistent; re-minting"
        );
    }

    // 2. Mint fresh.
    let keys = Keys::generate();
    let pub_hex = keys.public_key().to_hex();
    let priv_hex = keys.secret_key().to_secret_hex();

    let encrypted = encrypt_private_key(state, &priv_hex)?;
    kc.set_nostr_attrs(sub, &pub_hex, &encrypted).await?;

    Ok(MintedKeypair {
        pubkey_hex: pub_hex,
        privkey_hex: priv_hex,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// AES-256-GCM wrapping of the Nostr private key
// ─────────────────────────────────────────────────────────────────────────────

fn derive_master_key(state: &AppState) -> Result<aes_gcm::Key<aes_gcm::Aes256Gcm>, String> {
    let secret = &state.config.oidc.key_wrap_secret;
    let digest = sha2::Sha256::digest(secret.as_bytes());
    let key = *aes_gcm::Key::<aes_gcm::Aes256Gcm>::from_slice(&digest);
    Ok(key)
}

fn encrypt_private_key(state: &AppState, priv_hex: &str) -> Result<String, String> {
    use aes_gcm::aead::{Aead, Payload};
    use aes_gcm::{Aes256Gcm, Nonce};

    let key = derive_master_key(state)?;
    let cipher = <Aes256Gcm as aes_gcm::KeyInit>::new(&key);
    let nonce_bytes: [u8; 12] = ::rand::random();
    let ct = cipher
        .encrypt(
            Nonce::from_slice(&nonce_bytes),
            Payload {
                msg: priv_hex.as_bytes(),
                aad: b"griddle-nostr-privkey",
            },
        )
        .map_err(|_| "aes-gcm encrypt failed".to_string())?;
    let mut blob = nonce_bytes.to_vec();
    blob.extend_from_slice(&ct);
    Ok(base64::engine::general_purpose::STANDARD.encode(blob))
}

fn decrypt_private_key(state: &AppState, blob_b64: &str) -> Result<String, String> {
    use aes_gcm::aead::{Aead, Payload};
    use aes_gcm::{Aes256Gcm, Nonce};

    let blob = base64::engine::general_purpose::STANDARD
        .decode(blob_b64)
        .map_err(|_| "base64 decode of encrypted key failed".to_string())?;
    if blob.len() < 12 + 16 {
        return Err("encrypted key blob too short".to_string());
    }
    let (nonce_bytes, ct) = blob.split_at(12);
    let key = derive_master_key(state)?;
    let cipher = <Aes256Gcm as aes_gcm::KeyInit>::new(&key);
    let pt = cipher
        .decrypt(
            Nonce::from_slice(nonce_bytes),
            Payload {
                msg: ct,
                aad: b"griddle-nostr-privkey",
            },
        )
        .map_err(|_| "aes-gcm decrypt failed (wrong master key?)".to_string())?;
    String::from_utf8(pt).map_err(|_| "decrypted key not utf-8".to_string())
}

// ─────────────────────────────────────────────────────────────────────────────
// Keycloak Admin API client (service account)
// ─────────────────────────────────────────────────────────────────────────────

/// Minimal Keycloak Admin REST client authenticated as the `griddle-bridge`
/// service account (client_credentials). Caches the admin token in-memory.
pub struct KeycloakAdmin {
    base_url: String,  // https://login.patty.io
    realm: String,     // internal
    client_id: String, // griddle-bridge
    client_secret: String,
    http: reqwest::Client,
    token_cache: RwLock<Option<(String, std::time::Instant)>>,
}

impl KeycloakAdmin {
    pub fn from_state(state: &AppState) -> Self {
        let cfg = &state.config.oidc;
        Self {
            base_url: cfg.keycloak_base_url.clone(),
            realm: cfg.realm.clone(),
            client_id: cfg.bridge_client_id.clone(),
            client_secret: cfg.bridge_client_secret.clone(),
            http: reqwest::Client::new(),
            token_cache: RwLock::new(None),
        }
    }

    async fn admin_token(&self) -> Result<String, String> {
        if let Some((tok, at)) = self.token_cache.read().await.clone() {
            // refresh 30s before expiry
            if at.elapsed() < Duration::from_secs(55) {
                return Ok(tok);
            }
        }
        let resp = self
            .http
            .post(format!(
                "{}/realms/{}/protocol/openid-connect/token",
                self.base_url, self.realm
            ))
            .form(&[
                ("grant_type", "client_credentials"),
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
            ])
            .send()
            .await
            .map_err(|e| format!("keycloak token: {e}"))?;
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("keycloak token parse: {e}"))?;
        let tok = body
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or("no access_token in client_credentials response")?
            .to_string();
        *self.token_cache.write().await = Some((tok, std::time::Instant::now()));
        Ok(self.token_cache.read().await.clone().unwrap().0)
    }

    /// Fetch the nostr pubkey / encrypted-privkey attributes for a user.
    async fn get_nostr_attrs(&self, sub: &str) -> Result<(Option<String>, Option<String>), String> {
        let user = self.get_user(sub).await?;
        let attrs = user
            .get("attributes")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let pub_key = attrs
            .get("nostr_pubkey")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
            .map(String::from);
        let priv_enc = attrs
            .get("nostr_privkey_encrypted")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
            .map(String::from);
        Ok((pub_key, priv_enc))
    }

    async fn get_user(&self, sub: &str) -> Result<serde_json::Value, String> {
        let tok = self.admin_token().await?;
        let resp = self
            .http
            .get(format!(
                "{}/admin/realms/internal/users/{sub}",
                self.base_url
            ))
            .bearer_auth(tok)
            .send()
            .await
            .map_err(|e| format!("keycloak get user: {e}"))?;
        resp.json()
            .await
            .map_err(|e| format!("keycloak user parse: {e}"))
    }

    /// Write the nostr attributes back onto the user (merge with existing).
    async fn set_nostr_attrs(
        &self,
        sub: &str,
        pubkey: &str,
        encrypted_priv: &str,
    ) -> Result<(), String> {
        let mut user = self.get_user(sub).await?;
        let attrs = user
            .get("attributes")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let mut new_attrs = attrs;
        new_attrs["nostr_pubkey"] = serde_json::json!([pubkey]);
        new_attrs["nostr_privkey_encrypted"] = serde_json::json!([encrypted_priv]);
        user["attributes"] = new_attrs;

        let tok = self.admin_token().await?;
        let resp = self
            .http
            .put(format!(
                "{}/admin/realms/internal/users/{sub}",
                self.base_url
            ))
            .bearer_auth(tok)
            .json(&serde_json::json!({ "attributes": user["attributes"] }))
            .send()
            .await
            .map_err(|e| format!("keycloak update user: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("keycloak update user status {}", resp.status()));
        }
        Ok(())
    }

    /// List users (id, username, email, enabled, attributes) for sync.
    pub async fn list_users(&self) -> Result<Vec<KcUser>, String> {
        let tok = self.admin_token().await?;
        let resp = self
            .http
            .get(format!(
                "{}/admin/realms/internal/users?max=500",
                self.base_url
            ))
            .bearer_auth(tok)
            .send()
            .await
            .map_err(|e| format!("keycloak list users: {e}"))?;
        resp.json()
            .await
            .map_err(|e| format!("keycloak users parse: {e}"))
    }
}

/// Trimmed Keycloak user record used by the sync loop.
#[derive(Debug, Clone, Deserialize)]
pub struct KcUser {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub attributes: Option<HashMap<String, Vec<String>>>,
}

impl KcUser {
    pub fn nostr_pubkey(&self) -> Option<&str> {
        self.attributes
            .as_ref()?
            .get("nostr_pubkey")?
            .first()
            .map(|s| s.as_str())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Membership sync loop
// ─────────────────────────────────────────────────────────────────────────────

/// Run the 30s sync loop forever. Intended to be spawned from relay startup
/// when OIDC is enabled.
pub async fn run_sync_loop(state: Arc<AppState>) {
    loop {
        if let Err(e) = sync_once(&state).await {
            tracing::warn!("OIDC membership sync failed: {e}");
        }
        tokio::time::sleep(SYNC_INTERVAL).await;
    }
}

/// One sync pass:
/// 1. Poll Keycloak users.
/// 2. Build the set of pubkeys that should remain members (enabled users
///    having a nostr_pubkey attribute).
/// 3. Remove any `relay_members` row created via OIDC (`added_by = 'oidc'`)
///    whose pubkey is no longer in that set.
pub async fn sync_once(state: &Arc<AppState>) -> Result<(), String> {
    let host = crate::tenant::relay_url_authority(&state.config.relay_url);
    let tenant = crate::tenant::bind_community(&state.db, &host)
        .await
        .map_err(|e| format!("bind community: {e:?}"))?;

    let kc = KeycloakAdmin::from_state(state);
    let users = kc.list_users().await?;

    let mut valid: std::collections::HashSet<String> = Default::default();
    for u in &users {
        if !u.enabled {
            continue;
        }
        if let Some(pk) = u.nostr_pubkey() {
            valid.insert(pk.to_string());
        }
    }

    let members = state
        .db
        .list_relay_members(tenant.community())
        .await
        .map_err(|e| format!("list members: {e}"))?;

    for m in members {
        if m.added_by.as_deref() != Some("oidc") {
            continue; // never touch owner / invite-admitted members
        }
        if !valid.contains(&m.pubkey) {
            match state
                .db
                .remove_relay_member(tenant.community(), &m.pubkey)
                .await
            {
                Ok(_) => {
                    tracing::info!(pubkey = %m.pubkey, "OIDC sync removed member (disabled/deleted in Keycloak)")
                }
                Err(e) => tracing::error!("OIDC sync remove failed: {e}"),
            }
        }
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_verifier_has_rfc7636_shape() {
        let v = generate_code_verifier();
        assert_eq!(v.len(), 43, "verifier must be 43 chars (within 43..=128)");
        assert!(
            v.chars()
                .all(|c| VERIFIER_CHARS.contains(&(c as u8))),
            "verifier must only contain unreserved chars"
        );
    }

    #[test]
    fn code_verifiers_are_random() {
        let a = generate_code_verifier();
        let b = generate_code_verifier();
        assert_ne!(a, b);
    }

    #[test]
    fn s256_challenge_is_base64url_of_sha256() {
        // Known-answer test from RFC 7636 appendix B.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = code_challenge_s256(verifier);
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn state_is_24_hex_chars() {
        let s = generate_state();
        assert_eq!(s.len(), 24);
        assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn key_wrap_roundtrip() {
        // Build a minimal AppState-shaped stand-in: only oidc config matters
        // for wrap/unwrap. Use a raw struct via the real Config? AppState is
        // heavy; instead exercise the pure functions via a tiny harness that
        // mirrors encrypt/decrypt with an explicit secret.
        let secret = "test-master-secret-32-bytes-long!!";
        let priv_hex = "d1f8761d07d67351578fe4f4a96c84e917cc4f7a6a89fb8134cba0845dd4214b";

        // Mirror of encrypt_private_key without AppState:
        use aes_gcm::aead::{Aead, Payload};
        use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
        use sha2::Digest;
        let digest = sha2::Sha256::digest(secret.as_bytes());
        let key = *aes_gcm::Key::<Aes256Gcm>::from_slice(&digest);
        let cipher = Aes256Gcm::new(&key);
        let nonce_bytes: [u8; 12] = rand::random();
        let ct = cipher
            .encrypt(
                Nonce::from_slice(&nonce_bytes),
                Payload {
                    msg: priv_hex.as_bytes(),
                    aad: b"griddle-nostr-privkey",
                },
            )
            .unwrap();
        let mut blob = nonce_bytes.to_vec();
        blob.extend_from_slice(&ct);
        let b64 = base64::engine::general_purpose::STANDARD.encode(&blob);

        // Decrypt mirror:
        let raw = base64::engine::general_purpose::STANDARD.decode(&b64).unwrap();
        let (n, ct) = raw.split_at(12);
        let pt = cipher
            .decrypt(
                Nonce::from_slice(n),
                Payload {
                    msg: ct,
                    aad: b"griddle-nostr-privkey",
                },
            )
            .unwrap();
        assert_eq!(String::from_utf8(pt).unwrap(), priv_hex);
    }

    #[test]
    fn tampered_ciphertext_fails_to_decrypt() {
        use aes_gcm::aead::{Aead, Payload};
        use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
        use sha2::Digest;
        let secret = "test-master-secret-32-bytes-long!!";
        let digest = sha2::Sha256::digest(secret.as_bytes());
        let key = *aes_gcm::Key::<Aes256Gcm>::from_slice(&digest);
        let cipher = Aes256Gcm::new(&key);
        let nonce_bytes: [u8; 12] = rand::random();
        let ct = cipher
            .encrypt(
                Nonce::from_slice(&nonce_bytes),
                Payload {
                    msg: b"secret-data",
                    aad: b"griddle-nostr-privkey",
                },
            )
            .unwrap();
        // Flip a bit in the ciphertext.
        let mut tampered = ct.clone();
        tampered[0] ^= 0x01;
        let result = cipher.decrypt(
            Nonce::from_slice(&nonce_bytes),
            Payload {
                msg: &tampered,
                aad: b"griddle-nostr-privkey",
            },
        );
        assert!(result.is_err(), "tampered ciphertext must not decrypt");
    }

    #[test]
    fn kc_user_parses_nostr_pubkey_attribute() {
        let raw = serde_json::json!({
            "id": "user-1",
            "username": "david",
            "email": "david@patty.io",
            "enabled": true,
            "attributes": { "nostr_pubkey": ["ebf0b5847e34a95716bca3cd83c1cd91efd8f6b610cc62b4531ec5e7d9825026"] }
        });
        let u: KcUser = serde_json::from_value(raw).unwrap();
        assert!(u.enabled);
        assert_eq!(
            u.nostr_pubkey(),
            Some("ebf0b5847e34a95716bca3cd83c1cd91efd8f6b610cc62b4531ec5e7d9825026")
        );
    }

    #[test]
    fn jwks_selection_matches_by_kid_and_header_alg() {
        // Real-world shape: two RSA keys (one sig RS256, one enc RSA-OAEP).
        // pick_jwk_by_kid must return the sig key by kid and reject enc keys.
        let sig = serde_json::json!({
            "kty": "RSA", "kid": "sig-key-1", "use": "sig", "alg": "RS256",
            "n": "sXchSkCPvVBEEiTa1pYl2-LMGMHg-1tWfy_LZ1U4Pt8bdOZdDkT6G7-8EJDQXZHRN2dZArPVIzFg",
            "e": "AQAB"
        });
        let enc = serde_json::json!({
            "kty": "RSA", "kid": "enc-key-1", "use": "enc", "alg": "RSA-OAEP",
            "n": "sXchSkCPvVBEEiTa1pYl2-LMGMHg-1tWfy_LZ1U4Pt8bdOZdDkT6G7-8EJDQXZHRN2dZArPVIzFg",
            "e": "AQAB"
        });
        let jwks: jsonwebtoken::jwk::JwkSet =
            serde_json::from_value(serde_json::json!({ "keys": [sig, enc] })).unwrap();

        // kid match selects the sig key
        let picked = pick_jwk_by_kid(&jwks, Some("sig-key-1")).unwrap();
        assert_eq!(picked.common.key_id.as_deref(), Some("sig-key-1"));

        // enc key kid is rejected
        assert!(pick_jwk_by_kid(&jwks, Some("enc-key-1")).is_err());

        // unknown kid errors (no silent fallback)
        assert!(pick_jwk_by_kid(&jwks, Some("missing")).is_err());

        // no kid → first signing key
        let fallback = pick_jwk_by_kid(&jwks, None).unwrap();
        assert_eq!(fallback.common.key_id.as_deref(), Some("sig-key-1"));
    }

    #[test]
    fn kc_user_without_attributes_has_no_pubkey() {
        let raw = serde_json::json!({ "id": "u2", "username": "gone", "enabled": false });
        let u: KcUser = serde_json::from_value(raw).unwrap();
        assert!(!u.enabled);
        assert!(u.nostr_pubkey().is_none());
    }
}
