//! `versions_capture` — the after-image half of the depth-0 dispatch chokepoint, sibling to
//! `undo_capture` (`docs/scope/versions/entity-version-history-scope.md`, "Capture").
//!
//! Undo journals the **before**-image; history keeps the **after**-image — because "restore version
//! 7" means "what it looked like *after* that save". The two never share storage, keys, or
//! retention; they only share this seam.
//!
//! **Failure direction (load-bearing).** A capture failure never fails the user's save. The save
//! already committed; all this can do is add a history row. Every failure path here therefore
//! `warn!`s loudly and returns — the ring simply misses a version. What it must never do is fail
//! silently, because a history that quietly stopped recording is indistinguishable from one that
//! had nothing to record.

use std::sync::Arc;

use lb_auth::Principal;
use lb_store::{capped_insert, new_ulid, read_versioned, snapshot_safety};
use serde_json::Value;
use tracing::warn;

use crate::Node;

use super::cap::{read_config, resolve_cap};
use super::plan::classify;
use super::record::{cap_key, snapshot_hash, ts_of_ulid, EntityVersion, TABLE};
use super::store::head_hash;

/// Capture a version of whatever entity this **successful, depth-0** call wrote, if any.
///
/// Called after the dispatch (and after undo capture) has returned `Ok` — the after-image only
/// exists once the save has committed. A call that wrote nothing versionable returns immediately
/// without touching the store.
pub(crate) async fn capture_version(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) {
    let Some(c) = classify(qualified_tool, input) else {
        return;
    };
    let store = &node.store;

    // The after-image, read back through the versioned read so the row carries the `rev` this
    // snapshot was taken at (the provenance a reviewer checks against the live record).
    let versioned = match read_versioned(store, ws, c.plan.table, &c.id).await {
        Ok(v) => v,
        Err(e) => {
            warn!(
                kind = c.plan.kind,
                id = %c.id,
                error = %e,
                "version capture: could not read the after-image — this save has no history row"
            );
            return;
        }
    };
    // Absent after a successful save means the verb wrote elsewhere (or the record was removed
    // between the save and here). Nothing to snapshot; not an error worth shouting about.
    let Some(snapshot) = versioned.value else {
        return;
    };

    // The structural guard (undo-exposure-scope's prerequisite, shipped in `lb_store`): a snapshot
    // is refused — never redacted — if it would copy secret material into a durable ring. Redacting
    // would produce a version that LOOKS restorable and would write `***` over a live credential.
    if let Err(refusal) = snapshot_safety(c.plan.table, &snapshot) {
        warn!(
            kind = c.plan.kind,
            id = %c.id,
            refusal = %refusal,
            "version capture REFUSED by the snapshot guard — no history row was written"
        );
        return;
    }

    let hash = snapshot_hash(&snapshot, c.plan.hash_ignore);

    // Dedupe: a no-op save (the UI re-saving an unchanged board, an idempotent retry) must not burn
    // a ring slot and push a real version off the end.
    match head_hash(store, ws, c.plan.kind, &c.id).await {
        Ok(Some(head)) if head == hash => return,
        Ok(_) => {}
        Err(e) => {
            // A failed head read is not a reason to skip the capture — the worst case is a
            // duplicate row, which costs one slot; skipping would cost the version entirely.
            warn!(kind = c.plan.kind, id = %c.id, error = %e, "version capture: dedupe read failed, capturing anyway");
        }
    }

    let cfg = match read_config(store, ws).await {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "version capture: config read failed, using the default cap");
            Default::default()
        }
    };
    let cap = resolve_cap(&cfg, c.plan.kind);

    let version_id = new_ulid();
    let row = EntityVersion {
        version_id: version_id.clone(),
        kind: c.plan.kind.to_string(),
        entity_id: c.id.clone(),
        entity_rev: versioned.rev,
        // The kind's own counter, when it declares one (flows' run-pinning `version`) — read
        // generically from the snapshot by field name, never by matching on the kind.
        entity_version: c
            .plan
            .version_field
            .and_then(|f| snapshot.get(f))
            .and_then(Value::as_u64),
        tool: qualified_tool.to_string(),
        actor: principal.sub().to_string(),
        ts: ts_of_ulid(&version_id),
        hash,
        snapshot,
    };
    let value = match serde_json::to_value(&row) {
        Ok(v) => v,
        Err(e) => {
            warn!(error = %e, "version capture: row would not serialise");
            return;
        }
    };
    // Insert + trim-to-cap in ONE transaction, under the per-key lock — so concurrent saves of the
    // same entity can never over-grow the ring (`crates/store/src/capped.rs`).
    if let Err(e) = capped_insert(
        store,
        ws,
        TABLE,
        &version_id,
        &cap_key(c.plan.kind, &c.id),
        cap,
        &value,
    )
    .await
    {
        warn!(
            kind = c.plan.kind,
            id = %c.id,
            error = %e,
            "version capture: ring insert failed — this save has no history row"
        );
    }
}
