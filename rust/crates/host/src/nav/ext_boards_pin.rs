//! Resolving a PIN that points at a host-authored ext board row
//! (host-authored-ext-nav-boards scope, Decision 2 — "pinnable: yes").
//!
//! This is the half that makes that decision real rather than a claim. Host rows are pinnable
//! precisely because their refs are stable WITHOUT a mount — but `resolve_pins` could not resolve
//! them before this file, and a pin it cannot resolve **strips silently**:
//!
//! - `ext:<id>/<rowid>` (a section-root row) reads as the declared-destination grammar, so
//!   `resolve_ext_nav` searched the install's `[[ui.nav]]` list, found nothing, and stripped;
//! - `ext:<id>/<navid>/<rowid>` (a row under a declared item) has two slashes, which
//!   `split_ext_subref` deliberately refuses (a runtime `bridge.setNav` child the server can never
//!   resolve) — so it fell to the ordinary path, matched no install, and stripped.
//!
//! Both are correct for the cases they were written for and wrong for this one. The difference is
//! that a host row is HOST data: the server holds the record, so it CAN resolve the ref — which is
//! the whole structural argument for host rows over published children. So this runs FIRST, against
//! the `nav_ext_boards` record, and only a miss falls through to the shipped paths.
//!
//! **The `b:` marker on the last segment is what makes this unambiguous.** A host row's ref is
//! `<slot>/b:<rowid>`, never a bare `<slot>/<rowid>`. Without it, `ext:<id>/<rowid>` is
//! indistinguishable from a DECLARED destination — and since host rows resolve first, a row whose id
//! happened to match a declared nav id would silently SHADOW that destination. The marker also keeps
//! the shell's node keys distinct where a host row sits beside a published child of the same id;
//! identical keys there are a duplicate React key, not a harmless coincidence. `ext_boards.rs`
//! rejects a `:` in a row id so the marker can never be forged from one.
//!
//! Rule 10: the ref is split on its LAST `/` into an opaque slot key and an opaque row id, and both
//! are looked up as strings. Nothing here interprets an extension id, and both slot kinds resolve
//! through the one lookup rather than two shaped branches.

use std::sync::Arc;

use lb_auth::Principal;

use super::error::NavError;
use super::ext_boards_model::ExtBoardRow;
use super::model::{NavItem, ResolvedItem};
use super::resolve::resolve_item;
use super::store::read_ext_boards;
use crate::boot::Node;

/// The marker on a host row's last ref segment — see the module doc for why it exists.
pub(super) const ROW_MARKER: &str = "b:";

/// Split a pin ref into `(slot, row_id)`. The last `/`-segment must carry [`ROW_MARKER`]; everything
/// before it is the slot key, which must be the slot grammar (`ext:<id>` | `ext:<id>/<navid>`). Any
/// other shape — a bare surface key, `dashboard:<id>`, a whole-ext pin, or a DECLARED destination —
/// returns `None` and falls through to the shipped resolvers untouched.
fn split_row_ref(pin: &str) -> Option<(&str, &str)> {
    let (slot, last) = pin.rsplit_once('/')?;
    let row = last.strip_prefix(ROW_MARKER)?;
    if row.is_empty() || !slot.starts_with("ext:") || slot.len() == "ext:".len() {
        return None;
    }
    Some((slot, row))
}

/// Find the host-authored row a pin names, if the workspace record holds one.
async fn find_row(
    node: &Arc<Node>,
    ws: &str,
    slot: &str,
    row_id: &str,
) -> Result<Option<ExtBoardRow>, NavError> {
    let Some(record) = read_ext_boards(&node.store, ws).await? else {
        return Ok(None); // no admin ever bound a board here — the feature is inert.
    };
    Ok(record
        .slots
        .get(slot)
        .and_then(|rows| rows.iter().find(|r| r.id == row_id))
        .cloned())
}

/// Resolve a pin naming a host-authored ext board row, or `None` when it names none (the caller
/// then falls through to the shipped pin paths unchanged).
///
/// The row resolves AS the dashboard it points at, through the SAME `resolve_item` pipeline every
/// other dashboard item uses — so an unreadable or deleted board cap-strips the pin for free, and
/// the pinned entry opens the board var-bound exactly as clicking its rail row does. `ext`/`nav` are
/// then set so `item_ref` reconstructs the ref the caller pinned, in either slot grammar: `ext` is
/// the id after the `ext:` prefix and `nav` is everything after it, which for a section-root row is
/// just `<rowid>` and for an item row is `<navid>/<rowid>`.
///
/// **No authority is added.** A pin is a lens like any other nav row; this only lets the resolver
/// SAY what the ref points at. The board's own read gate is still what decides whether it survives.
pub async fn resolve_ext_board_pin(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    pin: &str,
) -> Result<Option<ResolvedItem>, NavError> {
    let Some((slot, row_id)) = split_row_ref(pin) else {
        return Ok(None);
    };
    let Some(row) = find_row(node, ws, slot, row_id).await? else {
        return Ok(None);
    };
    let item = NavItem {
        kind: "dashboard".into(),
        dashboard: row.dashboard.clone(),
        label: row.label.clone(),
        icon: row.icon.clone(),
        icon_color: row.icon_color.clone(),
        vars: row.vars.clone(),
        ..NavItem::default()
    };
    // `ext`/`nav` carry the ref VERBATIM (marker included) so `item_ref` reconstructs exactly what
    // the shell pinned — the round-trip has to be byte-stable or the pin lights no row.
    let ext_and_nav = pin.trim_start_matches("ext:");
    let Some((ext, nav)) = ext_and_nav.split_once('/') else {
        return Ok(None);
    };
    // The board's own read gate runs inside `resolve_item` under this principal — an unreadable or
    // deleted board strips the pin there, exactly as for any other dashboard item.
    Ok(resolve_item(node, principal, ws, &item)
        .await?
        .map(|mut r| {
            r.ext = ext.to_string();
            r.nav = nav.to_string();
            r
        }))
}

#[cfg(test)]
mod tests {
    use super::split_row_ref;

    /// Both slot grammars split into the SAME `(slot, row)` shape — one lookup, not two branches.
    #[test]
    fn both_slot_kinds_split_on_the_last_segment() {
        assert_eq!(split_row_ref("ext:alpha/b:iaq"), Some(("ext:alpha", "iaq")));
        assert_eq!(
            split_row_ref("ext:alpha/sites/b:iaq"),
            Some(("ext:alpha/sites", "iaq"))
        );
    }

    /// **The shadowing guard.** A DECLARED destination is `ext:<id>/<navid>` with no marker, and it
    /// must never read as a host row — host rows resolve FIRST, so without this a row whose id
    /// matched a declared nav id would silently take that destination's place. A published child
    /// (`ext:<id>/<navid>/<childid>`) is likewise not ours.
    #[test]
    fn a_declared_destination_is_never_mistaken_for_a_host_row() {
        assert_eq!(split_row_ref("ext:alpha/sites"), None);
        assert_eq!(split_row_ref("ext:alpha/sites/site-1"), None);
    }

    /// Refs that are not host-row refs never enter this path — they belong to the shipped resolvers.
    #[test]
    fn non_row_refs_are_declined() {
        for pin in [
            "dashboards",          // a bare surface key
            "dashboard:board-iaq", // a host dashboard pin
            "ext:alpha",           // a whole-extension pin (no row segment)
            "ext:/iaq",            // an empty ext id
            "ext:alpha/",          // an empty row id
            "group:Operations",    // a curated group heading
        ] {
            assert_eq!(
                split_row_ref(pin),
                None,
                "{pin} must not read as a host row"
            );
        }
    }
}
