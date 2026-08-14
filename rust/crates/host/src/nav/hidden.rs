//! `nav.hidden.get` / `nav.hidden.set` — the workspace hidden-set (hide-and-pins scope). One
//! admin-managed `nav_hidden:[ws]` record naming the item refs (bare surface key | `ext:<id>` |
//! `dashboard:<id>`, all opaque data — rule 10) the resolver subtracts from EVERY menu tier,
//! including the client-side `SURFACES` fallback (via the `ResolvedNav::hidden` echo).
//!
//! **Declutter, never authz.** Hiding an entry never blocks its route — `CoreGate` and the server's
//! per-verb re-checks are untouched; a permitted deep link still loads. Hide beats pin (the admin's
//! one curation lever must actually declutter), and un-hiding restores pins for free because the
//! resolver never mutates a member's `nav_pref` when it strips.
//!
//! The same record carries the workspace ORDERING (`nav.order.set`) — the admin's one ARRANGING
//! lever, over the identical opaque ref grammar. Subtraction and arrangement are independent: each
//! setter carries the sibling field through, so saving one never discards the other.
//!
//! Gates: the read rides `mcp:nav.resolve:call` (every member's resolve needs the set; the settings
//! tab reads it); the write rides `mcp:nav.save:call` — the same authoring privilege that shapes the
//! workspace's menus, exactly like `nav.set_default` (no separate cap for one pointer record).

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_nav;
use super::error::NavError;
use super::model::{NavHidden, MAX_HIDDEN, MAX_ORDER};
use super::store::{read_hidden, write_hidden};

/// Read the workspace hidden-set. Absent → an empty [`NavHidden`] (nothing hidden). Member-level
/// (rides `nav.resolve`).
pub async fn nav_hidden_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<NavHidden, NavError> {
    authorize_nav(principal, ws, "nav.resolve")?;
    Ok(read_hidden(store, ws).await?.unwrap_or_default())
}

/// Replace the workspace hidden-set (full-set LWW on the one `[ws]` record; empty clears it), at
/// logical time `now`. Bounded by [`MAX_HIDDEN`] (`BadInput` over — never silently truncated); an
/// empty/blank ref is rejected as malformed. Admin write — rides `mcp:nav.save:call`.
pub async fn nav_hidden_set(
    store: &Store,
    principal: &Principal,
    ws: &str,
    hidden: Vec<String>,
    now: u64,
) -> Result<NavHidden, NavError> {
    authorize_nav(principal, ws, "nav.save")?;
    if hidden.len() > MAX_HIDDEN {
        return Err(NavError::BadInput(format!(
            "hidden-set has {} refs, exceeds cap {MAX_HIDDEN}",
            hidden.len()
        )));
    }
    if hidden.iter().any(|r| r.trim().is_empty()) {
        return Err(NavError::BadInput("hidden ref must be non-empty".into()));
    }
    // Hidden and order are independent levers on one record: saving visibility must not silently
    // discard the workspace's ordering (and vice versa). Carry the sibling field through.
    let order = read_hidden(store, ws).await?.unwrap_or_default().order;
    let record = NavHidden {
        hidden,
        order,
        updated_ts: now,
    };
    write_hidden(store, ws, &record).await?;
    Ok(record)
}

/// Replace the workspace ORDERING (full-list LWW on the same `[ws]` record; empty clears it, meaning
/// natural order), at logical time `now`. Bounded by [`MAX_ORDER`] (`BadInput` over — never silently
/// truncated); an empty/blank ref is rejected as malformed, and a duplicated ref is rejected because
/// one ref cannot hold two positions. Admin write — rides `mcp:nav.save:call` like the hidden-set.
///
/// The list is a PARTIAL order: refs named here sort to their given positions, and every ref the
/// caller knows but this list does not keeps its natural order behind them. A stale ref is therefore
/// inert, not destructive — ordering never adds or removes an entry, it only arranges one.
pub async fn nav_order_set(
    store: &Store,
    principal: &Principal,
    ws: &str,
    order: Vec<String>,
    now: u64,
) -> Result<NavHidden, NavError> {
    authorize_nav(principal, ws, "nav.save")?;
    if order.len() > MAX_ORDER {
        return Err(NavError::BadInput(format!(
            "order has {} refs, exceeds cap {MAX_ORDER}",
            order.len()
        )));
    }
    if order.iter().any(|r| r.trim().is_empty()) {
        return Err(NavError::BadInput("order ref must be non-empty".into()));
    }
    let mut seen = std::collections::BTreeSet::new();
    if let Some(dup) = order.iter().find(|r| !seen.insert(r.trim())) {
        return Err(NavError::BadInput(format!(
            "order ref {dup:?} appears more than once"
        )));
    }
    let hidden = read_hidden(store, ws).await?.unwrap_or_default().hidden;
    let record = NavHidden {
        hidden,
        order,
        updated_ts: now,
    };
    write_hidden(store, ws, &record).await?;
    Ok(record)
}
