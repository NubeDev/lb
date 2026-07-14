//! `boot_workspaces` — which workspaces the node brings extensions up for at boot
//! (lifecycle-management scope).
//!
//! Boot bring-up used to run for exactly ONE workspace: the node's configured `cfg.workspace`. Both
//! tiers had it (`load_enabled` and `spawn_enabled` are each called with a single `ws`), so on a node
//! serving more than one workspace — which the platform supports everywhere else: `workspace.create`
//! is a verb, the UI has a switcher — **every other workspace's extensions stayed dead after a
//! restart, silently.** Same shape as issue #64, one level up: durable intent that says run, and
//! nothing that reads it.
//!
//! The set is `cfg.workspace` ∪ every **active** registered workspace:
//!
//! - The **union** is deliberate, not belt-and-braces. The workspace directory is written by
//!   `workspace_create`, so a node whose boot workspace was never created through that verb (the
//!   default `acme`, every test, an embedder that provisions its own identities) has NO row for it.
//!   Keying bring-up off the directory alone would start nothing at all on those nodes — trading a
//!   one-workspace gap for a zero-workspace one. `cfg.workspace` is always in the set, whether or not
//!   anyone registered it.
//! - **Active only.** An `Archived` workspace is soft-deleted: hidden and un-mintable, its data
//!   retained pending un-archive. Spawning its sidecars would resurrect exactly the activity the
//!   archive suppressed — and a purge tombstone (`kind = "__purged__"`) is not a workspace at all, so
//!   it never matches the `kind = "workspace"` filter `workspace_list` uses.
//!
//! This reads the node-level directory (a reserved namespace — node infrastructure, §7's carve-out),
//! not any tenant's data, and it crosses no wall: each returned id is then brought up *inside its own
//! workspace*, through the same per-workspace verbs, each of which re-enters that namespace.

use std::collections::BTreeSet;

use lb_store::{list, Store};

use super::error::ExtError;
use crate::workspaces::{WorkspaceRecord, WorkspaceStatus, KIND, TABLE, WORKSPACES_NS};

/// Every workspace the node should bring extensions up for: the configured `boot_ws` plus each
/// **active** workspace in the node's directory, deduped and ordered (`BTreeSet` — a stable boot log
/// beats arrival order).
///
/// Never fails the boot over the directory: an unreadable/absent directory degrades to just
/// `boot_ws`, which is exactly today's behaviour. The extra workspaces are an addition to the set,
/// so the worst case is the status quo rather than a node that brings up nothing.
pub async fn boot_workspaces(store: &Store, boot_ws: &str) -> Result<Vec<String>, ExtError> {
    let mut out = BTreeSet::new();
    out.insert(boot_ws.to_string());

    // The directory is a convenience, not the source of truth for `boot_ws` — so a read failure is
    // logged by the caller via the returned set being short, never a boot abort.
    let rows = list(store, WORKSPACES_NS, TABLE, "kind", KIND)
        .await
        .unwrap_or_default();
    for row in rows {
        let Ok(rec) = serde_json::from_value::<WorkspaceRecord>(row) else {
            continue; // a row we cannot read is not a workspace we should start processes for.
        };
        if rec.status == WorkspaceStatus::Active {
            out.insert(rec.ws);
        }
    }
    Ok(out.into_iter().collect())
}
