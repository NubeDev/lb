//! Capability-mediated, extension-owned secrets (README §6.7) — the `secret:*` surface.
//!
//! A secret is a workspace-walled record `secret:{ws}:{path}` holding an opaque string value (a DSN,
//! an API key), owned by the subject that created it. Access runs through **three gates**:
//!
//! 1. **Workspace** (structural, gate 1): `secret:{ws}:{path}` lives in the workspace namespace; a
//!    read for ws A physically cannot see ws B. Unchanged from the shipped baseline.
//! 2. **Capability** (gate 2): `secret:{path}:get|write` via `caps::check`.
//! 3. **Ownership / visibility** (gate 3, NEW): the record carries `owner` (the host-stamped
//!    creating subject) and `visibility: Private | Workspace`.
//!    - `get` on a **`Private`** secret resolves `caller.sub() == owner` — denied otherwise, **even
//!      with the capability** (an admin holding `secret:*:get` is denied another extension's Private
//!      secret). The owner wall is *below* the cap.
//!    - `get` on a **`Workspace`** secret: any principal past gates 1+2 may read.
//!    - `set` (overwrite), `set_visibility`, and `delete` are **owner-only** regardless of
//!      visibility — only the owner mutates its own record.
//!
//! The value is mediated — pulled by the host/supervisor and handed to the consumer (a pool, the
//! owning extension's own process), never returned to a rule, the page, a log, a `federation.query`
//! result, or `secret.list`. Ownership is the host-derived `caller ∩ install-grant` principal
//! (`ext:{id}` / `user:…`), never a guest-supplied claim — the host stamps `owner` at write time.
//!
//! NOTE (honest scope): envelope-encryption-at-rest is its own dedicated stage (the crate stays the
//! seam); values are plaintext-in-store for now. Stored in the one datastore, never a separate
//! secrets service (rule 2).

use lb_auth::Principal;
use lb_caps::{check, Action, Decision, Request, Surface};
use lb_store::{delete as store_delete, read, write, Store, StoreError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(thiserror::Error, Debug)]
pub enum SecretsError {
    #[error("denied")]
    Denied,
    #[error("not found")]
    NotFound,
    #[error(transparent)]
    Store(#[from] StoreError),
}

const TABLE: &str = "secret";

/// Owner-controlled visibility (secrets scope). `Private` walls the value behind the owner even
/// against a workspace admin holding a broad `secret:*:get` grant; `Workspace` opens it to any
/// principal in the workspace that passes the capability gate. The owner flips this at runtime via
/// [`set_visibility`] — it is NOT an admin capability re-grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    #[default]
    Private,
    Workspace,
}

impl Visibility {
    /// Parse from the MCP input string form (`"private"` / `"workspace"`).
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "private" => Some(Visibility::Private),
            "workspace" => Some(Visibility::Workspace),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Visibility::Private => "private",
            Visibility::Workspace => "workspace",
        }
    }
}

/// Metadata for a secret, as returned by [`list`]. **Never carries the value** — `list` is the
/// workspace-browse surface and must not leak plaintext (the mediation invariant).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretMeta {
    pub path: String,
    pub owner: String,
    pub visibility: Visibility,
}

#[derive(Serialize, Deserialize)]
struct SecretRecord {
    path: String,
    value: String,
    owner: String,
    visibility: Visibility,
}

/// Authorize a secret access on `path` (e.g. `federation/tsdb`, `ext/mqtt/broker-pw`) for `action`.
/// Workspace-first (gate 1), then the `secret:<path>:<action>` capability (gate 2).
fn authorize(
    principal: &Principal,
    ws: &str,
    path: &str,
    action: Action,
) -> Result<(), SecretsError> {
    let req = Request::new(ws, Surface::Secret, path, action);
    match check(principal, &req) {
        Decision::Allowed => Ok(()),
        Decision::Denied(_) => Err(SecretsError::Denied),
    }
}

/// Read the record at `path`, if present. Assumes gates 1+2 already passed.
async fn load(store: &Store, ws: &str, path: &str) -> Result<Option<SecretRecord>, SecretsError> {
    let val = read(store, ws, TABLE, &record_id(path)).await?;
    val.map(|v| {
        serde_json::from_value::<SecretRecord>(v).map_err(|e| StoreError::Decode(e.to_string()))
    })
    .transpose()
    .map_err(SecretsError::from)
}

/// Store a secret value at `path` in `ws`, owned by `principal.sub()`, default [`Visibility::Private`].
/// Gated `secret:<path>:write`. Idempotent upsert on the owner's own record — overwriting an
/// existing secret requires the caller to be its owner (gate 3). The host stamps `owner` from the
/// verified principal; a guest cannot assert its own subject.
pub async fn set(
    store: &Store,
    principal: &Principal,
    ws: &str,
    path: &str,
    value: &str,
) -> Result<(), SecretsError> {
    set_with(store, principal, ws, path, value, Visibility::Private).await
}

/// Like [`set`], but with an explicit initial [`Visibility`]. Use this when a secret is created
/// already-intended to be workspace-shared (e.g. a federated datasource DSN the platform pool
/// consumes via mediated injection, running as a different `ext:` principal than the admin who
/// registered it).
pub async fn set_with(
    store: &Store,
    principal: &Principal,
    ws: &str,
    path: &str,
    value: &str,
    visibility: Visibility,
) -> Result<(), SecretsError> {
    authorize(principal, ws, path, Action::Write)?;
    // Gate 3 (mutation): overwriting an existing secret is owner-only. A brand-new path has no
    // owner yet, so the caller becomes it.
    if let Some(existing) = load(store, ws, path).await? {
        if existing.owner != principal.sub() {
            return Err(SecretsError::Denied);
        }
    }
    let rec = json!({
        "path": path,
        "value": value,
        "owner": principal.sub(),
        "visibility": visibility,
    });
    write(store, ws, TABLE, &record_id(path), &rec).await?;
    Ok(())
}

/// Write a secret and TAKE ownership for `principal`, bypassing the gate-3 owner-overwrite wall.
/// Gates 1 (workspace) and 2 (capability `secret:<path>:write`) still apply; only the "you must
/// already be the owner to overwrite" check is skipped, and the record's `owner` is stamped to
/// `principal.sub()`.
///
/// This is a NARROW, deliberate path for a **host-mediated secret** — one the host manages on behalf
/// of an extension under a single stable principal (e.g. a federated datasource DSN owned by
/// `ext:federation`). It exists to HEAL an ownership drift: a record written by an earlier,
/// differently-named bootstrap principal is reclaimed by the canonical mediator so subsequent
/// CRUD by any admin is collision-free. NOT for user-owned secrets — those keep the owner wall so
/// one member can never overwrite another's secret. See `federation::secret::store_dsn`.
pub async fn reclaim(
    store: &Store,
    principal: &Principal,
    ws: &str,
    path: &str,
    value: &str,
    visibility: Visibility,
) -> Result<(), SecretsError> {
    authorize(principal, ws, path, Action::Write)?;
    let rec = json!({
        "path": path,
        "value": value,
        "owner": principal.sub(),
        "visibility": visibility,
    });
    write(store, ws, TABLE, &record_id(path), &rec).await?;
    Ok(())
}

/// Pull a secret value at `path` in `ws`. Gated `secret:<path>:get` (gate 2), then gate 3:
/// a `Private` secret resolves `caller.sub() == owner` — denied otherwise, even with the cap.
/// The value is for an authorized direct consumer (the owner, or any ws member when `Workspace`),
/// never a caller-visible surface that is not the consumer.
pub async fn get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    path: &str,
) -> Result<String, SecretsError> {
    authorize(principal, ws, path, Action::Get)?;
    let rec = load(store, ws, path).await?.ok_or(SecretsError::NotFound)?;
    // Gate 3 — the owner wall. Private => owner only; Workspace => any principal past gates 1+2.
    if rec.visibility == Visibility::Private && rec.owner != principal.sub() {
        return Err(SecretsError::Denied);
    }
    Ok(rec.value)
}

/// Flip a secret's visibility at runtime (owner-only). Gated `secret:<path>:write`, then gate 3:
/// only the owner may toggle. This is the owner-owned runtime decision — NOT an admin capability
/// re-grant — so the owner can flip `Workspace → Private` back without chasing down grants.
pub async fn set_visibility(
    store: &Store,
    principal: &Principal,
    ws: &str,
    path: &str,
    visibility: Visibility,
) -> Result<(), SecretsError> {
    authorize(principal, ws, path, Action::Write)?;
    let mut rec = load(store, ws, path).await?.ok_or(SecretsError::NotFound)?;
    if rec.owner != principal.sub() {
        return Err(SecretsError::Denied);
    }
    rec.visibility = visibility;
    let val = serde_json::to_value(&rec).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, &record_id(path), &val).await?;
    Ok(())
}

/// Delete a secret (owner-only). Gated `secret:<path>:write`, then gate 3: only the owner may
/// erase its own record. Idempotent at the store layer.
pub async fn delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    path: &str,
) -> Result<(), SecretsError> {
    authorize(principal, ws, path, Action::Write)?;
    let rec = load(store, ws, path).await?.ok_or(SecretsError::NotFound)?;
    if rec.owner != principal.sub() {
        return Err(SecretsError::Denied);
    }
    store_delete(store, ws, TABLE, &record_id(path)).await?;
    Ok(())
}

/// List the **metadata** of every secret in `ws`. Returns `{path, owner, visibility}` — **never
/// values** (the mediation invariant: `list` is the browse surface and must not leak plaintext).
/// Gated `secret:*:get` (the workspace-browse grant). A caller sees metadata for ALL secrets in the
/// workspace regardless of visibility — visibility gates the *value*, not the existence; an owner
/// needs to discover a shared secret's path to request access. The value remains behind [`get`].
pub async fn list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<SecretMeta>, SecretsError> {
    // The browse grant: `secret:*:get` (one segment) covers any single-segment top-level path; for
    // nested paths (`ext/mqtt/broker-pw`) use `secret:**:get`. Require the broad grant so listing
    // is itself a deliberate capability, not a free side-channel.
    authorize(principal, ws, "**", Action::Get)?;
    let rows = store_list_all(store, ws).await?;
    let mut out = Vec::with_capacity(rows.len());
    for v in rows {
        let rec: SecretRecord =
            serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?;
        out.push(SecretMeta {
            path: rec.path,
            owner: rec.owner,
            visibility: rec.visibility,
        });
    }
    Ok(out)
}

/// Read every record in the workspace's `secret` table. Uses the public raw-query escape hatch
/// (no field filter — secrets have no single shared discriminator); isolation still holds: the
/// namespace is selected from `ws` inside `query_ws`, so this can only ever return this
/// workspace's rows (gate 1). Each row's stored `data` field is unwrapped to the [`SecretRecord`]
/// JSON, exactly like [`read`].
async fn store_list_all(store: &Store, ws: &str) -> Result<Vec<Value>, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            "SELECT data FROM type::table($tb)",
            vec![("tb".into(), Value::String(TABLE.to_string()))],
        )
        .await?;
    let rows: Vec<Value> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rows
        .into_iter()
        .filter_map(|r| r.get("data").cloned())
        .collect())
}

/// A record id derived from the secret path. `/` is flattened to `_`; the original `path` rides on
/// the record so two paths never silently collide on a read.
fn record_id(path: &str) -> String {
    path.replace('/', "_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_auth::{mint, verify, Claims, Role, SigningKey};

    /// Build a verified principal in `ws` with `sub` and the given caps.
    fn principal_with(sub: &str, ws: &str, caps: &[&str]) -> Principal {
        let key = SigningKey::generate();
        let claims = Claims {
            sub: sub.into(),
            ws: ws.into(),
            role: Role::Member,
            caps: caps.iter().map(|s| s.to_string()).collect(),
            iat: 0,
            exp: u64::MAX,
        };
        verify(&key, &mint(&key, &claims), 1).unwrap()
    }

    fn user(ws: &str, caps: &[&str]) -> Principal {
        principal_with("user:test", ws, caps)
    }

    /// An extension principal standing in for the host-stamped `ext:{id}` principal. The owner wall
    /// is proven across two of these (the two-guest deny in the scope's testing plan).
    fn ext(id: &str, ws: &str, caps: &[&str]) -> Principal {
        Principal::routed(
            format!("ext:{id}"),
            ws.to_string(),
            caps.iter().map(|s| s.to_string()).collect(),
        )
    }

    #[tokio::test]
    async fn set_then_get_roundtrips_with_grant() {
        let store = Store::memory().await.unwrap();
        let p = user(
            "acme",
            &["secret:federation/*:write", "secret:federation/*:get"],
        );
        set(&store, &p, "acme", "federation/tsdb", "postgres://x")
            .await
            .unwrap();
        let v = get(&store, &p, "acme", "federation/tsdb").await.unwrap();
        assert_eq!(v, "postgres://x");
    }

    // --- Capability-deny (gate 2, mandatory) -----------------------------------------------

    #[tokio::test]
    async fn get_denied_without_get_cap() {
        let store = Store::memory().await.unwrap();
        let setter = user("acme", &["secret:federation/*:write"]);
        set(&store, &setter, "acme", "federation/tsdb", "postgres://x")
            .await
            .unwrap();
        // Different path grant — no :get on federation/*.
        let reader = user("acme", &["secret:other/*:get"]);
        let err = get(&store, &reader, "acme", "federation/tsdb")
            .await
            .unwrap_err();
        assert!(matches!(err, SecretsError::Denied));
    }

    #[tokio::test]
    async fn set_denied_without_write_cap() {
        let store = Store::memory().await.unwrap();
        let p = user("acme", &["secret:federation/*:get"]);
        let err = set(&store, &p, "acme", "federation/tsdb", "v")
            .await
            .unwrap_err();
        assert!(matches!(err, SecretsError::Denied));
    }

    // --- Ownership-deny (gate 3, the NEW load-bearing test) --------------------------------

    #[tokio::test]
    async fn non_owner_with_get_cap_denied_private_secret() {
        let store = Store::memory().await.unwrap();
        let mqtt = ext(
            "mqtt",
            "acme",
            &["secret:ext/mqtt/*:write", "secret:ext/mqtt/*:get"],
        );
        set(&store, &mqtt, "acme", "ext/mqtt/broker-pw", "s3cr3t")
            .await
            .unwrap();
        // A reporting extension holds the literal path grant but is NOT the owner.
        let reporting = ext("reporting", "acme", &["secret:ext/mqtt/broker-pw:get"]);
        let err = get(&store, &reporting, "acme", "ext/mqtt/broker-pw")
            .await
            .unwrap_err();
        assert!(
            matches!(err, SecretsError::Denied),
            "non-owner with the cap is denied a Private secret (the owner wall)"
        );
    }

    #[tokio::test]
    async fn admin_broad_cap_denied_private_extension_secret() {
        let store = Store::memory().await.unwrap();
        let mqtt = ext(
            "mqtt",
            "acme",
            &["secret:ext/mqtt/*:write", "secret:ext/mqtt/*:get"],
        );
        set(&store, &mqtt, "acme", "ext/mqtt/broker-pw", "s3cr3t")
            .await
            .unwrap();
        // An admin holding the broad secret:**:get grant — still denied the Private secret.
        let admin = principal_with("user:admin", "acme", &["secret:**:get", "secret:**:write"]);
        let err = get(&store, &admin, "acme", "ext/mqtt/broker-pw")
            .await
            .unwrap_err();
        assert!(
            matches!(err, SecretsError::Denied),
            "the owner wall holds even against a broad-cap admin"
        );
    }

    // --- Visibility toggle (owner-only, runtime) -------------------------------------------

    #[tokio::test]
    async fn owner_flip_to_workspace_lets_sibling_read_then_back_denies() {
        let store = Store::memory().await.unwrap();
        let mqtt = ext(
            "mqtt",
            "acme",
            &[
                "secret:ext/mqtt/*:write",
                "secret:ext/mqtt/*:get",
                "secret:**:write",
            ],
        );
        set(&store, &mqtt, "acme", "ext/mqtt/weather-key", "k")
            .await
            .unwrap();
        let reporting = ext("reporting", "acme", &["secret:ext/mqtt/weather-key:get"]);

        // Private by default → sibling denied.
        assert!(matches!(
            get(&store, &reporting, "acme", "ext/mqtt/weather-key")
                .await
                .unwrap_err(),
            SecretsError::Denied
        ));

        // Owner flips to Workspace → sibling now reads.
        set_visibility(
            &store,
            &mqtt,
            "acme",
            "ext/mqtt/weather-key",
            Visibility::Workspace,
        )
        .await
        .unwrap();
        assert_eq!(
            get(&store, &reporting, "acme", "ext/mqtt/weather-key")
                .await
                .unwrap(),
            "k"
        );

        // Owner flips back to Private → sibling denied again.
        set_visibility(
            &store,
            &mqtt,
            "acme",
            "ext/mqtt/weather-key",
            Visibility::Private,
        )
        .await
        .unwrap();
        assert!(matches!(
            get(&store, &reporting, "acme", "ext/mqtt/weather-key")
                .await
                .unwrap_err(),
            SecretsError::Denied
        ));
    }

    #[tokio::test]
    async fn only_owner_may_toggle_visibility() {
        let store = Store::memory().await.unwrap();
        let mqtt = ext(
            "mqtt",
            "acme",
            &["secret:ext/mqtt/*:write", "secret:ext/mqtt/*:get"],
        );
        set(&store, &mqtt, "acme", "ext/mqtt/broker-pw", "s3cr3t")
            .await
            .unwrap();
        // Sibling holds the write cap on the path but is not the owner.
        let reporting = ext("reporting", "acme", &["secret:ext/mqtt/broker-pw:write"]);
        let err = set_visibility(
            &store,
            &reporting,
            "acme",
            "ext/mqtt/broker-pw",
            Visibility::Workspace,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, SecretsError::Denied));
    }

    #[tokio::test]
    async fn only_owner_may_overwrite_or_delete() {
        let store = Store::memory().await.unwrap();
        let mqtt = ext(
            "mqtt",
            "acme",
            &["secret:ext/mqtt/*:write", "secret:ext/mqtt/*:get"],
        );
        set(&store, &mqtt, "acme", "ext/mqtt/broker-pw", "s3cr3t")
            .await
            .unwrap();
        let reporting = ext("reporting", "acme", &["secret:ext/mqtt/broker-pw:write"]);

        // Non-owner overwrite denied.
        assert!(matches!(
            set(&store, &reporting, "acme", "ext/mqtt/broker-pw", "x")
                .await
                .unwrap_err(),
            SecretsError::Denied
        ));
        // Non-owner delete denied.
        assert!(matches!(
            delete(&store, &reporting, "acme", "ext/mqtt/broker-pw")
                .await
                .unwrap_err(),
            SecretsError::Denied
        ));

        // Owner may overwrite + delete.
        set(&store, &mqtt, "acme", "ext/mqtt/broker-pw", "new")
            .await
            .unwrap();
        assert_eq!(
            get(&store, &mqtt, "acme", "ext/mqtt/broker-pw")
                .await
                .unwrap(),
            "new"
        );
        delete(&store, &mqtt, "acme", "ext/mqtt/broker-pw")
            .await
            .unwrap();
        assert!(matches!(
            get(&store, &mqtt, "acme", "ext/mqtt/broker-pw")
                .await
                .unwrap_err(),
            SecretsError::NotFound
        ));
    }

    /// `reclaim` HEALS an ownership drift the owner wall would otherwise deadlock: a record written
    /// by one principal (an earlier bootstrap identity) is force-rewritten AND re-owned by the
    /// canonical mediator, so subsequent owner-gated ops (overwrite/delete) by that mediator pass.
    /// This is the federation-DSN CRUD regression: a datasource seeded under `ext:federation-bootstrap`
    /// then updated under `ext:federation` must not be denied.
    #[tokio::test]
    async fn reclaim_takes_ownership_from_an_earlier_principal() {
        let store = Store::memory().await.unwrap();
        // The stale record: owned by the boot bootstrap principal.
        let bootstrap = ext(
            "federation-bootstrap",
            "acme",
            &["secret:federation/*:write", "secret:federation/*:get"],
        );
        set(
            &store,
            &bootstrap,
            "acme",
            "federation/tsdb",
            "postgres://old",
        )
        .await
        .unwrap();

        // The canonical mediator (a DIFFERENT sub) cannot overwrite via the owner-walled `set`...
        let fed = ext(
            "federation",
            "acme",
            &["secret:federation/*:write", "secret:federation/*:get"],
        );
        assert!(matches!(
            set(&store, &fed, "acme", "federation/tsdb", "postgres://new")
                .await
                .unwrap_err(),
            SecretsError::Denied
        ));

        // ...but `reclaim` rewrites the value AND re-owns it.
        reclaim(
            &store,
            &fed,
            "acme",
            "federation/tsdb",
            "postgres://new",
            Visibility::Workspace,
        )
        .await
        .unwrap();
        assert_eq!(
            get(&store, &fed, "acme", "federation/tsdb").await.unwrap(),
            "postgres://new"
        );
        // Now the mediator is the owner: it may overwrite and delete without denial.
        set(&store, &fed, "acme", "federation/tsdb", "postgres://newer")
            .await
            .unwrap();
        delete(&store, &fed, "acme", "federation/tsdb")
            .await
            .unwrap();
    }

    /// `reclaim` still enforces gate 2 (the write capability). Owner-reset is not an auth bypass.
    #[tokio::test]
    async fn reclaim_still_requires_the_write_cap() {
        let store = Store::memory().await.unwrap();
        let no_write = ext("federation", "acme", &["secret:federation/*:get"]);
        assert!(matches!(
            reclaim(
                &store,
                &no_write,
                "acme",
                "federation/tsdb",
                "x",
                Visibility::Workspace,
            )
            .await
            .unwrap_err(),
            SecretsError::Denied
        ));
    }

    // --- Workspace isolation (gate 1, mandatory) -------------------------------------------

    #[tokio::test]
    async fn workspace_isolated() {
        let store = Store::memory().await.unwrap();
        let a = ext(
            "mqtt",
            "acme",
            &["secret:ext/mqtt/*:write", "secret:ext/mqtt/*:get"],
        );
        set(&store, &a, "acme", "ext/mqtt/broker-pw", "secret-a")
            .await
            .unwrap();
        // ws-B principal with the same caps — gate 1 refuses before resolve: ws-B's read cannot
        // see ws-A's record (NotFound, the namespace wall), even with identical caps.
        let b = ext(
            "mqtt",
            "other",
            &["secret:ext/mqtt/*:write", "secret:ext/mqtt/*:get"],
        );
        assert!(matches!(
            get(&store, &b, "other", "ext/mqtt/broker-pw")
                .await
                .unwrap_err(),
            SecretsError::Denied | SecretsError::NotFound
        ));
        // ws-B writing the SAME path lands in ws-B's OWN namespace (a different record), never
        // overwriting or leaking ws-A's value. Prove the wall holds both ways: ws-B's write
        // succeeds (it is ws-B's namespace), then ws-A still reads its own untouched value.
        set(&store, &b, "other", "ext/mqtt/broker-pw", "leak-attempt")
            .await
            .unwrap();
        assert_eq!(
            get(&store, &a, "acme", "ext/mqtt/broker-pw").await.unwrap(),
            "secret-a",
            "ws-B's write did NOT clobber ws-A's secret (the namespace wall)"
        );
    }

    // --- Mediation invariant (absence test) ------------------------------------------------

    #[tokio::test]
    async fn list_returns_metadata_only_never_values() {
        let store = Store::memory().await.unwrap();
        let mqtt = ext(
            "mqtt",
            "acme",
            &[
                "secret:ext/mqtt/*:write",
                "secret:ext/mqtt/*:get",
                "secret:**:get",
            ],
        );
        let sensitive = "super-secret-value-leaked-via-list";
        set(&store, &mqtt, "acme", "ext/mqtt/broker-pw", sensitive)
            .await
            .unwrap();

        let metas = list(&store, &mqtt, "acme").await.unwrap();
        let dumped = serde_json::to_string(&metas).unwrap();
        assert!(
            !dumped.contains(sensitive),
            "list LEAKED the secret value: {dumped}"
        );
        assert!(metas.iter().any(|m| m.path == "ext/mqtt/broker-pw"));
        assert!(metas.iter().all(|m| m.visibility == Visibility::Private));
        assert!(metas.iter().all(|m| m.owner == "ext:mqtt"));
    }

    #[tokio::test]
    async fn value_never_in_error_message() {
        let store = Store::memory().await.unwrap();
        let mqtt = ext(
            "mqtt",
            "acme",
            &["secret:ext/mqtt/*:write", "secret:ext/mqtt/*:get"],
        );
        let sensitive = "the-real-password";
        set(&store, &mqtt, "acme", "ext/mqtt/broker-pw", sensitive)
            .await
            .unwrap();
        let reporting = ext("reporting", "acme", &["secret:ext/mqtt/broker-pw:get"]);
        let err = get(&store, &reporting, "acme", "ext/mqtt/broker-pw")
            .await
            .unwrap_err();
        assert!(!format!("{err}").contains(sensitive));
    }

    // --- Workspace-visibility happy path + mediated-consumer shape -------------------------

    #[tokio::test]
    async fn workspace_secret_readable_by_any_member_with_cap() {
        let store = Store::memory().await.unwrap();
        // Admin registers a federation DSN as Workspace-shared — the mediated pool runs as a
        // DIFFERENT ext principal, so it must read a Workspace secret.
        let admin = principal_with(
            "user:admin",
            "acme",
            &["secret:federation/*:write", "secret:**:write"],
        );
        set_with(
            &store,
            &admin,
            "acme",
            "federation/tsdb",
            "postgres://pool",
            Visibility::Workspace,
        )
        .await
        .unwrap();
        let pool = ext("federation", "acme", &["secret:federation/tsdb:get"]);
        assert_eq!(
            get(&store, &pool, "acme", "federation/tsdb").await.unwrap(),
            "postgres://pool"
        );
    }
}
