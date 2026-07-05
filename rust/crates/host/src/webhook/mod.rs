//! The **webhook** service — a first-class, named, workspace-walled, credential-protected inbound
//! HTTP surface (webhooks scope). Composes three pieces the platform already owns:
//!
//! - the **credential** is the API-key model (`§api-keys`). `bearer` mode literally issues a real
//!   `apikey:{ws}:{keyid}` record scoped to the hook (subject `key:webhook:{id}`, one narrowed cap
//!   `mcp:ingest.write:call`); `signature` mode stores a shared secret in `lb-secrets` and
//!   verifies HMAC-SHA256 over the raw body (the universal `sha256=<hex>` shape, admin-picked
//!   header). No new auth system, no new bearer format.
//! - the **endpoint** is one gateway route `POST /hooks/{ws}/{id}` (in
//!   `role/gateway/src/routes/webhook.rs`). The route holds no business logic; it captures the raw
//!   body, calls [`webhook_resolve`] (load record + per-mode verify → Principal), then
//!   [`webhook_accept`] (build a `Sample`, write through the existing `ingest.write`, drain+publish
//!   motion, bump `last_hit_at`).
//! - the **reaction** is whatever subscribes to the series `webhook:{ws}:{id}` — a flow's
//!   `webhook` source node, a rule, a dashboard tile, or a raw `series.read`. The webhook service
//!   is a **producer** of ingest samples, never a second store.
//!
//! ## The hard constraint (rule 10)
//! A webhook is a **generic authenticated HTTP inlet that emits a `Sample`**. There is **no Slack
//! webhook, no GitHub webhook, no Stripe webhook** in this crate — those are *shapes of payload a
//! caller sends*, normalized (if at all) by an out-of-core bridge extension
//! (`lb-role-github-webhook`), never a branch in the host. If a provider name appears in this
//! crate, the scope has failed.
//!
//! ## Files (FILE-LAYOUT §3, one verb per file)
//! - [`model`] — the `webhook` record + the credential-free views.
//! - [`error`] — the service errors (`Denied` opaque, `NotFound` opaque on the public route).
//! - [`create`] — `webhook.create` (both modes); returns the secret ONCE.
//! - [`list`] / [`get`] — credential-free enumeration.
//! - [`revoke`] — tombstone + revoke the linked apikey + cache-bust.
//! - [`rotate`] — replace the credential (delegate to `apikey_rotate` for bearer; overwrite the
//!   `lb-secrets` shared secret for signature).
//! - [`verify`] — the `signature`-mode HMAC verifier (constant-time, raw-body).
//! - [`auth`] — [`webhook_resolve`]: the inbound auth path the route calls.
//! - [`accept`] — [`webhook_accept`]: build a `Sample`, write through `ingest.write`.
//! - [`secret`] — the shared-secret generator (reuses `lb_apikey::generate_secret`).

pub mod accept;
pub mod auth;
pub mod verify;

mod create;
mod error;
mod get;
mod list;
mod model;
mod revoke;
mod rotate;
mod secret;

pub use accept::webhook_accept;
pub use auth::{webhook_resolve, INGEST_CAP};
pub use create::{webhook_create, CreateArgs, CreatedWebhook};
pub use error::WebhookError;
pub use get::webhook_get;
pub use list::webhook_list;
pub use model::{
    AuthMode, WebhookRecord, WebhookView, DEFAULT_HMAC_HEADER, HMAC_SCHEME, KIND_DISCRIM, TABLE,
    TOMBSTONE_STATUS,
};
pub use revoke::webhook_revoke;
pub use rotate::webhook_rotate;
pub use verify::{verify_signature, SignatureError};

/// The host re-uses the apikey verification cache for `bearer`-mode webhooks (one cache per node,
/// busted by `apikey.revoke`/`apikey.rotate`/`webhook.revoke`/`webhook.rotate`). The alias keeps
/// the webhook verb signatures readable (`&ApiKeyCache` not `&lb_host::ApiKeyCache`).
pub use crate::apikey::ApiKeyCache;
