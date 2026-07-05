//! The `webhook` record + the credential-free views the admin verbs return (webhooks scope). One
//! row per `(ws, id)` at `webhook:{ws}:{id}` in the workspace's own namespace (the hard wall, §7).
//!
//! **Two auth modes** (admin-selected per hook, webhooks-scope "Goals"):
//! - `bearer` — the caller sends `Authorization: Bearer lbk_{ws}.{keyid}.{secret}`. The credential
//!   IS a real `apikey:{ws}:{keyid}` record (reused, not duplicated): `lb_apikey::apikey_create`
//!   mints it under subject `key:webhook:{id}` with a single narrowed cap `mcp:ingest.write:call`.
//!   The webhook row carries `bearer_key_id` so revoke/rotate reach it. Rotate = rotate the apikey;
//!   revoke = revoke the apikey (and the webhook tombstone).
//! - `signature` — the caller signs the raw body with a shared secret. The secret lives in
//!   `lb-secrets` at `webhook/{id}` (Workspace visibility, so the host can mediate-read it on
//!   verify); the webhook row carries `secret_ref` + the admin-picked `hmac_header` name. v1 ships
//!   the `hmac-sha256` scheme only (header value `sha256=<hex>`), the near-universal shape.
//!
//! Neither the raw secret nor the apikey hash is ever on this row (asserted in a test). The hash
//! discipline + one-time reveal are inherited from `§api-keys` (bearer) / `lb-secrets` (signature).
//! A revoked webhook is a `status = "__revoked__"` tombstone (sync-idempotent, like the apikey
//! tombstone). `kind_discrim` is the list-filter discriminant (every row carries `"webhook"`).
//!
//! The series a webhook emits to is `webhook:{ws}:{id}` — derived, not admin-chosen, so a hit can
//! never land on another series. The producer stamp is `webhook:{id}` (the un-spoofable identity
//! the route sets, mirroring how `ingest.write` overrides `producer` with `principal.sub()`).

use serde::{Deserialize, Serialize};

/// The store table webhook records live in, within a workspace namespace.
pub const TABLE: &str = "webhook";

/// The constant `kind_discrim` discriminant so [`webhook_list`](super::list) selects every row.
pub const KIND_DISCRIM: &str = "webhook";

/// The `status` a revoked webhook carries. Auth treats it as absent (refused); `list` surfaces it
/// as "revoked" for audit. Tombstoned (not deleted) so it replays cleanly under sync.
pub const TOMBSTONE_STATUS: &str = "__revoked__";

/// The HMAC scheme v1 supports (webhooks-scope open question: v1 ships `hmac-sha256` only).
pub const HMAC_SCHEME: &str = "hmac-sha256";

/// The default header name for `signature` mode if the admin does not pick one (`X-Signature`).
pub const DEFAULT_HMAC_HEADER: &str = "X-Signature";

/// The supported auth modes (webhooks-scope "Goals"). Labelling on top of the credential model —
/// `bearer` reuses the apikey path; `signature` uses an `lb-secrets` shared secret + HMAC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    Bearer,
    Signature,
}

impl AuthMode {
    /// Parse the MCP input string form (`"bearer"` / `"signature"`); unknown ⇒ `None`.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "bearer" => Some(AuthMode::Bearer),
            "signature" => Some(AuthMode::Signature),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            AuthMode::Bearer => "bearer",
            AuthMode::Signature => "signature",
        }
    }
}

/// A stored webhook: identity + auth-mode config + the (mode-specific) credential reference. The
/// raw secret / apikey hash is NEVER here (shown once at create; the apikey row holds its own hash,
/// the signature shared-secret lives in `lb-secrets`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebhookRecord {
    /// The webhook id (Crockford base32, no `_`/`.`). The URL path segment and the producer stamp.
    pub id: String,
    /// The workspace this webhook is walled to (mirrors the namespace; the wall is the namespace).
    pub ws: String,
    /// A human label (`plant-alerts`). Display only.
    pub name: String,
    /// The derived series hits land on: `webhook:{ws}:{id}`. Persisted so list/get surface it.
    pub series: String,
    /// How the inbound request is authenticated.
    pub auth_mode: AuthMode,
    /// `bearer` mode: the linked apikey id (whose hash + grants live on the `apikey` row). None in
    /// `signature` mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bearer_key_id: Option<String>,
    /// `signature` mode: the `lb-secrets` path holding the shared secret (`webhook/{id}`). None in
    /// `bearer` mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_ref: Option<String>,
    /// `signature` mode: the header name the caller signs (`X-Signature`, `X-Hub-Signature-256`).
    /// Empty in `bearer` mode.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hmac_header: String,
    /// `"active"` or `"__revoked__"` (the tombstone).
    pub status: String,
    /// Caller-injected creation timestamp (no wall-clock — testing §3).
    pub created_ts: u64,
    /// The last accepted hit's logical ts (for audit/diagnostics). `0` when never hit.
    #[serde(default)]
    pub last_hit_at: u64,
    /// Constant `"webhook"` so [`lb_store::list`] filters every row.
    pub kind_discrim: String,
    /// The list order key (== `created_ts`).
    pub ts: u64,
}

impl WebhookRecord {
    /// The derived series id for `webhook:{ws}:{id}`. Host-owned so a hit can never name another
    /// series (the route constructs the Sample with this, never the caller).
    pub fn series_for(ws: &str, id: &str) -> String {
        format!("webhook:{ws}:{id}")
    }

    /// The producer stamp every hit carries (`webhook:{id}`). Constant per webhook, un-spoofable
    /// (the route forces it; `ingest.write` would override `producer` with `principal.sub()`
    /// anyway, so the two coincide for the synthetic webhook principal).
    pub fn producer_for(id: &str) -> String {
        format!("webhook:{id}")
    }

    /// The `lb-secrets` path for a `signature`-mode shared secret.
    pub fn secret_path(id: &str) -> String {
        format!("webhook/{id}")
    }

    /// Build a fresh active record. `ts` is the caller-injected logical clock (no wall-clock).
    pub fn new(
        id: impl Into<String>,
        ws: impl Into<String>,
        name: impl Into<String>,
        auth_mode: AuthMode,
        bearer_key_id: Option<String>,
        secret_ref: Option<String>,
        hmac_header: String,
        created_ts: u64,
    ) -> Self {
        let id = id.into();
        let ws = ws.into();
        let series = Self::series_for(&ws, &id);
        Self {
            id,
            ws,
            name: name.into(),
            series,
            auth_mode,
            bearer_key_id,
            secret_ref,
            hmac_header,
            status: "active".to_string(),
            created_ts,
            last_hit_at: 0,
            kind_discrim: KIND_DISCRIM.to_string(),
            ts: created_ts,
        }
    }

    /// Is this record revoked (tombstoned)?
    pub fn is_revoked(&self) -> bool {
        self.status == TOMBSTONE_STATUS
    }

    /// The credential-prefix view of the auth mode, for diagnostics/list badge.
    pub fn auth_mode_label(&self) -> &'static str {
        self.auth_mode.as_str()
    }
}

/// The credential-free list view — `id`, name, series, auth mode, URL, status, timing. Carries **no
/// hash and no secret** (asserted in a test): `webhook.list`/`webhook.get` must never enumerate a
/// credential. The URL is the public stable endpoint (always safe to surface — the credential is
/// the secret, not the path).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebhookView {
    pub id: String,
    pub name: String,
    pub series: String,
    pub auth_mode: String,
    /// The inbound URL path (`/hooks/{ws}/{id}`); the host/gateway resolves the public origin.
    pub url_path: String,
    pub status: String,
    pub created_ts: u64,
    pub last_hit_at: u64,
}

impl WebhookView {
    /// Build the view from a stored record. Never carries the credential.
    pub fn from_record(record: &WebhookRecord) -> Self {
        Self {
            id: record.id.clone(),
            name: record.name.clone(),
            series: record.series.clone(),
            auth_mode: record.auth_mode.as_str().to_string(),
            url_path: format!("/hooks/{}/{}", record.ws, record.id),
            status: record.status.clone(),
            created_ts: record.created_ts,
            last_hit_at: record.last_hit_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn series_is_ws_and_id_derived() {
        assert_eq!(
            WebhookRecord::series_for("acme", "wh_9f2"),
            "webhook:acme:wh_9f2"
        );
    }

    #[test]
    fn secret_path_is_id_derived() {
        assert_eq!(WebhookRecord::secret_path("wh_9f2"), "webhook/wh_9f2");
    }

    #[test]
    fn auth_mode_round_trips() {
        for m in [AuthMode::Bearer, AuthMode::Signature] {
            assert_eq!(AuthMode::parse(m.as_str()), Some(m));
        }
        assert!(AuthMode::parse("garbage").is_none());
        assert!(AuthMode::parse("").is_none());
    }

    #[test]
    fn view_from_record_carries_no_credential() {
        let rec = WebhookRecord::new(
            "wh_1",
            "acme",
            "plant-alerts",
            AuthMode::Signature,
            None,
            Some("webhook/wh_1".into()),
            "X-Signature".into(),
            100,
        );
        let view = WebhookView::from_record(&rec);
        let dumped = serde_json::to_string(&view).unwrap();
        assert!(!dumped.contains("secret"));
        assert!(!dumped.contains("hash"));
        assert!(!dumped.contains("bearer_key_id"));
        assert_eq!(view.url_path, "/hooks/acme/wh_1");
        assert_eq!(view.series, "webhook:acme:wh_1");
    }
}
