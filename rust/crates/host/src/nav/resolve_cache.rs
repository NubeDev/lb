//! ONE resolve, one read per thing read — the per-request memo `nav.resolve` threads through its
//! walk.
//!
//! A menu is a tree of references, and references repeat: an estate's twenty site folders point at
//! the same "Energy" board, every `ext` item asks for the same installed-extension list, and every
//! pin re-reads the same records. Nothing remembered anything within a request, so the cost grew with
//! the number of ITEMS rather than with the number of distinct things they name. Measured on
//! rubix-ai's `esr` workspace: a 48-item menu made 44 dashboard reads for 6 distinct boards, plus one
//! full `install` table scan per ext item, to produce an 11.8 KB response.
//!
//! Scope and lifetime are the whole safety argument: this is created inside `nav_resolve`, borrowed
//! by its walk, and dropped when it returns. It is NEVER shared between requests or principals, so a
//! cached verdict cannot outlive the caller it was computed for, and there is nothing to invalidate —
//! the next request starts empty. Any cross-request cache would have to key on the principal, the
//! workspace AND every record it touched, and would have to be dropped on writes it cannot see; that
//! is a different feature with a different risk, and this is deliberately not it.
//!
//! A `Mutex`, not a `RefCell`: a resolve future is spawned onto the runtime (the reactors and the
//! gateway both `tokio::spawn` work that reaches here), so everything it holds must be `Send`. There
//! is no contention to speak of — one task owns this for the length of one request — and every lock
//! is taken, read or written, and RELEASED before the next `await`. Holding one across an await is
//! the deadlock in waiting; the scopes below exist to make that impossible to do by accident.
//!
//! One responsibility: remember what this resolve has already read.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use lb_auth::Principal;

use crate::dashboard::{dashboard_head, DashboardHead};
use crate::ext::{ext_list, ExtRow};
use crate::Node;

#[derive(Default)]
pub(super) struct ResolveCache {
    /// dashboard id → its gated heading, or `None` for "this caller cannot read it". The negative
    /// answer is cached too: an unreadable board named by twelve menu items must cost one denial, not
    /// twelve.
    dashboards: Mutex<HashMap<String, Option<DashboardHead>>>,
    /// The installed extensions, read at most once. `None` when the read failed — retrying it per item
    /// would just repeat the failure.
    exts: Mutex<Option<Option<Arc<Vec<ExtRow>>>>>,
}

impl ResolveCache {
    pub(super) fn new() -> Self {
        Self::default()
    }

    /// The heading of dashboard `id`, or `None` when the caller cannot read it (absent, denied,
    /// tombstoned — one answer, as the resolver's lens already treats them).
    pub(super) async fn dashboard(
        &self,
        node: &Arc<Node>,
        principal: &Principal,
        ws: &str,
        id: &str,
    ) -> Option<DashboardHead> {
        // Scoped: the guard is dropped before the await below, never held across it.
        if let Some(hit) = self.dashboards.lock().ok().and_then(|m| m.get(id).cloned()) {
            return hit;
        }
        let head = dashboard_head(&node.store, principal, ws, id).await.ok();
        if let Ok(mut map) = self.dashboards.lock() {
            map.insert(id.to_string(), head.clone());
        }
        head
    }

    /// The installed extensions for this workspace, read once per resolve. `None` when the read
    /// failed — every caller in the walk treats that as "no extensions", exactly as before.
    pub(super) async fn ext_list(
        &self,
        node: &Arc<Node>,
        principal: &Principal,
        ws: &str,
    ) -> Option<Arc<Vec<ExtRow>>> {
        if let Some(hit) = self.exts.lock().ok().and_then(|slot| slot.clone()) {
            return hit;
        }
        let rows = ext_list(node, principal, ws).await.ok().map(Arc::new);
        if let Ok(mut slot) = self.exts.lock() {
            *slot = Some(rows.clone());
        }
        rows
    }
}
