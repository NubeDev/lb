//! The one record lb writes of its own (node-update scope §Data): an **audit** entry per
//! `apply`/`rollback`/`credential.*` and per completed upload — actor, workspace, subject, verdict —
//! "because *who replaced the binary on this box* must survive the binary".
//!
//! `update.history` still comes from the provider: the executor's journal is the authority on what
//! happened to the binary, lb's audit on who asked (scope decision 2). The two are merged by `tx`.
//!
//! The record NEVER carries the credential — only a fingerprint, and only when the verb had one.

use lb_store::{new_ulid, write, Store};
use serde_json::json;

/// The audit table. Host-owned and reserved, so no `store.write` holder can forge or erase a row.
pub const TABLE: &str = "update_audit";

/// Append one audit row in `ws`. Best-effort by contract: a failure to audit is logged and never
/// turns a successful verb into a failed one — but it IS logged, because a silent audit gap is the
/// failure this record exists to prevent.
///
/// `subject` is the verb's object (a version, `"credential"`, or `"{sink}:{digest}"`); `verdict` is
/// what lb observed (`"accepted"`, `"sealed"`, `"auto_enrolled"`, `"completed"`, …).
pub async fn record(
    store: &Store,
    ws: &str,
    actor: &str,
    verb: &str,
    subject: &str,
    verdict: &str,
    tx: Option<&str>,
) {
    let id = new_ulid();
    let rec = json!({
        "id": id,
        "actor": actor,
        "workspace": ws,
        "verb": verb,
        "subject": subject,
        "verdict": verdict,
        "tx": tx,
    });
    if let Err(e) = write(store, ws, TABLE, &id, &rec).await {
        tracing::warn!(
            target: "lb::update",
            "update audit write failed for {verb}/{subject}: {e}"
        );
    }
}
