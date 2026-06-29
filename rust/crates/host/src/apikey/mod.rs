//! The **apikey** service — long-lived, non-human credentials over the existing authz model
//! (api-keys scope). A key is a durable `apikey:{ws}:{id}` record plus a peppered secret hash;
//! its permissions are grants on `Subject::Key("{id}")` resolved by the SAME `resolve_subject_caps`
//! machinery as a user — read-only/read-write and tool/page limits are just *which caps the key
//! resolves to*, enforced at the one `caps::check` chokepoint. No new permission grammar, surface,
//! or action.
//!
//! Two halves, one per file (FILE-LAYOUT §3):
//!   - **management verbs** (each gated `mcp:apikey.manage:call`): `create` (returns the secret
//!     ONCE), `revoke` (tombstone + cache-bust + grant-revoke), `rotate` (new secret, old dead),
//!     `list` (credential-free views + badge), `get` (full resolved caps).
//!   - **the per-request auth** ([`apikey_authenticate`]): verify a bearer credential and build a
//!     `Principal::for_key` via the cache.
//!
//! The cache ([`ApiKeyCache`]) lives on [`Node`](crate::Node) so revoke/rotate bust the entry the
//! auth path reads — instant local revoke. Expiry is a lazy check at auth (mirroring `verify`'s
//! `exp`); the outbox only tombstones + notifies (housekeeping, never the security gate).

pub mod cache;

mod auth;
mod create;
mod error;
mod get;
mod list;
mod model;
mod revoke;
mod rotate;
mod seed;

pub use auth::apikey_authenticate;
pub use cache::ApiKeyCache;
pub use create::apikey_create;
pub use error::{is_auth_failure, ApiKeyError};
pub use get::apikey_get;
pub use list::apikey_list;
pub use model::{ApiKeyFull, ApiKeyView, KINDS, KIND_DISCRIM, TABLE, TOMBSTONE_STATUS};
pub use revoke::apikey_revoke;
pub use rotate::apikey_rotate;
pub use seed::ensure_builtin_roles;
