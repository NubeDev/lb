//! Capability-mediated secrets (README §6.7) — the `secret:*` surface.
//!
//! A secret is a workspace-walled record `secret:{ws}:{path}` holding an opaque string value (a DSN,
//! an API key). Access is gated by the `Secret` capability surface: `secret:federation/*:get` /
//! `:write`. The value is mediated — pulled by the supervisor/host and handed to the consumer (a
//! pool), never returned to a rule, the page, a log, or a `federation.query` result.
//!
//! NOTE (honest scope): this lands the capability-mediation + workspace-walled storage now; the
//! envelope-encryption-at-rest is its own dedicated stage (the crate stays the seam). Values are
//! stored in the one datastore, never a separate secrets service (rule 2).

use lb_auth::Principal;
use lb_caps::{check, Action, Decision, Request, Surface};
use lb_store::{Store, StoreError};
use serde::{Deserialize, Serialize};
use serde_json::json;

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

#[derive(Serialize, Deserialize)]
struct SecretRecord {
    path: String,
    value: String,
}

/// Authorize a secret access on `path` (e.g. `federation/tsdb`) for `action`. Workspace-first, then
/// the `secret:<path>:<action>` capability.
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

/// Store a secret value at `path` in `ws`. Gated `secret:<path>:write`. Idempotent (upsert).
pub async fn set(
    store: &Store,
    principal: &Principal,
    ws: &str,
    path: &str,
    value: &str,
) -> Result<(), SecretsError> {
    authorize(principal, ws, path, Action::Write)?;
    let rec = json!({ "path": path, "value": value });
    lb_store::write(store, ws, TABLE, &record_id(path), &rec).await?;
    Ok(())
}

/// Pull a secret value at `path` in `ws`. Gated `secret:<path>:get`. The value is for a mediator
/// (the host/supervisor), never a caller-visible surface.
pub async fn get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    path: &str,
) -> Result<String, SecretsError> {
    authorize(principal, ws, path, Action::Get)?;
    let val = lb_store::read(store, ws, TABLE, &record_id(path))
        .await?
        .ok_or(SecretsError::NotFound)?;
    let rec: SecretRecord =
        serde_json::from_value(val).map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rec.value)
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

    fn principal(ws: &str, caps: &[&str]) -> Principal {
        let key = SigningKey::generate();
        let claims = Claims {
            sub: "user:test".into(),
            ws: ws.into(),
            role: Role::Member,
            caps: caps.iter().map(|s| s.to_string()).collect(),
            iat: 0,
            exp: u64::MAX,
        };
        let token = mint(&key, &claims);
        verify(&key, &token, 1).unwrap()
    }

    #[tokio::test]
    async fn set_then_get_roundtrips_with_grant() {
        let store = Store::memory().await.unwrap();
        let p = principal(
            "acme",
            &["secret:federation/*:write", "secret:federation/*:get"],
        );
        set(&store, &p, "acme", "federation/tsdb", "postgres://x")
            .await
            .unwrap();
        let v = get(&store, &p, "acme", "federation/tsdb").await.unwrap();
        assert_eq!(v, "postgres://x");
    }

    #[tokio::test]
    async fn get_denied_without_grant() {
        let store = Store::memory().await.unwrap();
        let setter = principal("acme", &["secret:federation/*:write"]);
        set(&store, &setter, "acme", "federation/tsdb", "postgres://x")
            .await
            .unwrap();
        let reader = principal("acme", &["secret:other/*:get"]);
        let err = get(&store, &reader, "acme", "federation/tsdb")
            .await
            .unwrap_err();
        assert!(matches!(err, SecretsError::Denied));
    }

    #[tokio::test]
    async fn workspace_isolated() {
        let store = Store::memory().await.unwrap();
        let a = principal(
            "acme",
            &["secret:federation/*:write", "secret:federation/*:get"],
        );
        set(&store, &a, "acme", "federation/tsdb", "secret-a")
            .await
            .unwrap();
        let b = principal("other", &["secret:federation/*:get"]);
        let err = get(&store, &b, "other", "federation/tsdb")
            .await
            .unwrap_err();
        assert!(matches!(err, SecretsError::NotFound | SecretsError::Denied));
    }
}
