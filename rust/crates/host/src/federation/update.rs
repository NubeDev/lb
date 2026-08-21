//! `datasource.update {name, kind?, endpoint?, dsn?, dsn_set?}` (admin) — edit a registered source
//! WITHOUT retyping its credential (datasources scope, the editable-datasources slice).
//!
//! The DSN is one sealed secret, so a client that lets an admin change the host cannot rebuild it —
//! it never saw the password. Grafana's edit form solves this with "configured" secure fields whose
//! untouched state means *keep what is stored*; this verb is the server half of that contract:
//!
//!   * `dsn`      — full replacement, for when the credential WAS retyped (or a file-kind path).
//!   * `dsn_set`  — a MERGE: `{"host": "db2", "port": "5433"}` overlays the STORED DSN's keyword
//!                  pairs, `null` removes a key, and every key not named — the password — survives
//!                  as it was. The merge happens here, mediated as `ext:federation`, so the value
//!                  never crosses to the caller (§6.7).
//!
//! Both are optional: an update naming neither touches only the record (kind/endpoint). The DSN
//! keyword form is the sidecar's own (`host=… password=… dbname=…`, whitespace-split, raw values) —
//! see `dsnBuild` on the client side for why values never contain whitespace or `=`.
//!
//! Everything else mirrors `add`: admin-gated (`mcp:datasource.update:call`), endpoint changes are
//! self-approved into the net grant (registration IS the approval), and the record keeps its
//! `secret_ref` — editing never re-points the secret.

use lb_auth::Principal;
use serde_json::Value;

use super::authorize::authorize;
use super::error::FederationError;
use super::net::grant_endpoint;
use super::record::{put, resolve, Datasource};
use super::secret::{mediate_dsn, store_dsn};
use crate::boot::Node;

/// Apply an edit to the datasource `name` in `ws`. Fields left `None` keep their stored value.
/// `dsn` (full replacement) wins over `dsn_set` (merge) when both are supplied.
#[allow(clippy::too_many_arguments)]
pub async fn datasource_update(
    node: &Node,
    caller: &Principal,
    ws: &str,
    name: &str,
    kind: Option<&str>,
    endpoint: Option<&str>,
    dsn: Option<&str>,
    dsn_set: Option<&Value>,
    ts: u64,
) -> Result<(), FederationError> {
    authorize(caller, ws, "datasource.update")?;

    let existing = resolve(&node.store, ws, name)
        .await?
        .ok_or(FederationError::NotFound)?;

    if let Some(dsn) = dsn {
        store_dsn(node, ws, &existing.secret_ref, dsn).await?;
    } else if let Some(set) = dsn_set.and_then(|v| v.as_object()) {
        if !set.is_empty() {
            // Read the stored DSN as the mediator (never the caller); a source registered without
            // one starts the merge from empty — the edit can complete a half-configured source.
            let stored = match mediate_dsn(node, ws, &existing.secret_ref).await {
                Ok(v) => v,
                // Registered without a DSN: the merge starts from empty and the edit completes it.
                Err(FederationError::SecretUnavailable) => String::new(),
                Err(e) => return Err(e),
            };
            let mut pairs = parse_keyword_dsn(&stored);
            for (key, val) in set {
                match val.as_str() {
                    Some(v) if !v.is_empty() => upsert(&mut pairs, key, v),
                    // `null` (or empty) removes the key — "clear the password" is expressible.
                    _ => pairs.retain(|(k, _)| k != key),
                }
            }
            store_dsn(node, ws, &existing.secret_ref, &render_keyword_dsn(&pairs)).await?;
        }
    }

    let endpoint = endpoint.unwrap_or(&existing.endpoint);
    if endpoint != existing.endpoint {
        // Same self-approval as `add`: the admin editing the endpoint IS the approval for it.
        grant_endpoint(&node.store, ws, endpoint).await?;
    }

    let ds = Datasource::new(
        name,
        kind.unwrap_or(&existing.kind),
        endpoint,
        existing.secret_ref.clone(),
        ts,
    );
    put(&node.store, ws, &ds).await?;
    Ok(())
}

/// Split a keyword DSN into ordered pairs. The sidecar's parser splits on whitespace and takes each
/// `key=value` raw, so this does exactly that — a token without `=` is dropped (unexpressable here,
/// and the pool would reject it anyway).
fn parse_keyword_dsn(dsn: &str) -> Vec<(String, String)> {
    dsn.split_whitespace()
        .filter_map(|tok| tok.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// Replace `key` in place (keeping the stored order stable) or append it.
fn upsert(pairs: &mut Vec<(String, String)>, key: &str, value: &str) {
    match pairs.iter_mut().find(|(k, _)| k == key) {
        Some((_, v)) => *v = value.to_string(),
        None => pairs.push((key.to_string(), value.to_string())),
    }
}

fn render_keyword_dsn(pairs: &[(String, String)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_replaces_named_keys_and_keeps_the_password() {
        let mut pairs = parse_keyword_dsn(
            "host=old port=5432 user=me password=hunter2 dbname=db sslmode=require",
        );
        upsert(&mut pairs, "host", "new-db.example.com");
        upsert(&mut pairs, "port", "5433");
        assert_eq!(
            render_keyword_dsn(&pairs),
            "host=new-db.example.com port=5433 user=me password=hunter2 dbname=db sslmode=require"
        );
    }

    #[test]
    fn merge_can_add_a_key_the_stored_dsn_lacked() {
        let mut pairs = parse_keyword_dsn("host=h dbname=db");
        upsert(&mut pairs, "sslmode", "require");
        assert_eq!(
            render_keyword_dsn(&pairs),
            "host=h dbname=db sslmode=require"
        );
    }

    #[test]
    fn a_malformed_token_is_dropped_not_kept() {
        assert_eq!(
            parse_keyword_dsn("host=h garbage dbname=db"),
            vec![
                ("host".to_string(), "h".to_string()),
                ("dbname".to_string(), "db".to_string()),
            ]
        );
    }
}
