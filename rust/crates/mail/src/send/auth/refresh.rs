//! The **OAuth2 refresh-token exchange** — turn a long-lived refresh token into a short-lived access
//! token at the provider's token endpoint.
//!
//! One `POST` of `application/x-www-form-urlencoded`, per RFC 6749 §6, which is what Google, Microsoft
//! and every other provider implements. The operator seals `client_secret` + `refresh_token` in
//! `secrets/`; the config holds only the endpoint URL, the client id, and the secret PATHS.
//!
//! Failure classification matters here as much as in SMTP: a `5xx` or a network error at the token
//! endpoint is **transient** (retry later, the mail is still deliverable), while `invalid_grant` — a
//! revoked or wrong refresh token — is **permanent**: it will never succeed until an operator redoes
//! the consent ceremony, and retrying it five times just puts the failure further away from the
//! operator who can fix it. The error text is the provider's `error`/`error_description` fields, never
//! the body verbatim, so a token cannot ride out in a log line.

use serde::Deserialize;

use crate::error::{MailError, MailResult};

/// What is needed to mint an access token. Every field is a *name or a value the caller already
/// resolved* — this struct never reads a secret store itself.
#[derive(Clone)]
pub struct RefreshRequest {
    /// The provider's token endpoint (`https://oauth2.googleapis.com/token`).
    pub token_endpoint: String,
    /// The OAuth2 client id (not a secret).
    pub client_id: String,
    /// The OAuth2 client secret — resolved from `secrets/` by the caller.
    pub client_secret: String,
    /// The long-lived refresh token — resolved from `secrets/` by the caller.
    pub refresh_token: String,
}

impl RefreshRequest {
    /// The cache key for this request: endpoint + client + a *fingerprint* of the refresh token, so
    /// rotating the sealed refresh token invalidates the cached access token instead of silently
    /// reusing one minted from the old grant. The fingerprint is a **SHA-256 prefix**, never the token
    /// — cache keys end up in maps that get logged.
    ///
    /// It is a real digest because the cheap version wasn't safe: a length-plus-tail fingerprint
    /// collided for two same-length tokens with a shared suffix (`…nValue`), which the rotation test
    /// caught — the rotated grant silently reused the old grant's access token. A rotation that keeps
    /// working *by accident* is the worst failure mode here, since the operator believes the old token
    /// is out of service.
    pub fn cache_key(&self) -> String {
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(self.refresh_token.as_bytes());
        let fingerprint: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
        format!("{}|{}|{fingerprint}", self.token_endpoint, self.client_id)
    }
}

/// The subset of the token response we read.
#[derive(Debug, Deserialize)]
pub struct TokenEndpointResponse {
    pub access_token: String,
    /// Lifetime in seconds. Absent ⇒ treated as the conservative default (see
    /// [`super::xoauth2::DEFAULT_TOKEN_TTL_SECS`]).
    #[serde(default)]
    pub expires_in: Option<u64>,
}

/// Exchange `refresh_token` for an access token. See the module note for the failure classification.
pub async fn refresh_access_token(
    client: &reqwest::Client,
    req: &RefreshRequest,
) -> MailResult<TokenEndpointResponse> {
    let form = [
        ("grant_type", "refresh_token"),
        ("client_id", req.client_id.as_str()),
        ("client_secret", req.client_secret.as_str()),
        ("refresh_token", req.refresh_token.as_str()),
    ];
    let response = client
        .post(&req.token_endpoint)
        .form(&form)
        .send()
        .await
        // A network failure reaching the token endpoint says nothing about the mail — retry.
        .map_err(|e| MailError::Transient(format!("mail: token endpoint unreachable: {e}")))?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if status.is_success() {
        let parsed: TokenEndpointResponse = serde_json::from_str(&body).map_err(|e| {
            // A 200 that is not a token response is a wrong endpoint — an operator must fix it.
            MailError::Permanent(format!(
                "mail: token endpoint returned no access_token ({e})"
            ))
        })?;
        if parsed.access_token.trim().is_empty() {
            return Err(MailError::Permanent(
                "mail: token endpoint returned an empty access_token".into(),
            ));
        }
        return Ok(parsed);
    }

    // Read the provider's machine-readable `error` field only — never echo the body, which on some
    // providers includes the request parameters we just sent.
    let code = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| {
            v.get("error")
                .and_then(|e| e.as_str().map(str::to_string))
                .or_else(|| {
                    v.get("error")
                        .and_then(|e| e.get("code"))
                        .map(|c| c.to_string())
                })
        })
        .unwrap_or_else(|| status.as_str().to_string());

    // `invalid_grant`/`invalid_client` = the sealed grant is wrong or revoked: no retry will fix it.
    let permanent = matches!(
        code.as_str(),
        "invalid_grant" | "invalid_client" | "unauthorized_client" | "invalid_scope"
    ) || (status.is_client_error() && status.as_u16() != 429);
    let message = format!("mail: token refresh failed ({}): {code}", status.as_u16());
    if permanent {
        Err(MailError::Permanent(message))
    } else {
        Err(MailError::Transient(message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_fingerprints_the_token_without_carrying_it() {
        let req = RefreshRequest {
            token_endpoint: "https://oauth2.googleapis.com/token".into(),
            client_id: "cid".into(),
            client_secret: "csecret".into(),
            refresh_token: "1//0gRefreshTokenValue".into(),
        };
        let key = req.cache_key();
        assert!(!key.contains("1//0gRefreshTokenValue"), "{key}");
        // Rotating the sealed token must change the key (else the old access token is reused). The
        // rotated value here shares its LENGTH and its last six characters with the original — the
        // collision the previous length-and-tail fingerprint let through.
        let rotated = RefreshRequest {
            refresh_token: "1//0gRotatedTokenValue".into(),
            ..req.clone()
        };
        assert_eq!(
            req.refresh_token.len(),
            rotated.refresh_token.len(),
            "the fixture must keep the collision shape it guards"
        );
        assert_ne!(key, rotated.cache_key());
    }
}
