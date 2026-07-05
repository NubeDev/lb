//! `webhook.create` — mint a workspace-walled webhook (webhooks scope). Gated by
//! `mcp:webhook.manage:call`, workspace-first. Resolves the auth mode, persists the record, and
//! provisions the credential per mode:
//!
//! - `bearer` — calls `apikey_create` to mint a real `apikey:{ws}:{keyid}` row scoped to this hook
//!   (label `webhook:{id}`, role empty, one narrowed cap `mcp:ingest.write:call`). The one-time
//!   bearer string `lbk_{ws}.{keyid}.{secret}` is returned. The webhook row carries
//!   `bearer_key_id` so revoke/rotate reach the linked apikey. **No-widening:** the admin creator
//!   must already hold `mcp:ingest.write:call` — `apikey_create` runs that check itself, so the
//!   refusal surfaces as `Widen` here.
//!
//! - `signature` — generates a fresh shared secret, stores it in `lb-secrets` at `webhook/{id}`
//!   under the creator's principal as a `Workspace`-visibility secret (so the host can
//!   mediate-read it on verify), and stores `secret_ref` + the admin-picked `hmac_header` on the
//!   webhook row. The one-time shared secret is returned. The creator must hold
//!   `secret:webhook/*:write` (the cap `lb_secrets::set_with` re-checks) and `mcp:ingest.write:
//!   call` (the no-widening guard, applied symmetrically across modes).
//!
//! Returns [`CreatedWebhook`] — the URL is always safe to surface; `secret` is the ONLY egress of
//! the raw credential (bearer: the `lbk_` string; signature: the shared secret).

use lb_apikey::generate_id;
use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_secrets::{set_with, Visibility};
use lb_store::{write, Store};

use super::error::WebhookError;
use super::model::{AuthMode, WebhookRecord, DEFAULT_HMAC_HEADER, TABLE};
use super::secret::generate_shared_secret;

/// The cap the inbound principal resolves to (and therefore the cap the creator must hold for the
/// no-widening guard to pass). Always `mcp:ingest.write:call` (webhooks-scope open question 3,
/// leaning always-narrowed — applied as the floor in v1).
pub const INGEST_CAP: &str = "mcp:ingest.write:call";

/// The create-reply envelope. `secret` is the one-time raw credential — `lbk_{ws}.{keyid}.{secret}`
/// in `bearer` mode, the shared secret in `signature` mode. Shown once; never recoverable.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CreatedWebhook {
    /// The webhook id (also the URL's last path segment).
    pub id: String,
    /// The stable inbound URL path (`/hooks/{ws}/{id}`). The host/gateway resolves the public
    /// origin; the path is the durable, host-owned identity.
    pub url_path: String,
    /// The one-time raw credential. `bearer` mode: `lbk_{ws}.{keyid}.{secret}`; `signature` mode:
    /// the shared secret to use when signing the raw body.
    pub secret: String,
    /// The auth mode that was provisioned (so the wizard shows the right "how to call" panel).
    pub auth_mode: String,
    /// `signature` mode only: the header name the caller must send the signature in.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hmac_header: String,
}

/// The args for [`webhook_create`] (mirrors the MCP `webhook.create` input).
#[derive(Debug, Clone)]
pub struct CreateArgs<'a> {
    pub name: &'a str,
    pub auth_mode: AuthMode,
    /// `signature` mode: the header name the caller signs. Empty/None ⇒ `X-Signature`.
    pub hmac_header: Option<&'a str>,
}

/// Create a webhook in `ws` as `principal`, returning the one-time credential envelope.
///
/// **No-widening (load-bearing, both modes).** The hook's inbound principal resolves to
/// `mcp:ingest.write:call`; an admin who lacks it cannot mint a webhook that would resolve to a cap
/// they don't hold. We check it once up front (covering both modes symmetrically); `bearer` mode
/// additionally relies on `apikey_create`'s own effective-caps check (defense in depth).
#[allow(clippy::too_many_arguments)]
pub async fn webhook_create(
    store: &Store,
    principal: &Principal,
    ws: &str,
    pepper: &[u8],
    args: CreateArgs<'_>,
    now: u64,
) -> Result<CreatedWebhook, WebhookError> {
    authorize_tool(principal, ws, "webhook.manage").map_err(|_| WebhookError::Denied)?;
    if args.name.trim().is_empty() {
        return Err(WebhookError::BadInput("name is required".into()));
    }

    // The no-widening guard (both modes). The hook's principal will resolve to INGEST_CAP; the
    // creator must already hold it. `bearer` mode's `apikey_create` re-runs this inside (defense in
    // depth) — but running it here keeps `signature` mode honest too (it has no apikey path).
    if !crate::authz::holds_cap(principal, ws, INGEST_CAP) {
        return Err(WebhookError::Widen(INGEST_CAP.to_string()));
    }

    let id = format!("wh_{}", generate_id());
    let url_path = format!("/hooks/{ws}/{id}");
    let name = args.name.trim();

    match args.auth_mode {
        AuthMode::Bearer => {
            create_bearer(store, principal, ws, pepper, &id, &url_path, name, now).await
        }
        AuthMode::Signature => {
            create_signature(
                store,
                principal,
                ws,
                &id,
                &url_path,
                name,
                args.hmac_header,
                now,
            )
            .await
        }
    }
}

/// `bearer` mode: mint the linked apikey (one narrowed cap), then persist the webhook row with the
/// `bearer_key_id` cross-reference. The one-time `lbk_…` string is returned. The cache is unused
/// here (`apikey_create` does not bust on success — it only stores + grants); it is threaded for
/// symmetry with `revoke`/`rotate` (which DO bust).
async fn create_bearer(
    store: &Store,
    principal: &Principal,
    ws: &str,
    pepper: &[u8],
    id: &str,
    url_path: &str,
    name: &str,
    now: u64,
) -> Result<CreatedWebhook, WebhookError> {
    // Mint the linked apikey. `apikey_create` runs its own effective-caps no-widening check, so a
    // creator lacking INGEST_CAP is refused here too (mapped to `Widen`). Label ties the apikey to
    // this hook for an admin correlating the two tables. Kind `api`, no role, one narrowed cap.
    let label = format!("webhook:{id}");
    let bearer = crate::apikey_create(
        store,
        principal,
        ws,
        pepper,
        &label,
        "api",
        "",
        &[INGEST_CAP.to_string()],
        0,
        now,
    )
    .await
    .map_err(map_apikey_err)?;
    let key_id = parse_bearer_key_id(&bearer)?;

    let record = WebhookRecord::new(
        id,
        ws,
        name,
        AuthMode::Bearer,
        Some(key_id),
        None,
        String::new(),
        now,
    );
    persist(store, ws, id, &record).await?;

    Ok(CreatedWebhook {
        id: id.to_string(),
        url_path: url_path.to_string(),
        secret: bearer,
        auth_mode: AuthMode::Bearer.as_str().to_string(),
        hmac_header: String::new(),
    })
}

/// `signature` mode: generate the shared secret, store it in `lb-secrets` at `webhook/{id}`
/// (Workspace visibility, host-mediate-readable), then persist the webhook row.
async fn create_signature(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    url_path: &str,
    name: &str,
    hmac_header: Option<&str>,
    now: u64,
) -> Result<CreatedWebhook, WebhookError> {
    let shared = generate_shared_secret();
    let secret_ref = WebhookRecord::secret_path(id);
    let header = hmac_header
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_HMAC_HEADER)
        .to_string();
    // Workspace visibility so the host's verify path can mediate-read it via `secret::get_workspace`
    // under a synthetic webhook principal (which holds no `secret:*:get` cap). Private would deny
    // the host and break every inbound verify.
    set_with(
        store,
        principal,
        ws,
        &secret_ref,
        &shared,
        Visibility::Workspace,
    )
    .await
    .map_err(|_| WebhookError::Denied)?;

    let record = WebhookRecord::new(
        id,
        ws,
        name,
        AuthMode::Signature,
        None,
        Some(secret_ref),
        header.clone(),
        now,
    );
    persist(store, ws, id, &record).await?;

    Ok(CreatedWebhook {
        id: id.to_string(),
        url_path: url_path.to_string(),
        secret: shared,
        auth_mode: AuthMode::Signature.as_str().to_string(),
        hmac_header: header,
    })
}

/// Persist a webhook record at `webhook:{ws}:{id}`.
async fn persist(
    store: &Store,
    ws: &str,
    id: &str,
    record: &WebhookRecord,
) -> Result<(), WebhookError> {
    let value =
        serde_json::to_value(record).map_err(|e| lb_store::StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, id, &value).await?;
    Ok(())
}

/// Extract the apikey id from a bearer string `lbk_{ws}.{keyid}.{secret}` — three dot-separated
/// fields after `lbk_`. The webhook row stores it as `bearer_key_id`. Requires the full
/// three-field shape (the secret is what `apikey_authenticate` will verify later, but the shape
/// pins that there IS one — a two-field bearer is malformed).
fn parse_bearer_key_id(bearer: &str) -> Result<String, WebhookError> {
    let rest = bearer.strip_prefix("lbk_").ok_or_else(|| {
        WebhookError::Store(lb_store::StoreError::Decode(format!(
            "apikey_create returned a non-lbk_ bearer: {bearer}"
        )))
    })?;
    let mut parts = rest.split('.');
    let _ws = parts.next();
    let key_id = parts.next().ok_or_else(|| {
        WebhookError::Store(lb_store::StoreError::Decode(format!(
            "bearer has no keyid field: {bearer}"
        )))
    })?;
    // Require the secret field too — a two-field bearer is malformed (the secret is what the
    // caller will present; a record linked to a bearer with no secret field is broken).
    if parts.next().is_none() {
        return Err(WebhookError::Store(lb_store::StoreError::Decode(format!(
            "bearer has no secret field: {bearer}"
        ))));
    }
    Ok(key_id.to_string())
}

/// Map an `ApiKeyError` from the linked `apikey_create` onto the webhook error surface. A
/// no-widening refusal (the creator lacks `ingest.write`) is `Widen`; a generic deny is `Denied`;
/// the auth-path outcomes never arise on `create` (mapped to deny just in case).
fn map_apikey_err(e: crate::ApiKeyError) -> WebhookError {
    use crate::ApiKeyError;
    match e {
        ApiKeyError::Widen(c) => WebhookError::Widen(c),
        ApiKeyError::Denied => WebhookError::Denied,
        ApiKeyError::BadInput(m) => WebhookError::BadInput(m),
        ApiKeyError::NotFound => WebhookError::NotFound,
        ApiKeyError::Store(s) => WebhookError::Store(s),
        // Auth-path outcomes (Revoked/Expired/Invalid) don't arise on `create` — surface as deny.
        ApiKeyError::Revoked | ApiKeyError::Expired | ApiKeyError::Invalid => WebhookError::Denied,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bearer_key_id_splits_three_fields() {
        let k = parse_bearer_key_id("lbk_acme.k7f3a.sss").unwrap();
        assert_eq!(k, "k7f3a");
    }

    #[test]
    fn parse_bearer_key_id_rejects_non_lbk() {
        assert!(parse_bearer_key_id("xxx_acme.k.s").is_err());
    }

    #[test]
    fn parse_bearer_key_id_rejects_two_fields() {
        assert!(parse_bearer_key_id("lbk_acme.k7f3a").is_err());
    }
}
