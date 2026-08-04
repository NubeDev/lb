//! The **structural `Secret<T>` never-in-a-snapshot guard** — the hard prerequisite
//! `docs/scope/undo/undo-exposure-scope.md` names for widening any captured floor, and the gate
//! `docs/scope/versions/entity-version-history-scope.md` sequences before entity version history.
//!
//! Two subsystems now persist **full copies of records the user did not ask to copy**: the undo
//! journal (before-images) and the entity-version ring (after-images). Both multiply the blast
//! radius of any secret material that reaches a captured record — a value that used to live in one
//! place now lives in `N + 1`, in a table with different read gates and a different retention. The
//! guard is that a snapshot is *structurally incapable* of carrying secret material, not that
//! someone reviewed the current kinds and found them clean.
//!
//! ## Two layers, because the material arrives two ways
//!
//! **Layer 1 — the type (`lb_telemetry::Secret<T>`).** Secret material inside the host is wrapped in
//! `Secret<T>`, whose `Debug`/`Display` render `***` and which implements **no `Serialize`**. A Rust
//! struct holding one therefore cannot `derive(Serialize)`, so it can never be turned into the JSON
//! a snapshot is made of. That is a compile error, not a runtime check — the strongest form
//! available, and the reason `Secret<T>` deliberately keeps no `Serialize` impl (adding one that
//! emitted `"***"` would trade a compile error for a *silent* redaction, which is worse here: a
//! redacted snapshot restores `***` over a real credential).
//!
//! **Layer 2 — the JSON boundary (this file).** A snapshot is not built from a Rust type; it is read
//! back out of the store as opaque `serde_json::Value`. Layer 1 cannot bind there, so the one
//! function every snapshotting subsystem calls — [`snapshot_safety`] — decides structurally whether
//! a `(table, value)` pair may be copied into a durable snapshot at all. Both callers (undo capture,
//! versions capture) go through it, so a new snapshotting subsystem inherits the guard by using the
//! same seam rather than by remembering a rule.
//!
//! ## The decision, and why it is refuse-not-redact
//!
//! [`snapshot_safety`] refuses on two structural signals:
//!
//!   1. **The table is part of the secret plane.** `secret` (`lb-secrets`), `credential` /
//!      `identity_credential` (password hashes), and `apikey` (token material) are never snapshotted,
//!      whatever the caller's plan table says. This is a floor *under* every allowlist.
//!   2. **The value carries a secret-shaped leaf** — a non-empty string under one of a narrow set of
//!      unambiguous key names ([`SECRET_KEYS`]), at any depth.
//!
//! The outcome is **refuse the snapshot**, not redact it. A redacted snapshot looks restorable and
//! is not: restoring it would write `"***"` over the live credential — the silent-wrong-restore class
//! the undo scope exists to prevent. A refused snapshot costs the ring one version, which the caller
//! logs loudly; the entity's own save is untouched (capture failure never fails a save).
//!
//! False positives are therefore cheap-but-visible (a missing version + a warning) and false
//! negatives are expensive, so the key list is chosen for *unambiguity*, not coverage: `password`
//! beats `dsn`, which is a legitimate non-secret field on records the platform snapshots today.
//! Widening the list is a deliberate act with a test, exactly like widening the capture floor.

use serde_json::Value;

/// The secret-plane tables, from the ONE canonical list ([`crate::secret_tables`]). Shared with the
/// host's raw-read wall (`store.query`/`store.scan`/`store.graph`) so a table added there is refused
/// by both surfaces at once — a second copy of this list is how the two drift apart.
#[cfg(test)]
use crate::secret_tables::SECRET_TABLES;

/// Object keys whose non-empty string value is treated as secret material at any depth. Deliberately
/// narrow and unambiguous: each of these names a credential in every context we have, so a refusal
/// is never a judgement call about the surrounding record. `dsn`/`url`/`endpoint` are POINTEDLY
/// absent — they are ordinary fields on records the platform snapshots today, and listing them would
/// make the guard fire on healthy data.
const SECRET_KEYS: &[&str] = &[
    "password",
    "passwd",
    "secret",
    "client_secret",
    "token",
    "access_token",
    "refresh_token",
    "api_key",
    "apikey",
    "private_key",
    "secret_key",
    "credential",
    "credentials",
];

/// How deep the shape scan walks. A snapshot deeper than this is refused rather than partially
/// scanned — an unbounded recursion over caller-shaped JSON is its own hazard, and no captured kind
/// is anywhere near this deep.
const MAX_DEPTH: usize = 32;

/// Why a snapshot was refused — carried so the caller can log something actionable instead of
/// "capture failed".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotRefusal {
    /// The table is part of the secret plane ([`SECRET_TABLES`]).
    SecretTable(&'static str),
    /// A secret-shaped leaf was found at this dotted path (e.g. `config.auth.password`).
    SecretKey(String),
    /// The value nests deeper than [`MAX_DEPTH`], so it could not be fully scanned.
    TooDeep,
}

impl std::fmt::Display for SnapshotRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SnapshotRefusal::SecretTable(t) => {
                write!(
                    f,
                    "table `{t}` is part of the secret plane and is never snapshotted"
                )
            }
            SnapshotRefusal::SecretKey(path) => write!(
                f,
                "secret-shaped value at `{path}` — refusing to copy it into a durable snapshot"
            ),
            SnapshotRefusal::TooDeep => write!(
                f,
                "value nests deeper than {MAX_DEPTH} levels and could not be fully scanned"
            ),
        }
    }
}

/// May `value`, read from `table`, be copied into a durable snapshot (an undo before-image, a
/// version ring row)? `Ok(())` = safe; `Err(refusal)` = the caller must **skip the snapshot** and
/// log the reason — never redact and store, and never fail the user's own operation.
///
/// The check is pure, so it is unit-testable without a store and cannot depend on caller state.
pub fn snapshot_safety(table: &str, value: &Value) -> Result<(), SnapshotRefusal> {
    if let Some(t) = crate::secret_tables::secret_table_of(table) {
        return Err(SnapshotRefusal::SecretTable(t));
    }
    scan(value, "", 0)
}

/// Walk `value`, refusing the first secret-shaped leaf. `path` is the dotted breadcrumb used in the
/// refusal so an operator can find the field without dumping the record.
fn scan(value: &Value, path: &str, depth: usize) -> Result<(), SnapshotRefusal> {
    if depth > MAX_DEPTH {
        return Err(SnapshotRefusal::TooDeep);
    }
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                // A secret-shaped KEY refuses only when it actually holds material — an empty
                // string, a null, or a bool carries nothing (see `holds_material`). A non-empty
                // string is refused WITHOUT trying to tell a secret *name* from a secret *value*:
                // that distinction is exactly the per-kind judgement call this guard exists to
                // replace. Kinds that reference secrets by name do so under a differently-named
                // field (`secret_ref`, `source`), which is the shape the platform already uses.
                if is_secret_key(key) && holds_material(child) {
                    return Err(SnapshotRefusal::SecretKey(child_path));
                }
                scan(child, &child_path, depth + 1)?;
            }
            Ok(())
        }
        Value::Array(items) => {
            for (i, child) in items.iter().enumerate() {
                let child_path = format!("{path}[{i}]");
                scan(child, &child_path, depth + 1)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Is `key` one of the unambiguous secret-material names? Case-insensitive and separator-insensitive
/// (`apiKey`, `API_KEY`, `api-key` all normalise to `apikey`), because a record's casing convention
/// must not decide whether the guard fires.
fn is_secret_key(key: &str) -> bool {
    let norm: String = key
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    SECRET_KEYS.iter().any(|k| {
        let kn: String = k.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        kn == norm
    })
}

/// Does this value actually hold material worth refusing? A non-empty string does; `null`, an empty
/// string, a bool, and a number do not (a `"credentials": false` flag or a `"token": ""` placeholder
/// is not a credential, and refusing on those would fire the guard on healthy records).
fn holds_material(v: &Value) -> bool {
    matches!(v, Value::String(s) if !s.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_secret_plane_is_never_snapshotted() {
        for table in SECRET_TABLES {
            let err = snapshot_safety(table, &json!({ "anything": 1 })).unwrap_err();
            assert_eq!(err, SnapshotRefusal::SecretTable(table));
        }
    }

    #[test]
    fn todays_captured_kinds_pass() {
        // A representative dashboard, flow, and rule record — the v1 version-history kinds.
        snapshot_safety(
            "dashboard",
            &json!({ "id": "plant-room", "title": "Plant Room", "cells": [{ "i": "c1", "view": "timeseries" }] }),
        )
        .expect("a dashboard is snapshot-safe");
        snapshot_safety(
            "flow",
            &json!({ "id": "f1", "version": 3, "nodes": [{ "id": "n1", "config": { "cron": "* * * * *" } }] }),
        )
        .expect("a flow is snapshot-safe");
        snapshot_safety(
            "rule",
            &json!({ "id": "r1", "name": "r1", "body": "let x = 1;", "params": [] }),
        )
        .expect("a rule is snapshot-safe");
    }

    #[test]
    fn a_nested_secret_leaf_is_refused_with_its_path() {
        let err = snapshot_safety(
            "flow",
            &json!({ "nodes": [{ "config": { "auth": { "password": "hunter2" } } }] }),
        )
        .unwrap_err();
        assert_eq!(
            err,
            SnapshotRefusal::SecretKey("nodes[0].config.auth.password".to_string())
        );
    }

    #[test]
    fn key_matching_ignores_case_and_separators() {
        for key in ["apiKey", "API_KEY", "api-key", "Api Key"] {
            let v = json!({ "cfg": { key: "sk-live-xyz" } });
            assert!(
                snapshot_safety("dashboard", &v).is_err(),
                "`{key}` must be recognised as secret material"
            );
        }
    }

    /// A placeholder is not a credential — the guard must not fire on healthy records, or every
    /// capture of a record with an empty auth block would silently lose its version.
    #[test]
    fn empty_and_non_string_values_do_not_fire() {
        snapshot_safety("dashboard", &json!({ "token": "" }))
            .expect("empty string is not material");
        snapshot_safety("dashboard", &json!({ "token": null })).expect("null is not material");
        snapshot_safety("dashboard", &json!({ "credentials": false }))
            .expect("a bool flag is not material");
    }

    /// `dsn` / `url` are deliberately NOT in the key list — they are ordinary fields on records the
    /// platform snapshots, and listing them would make the guard fire on healthy data.
    #[test]
    fn ordinary_connection_fields_are_not_treated_as_secrets() {
        snapshot_safety(
            "dashboard",
            &json!({ "dsn": "/var/lib/site.db", "url": "https://example.test" }),
        )
        .expect("dsn/url are not in the unambiguous key set");
    }

    #[test]
    fn a_pathologically_deep_value_is_refused_not_scanned_forever() {
        let mut v = json!("leaf");
        for _ in 0..(MAX_DEPTH + 5) {
            v = json!({ "n": v });
        }
        assert_eq!(
            snapshot_safety("dashboard", &v).unwrap_err(),
            SnapshotRefusal::TooDeep
        );
    }
}
