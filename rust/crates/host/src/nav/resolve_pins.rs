//! Resolving the caller's PINNED refs (`nav_pref.pinned`) into rendered items — the pin half of
//! `nav.resolve`, in its own file so "what can a pin point at" has one owner (FILE-LAYOUT).
//!
//! There are now FOUR pin grammars, and they are tried in a deliberate order because two of them
//! overlap by shape:
//!
//!   1. a HOST-AUTHORED ext board row (`ext_boards_pin`) — tried FIRST, because `ext:<id>/<rowid>`
//!      is indistinguishable by shape from a declared destination and would otherwise strip;
//!   2. an `ext:<ext>/<navid>` DECLARED destination — resolved against the install's `[[ui.nav]]`;
//!   3. everything else — a bare surface key / `dashboard:<id>` / whole-`ext:<id>` — through the
//!      synthetic-item path and the ordinary `resolve_item` pipeline;
//!   4. nothing: a ref matching none of these strips SILENTLY.
//!
//! **A strip is silent and never destructive.** A pin the caller cannot reach, that no longer
//! exists, or that the admin hid (hide beats pin) simply does not render — the stored record is
//! untouched, so a later un-hide or re-grant restores it for free. That invariant is why every arm
//! here returns `Ok(None)` rather than an error: one stale favourite must never fault a whole menu.

use std::collections::BTreeSet;
use std::sync::Arc;

use lb_auth::Principal;

use super::error::NavError;
use super::ext_boards_pin::resolve_ext_board_pin;
use super::model::{NavItem, ResolvedItem};
use super::resolve::{label_or, resolve_item};
use super::store::read_pref;
use crate::boot::Node;
use crate::ext::ext_list;

/// Resolve the caller's pinned refs (`nav_pref.pinned`) to rendered items, in the member's order.
/// Each ref maps to a synthetic [`NavItem`] and runs through the SAME `resolve_item` pipeline as a
/// menu entry — so a pin the caller can't reach (cap), that no longer exists (deleted dashboard,
/// uninstalled ext), or that the admin hid (hide beats pin) strips silently. The stored record is
/// never mutated by a strip, so a later un-hide/regrant restores the pin for free.
pub(super) async fn resolve_pins(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    hidden: &BTreeSet<String>,
) -> Result<Vec<ResolvedItem>, NavError> {
    let pref = read_pref(&node.store, ws, principal.sub())
        .await?
        .unwrap_or_default();
    let mut pinned = Vec::new();
    for pin in &pref.pinned {
        if hidden.contains(pin) {
            continue; // hide beats pin — the admin's curation lever actually declutters.
        }
        // An `ext:<ext>/<navid>` pin targets one of the extension's DECLARED nav destinations
        // (ext-subref-pins scope), which the generic `resolve_item` pipeline has no kind for — it
        // resolves against the install's `[[ui.nav]]` list instead. Everything else keeps the
        // synthetic-item path unchanged.
        // A HOST-AUTHORED board row (host-authored-ext-nav-boards scope, Decision 2) resolves FIRST,
        // off the workspace record. Both of its ref shapes would otherwise strip silently below —
        // `ext:<id>/<rowid>` reads as a declared destination the manifest does not have, and
        // `ext:<id>/<navid>/<rowid>` is refused by `split_ext_subref` as a runtime published child.
        // Being resolvable WITHOUT a mount is exactly what makes a host row pinnable at all, so this
        // is the half that turns that claim into behaviour.
        if let Some(resolved) = resolve_ext_board_pin(node, principal, ws, pin).await? {
            pinned.push(resolved);
            continue;
        }
        if let Some((ext, nav)) = split_ext_subref(pin) {
            if let Some(resolved) = resolve_ext_nav(node, principal, ws, ext, nav).await? {
                pinned.push(resolved);
            }
            continue;
        }
        let item = pin_to_item(pin);
        if let Some(resolved) = resolve_item(node, principal, ws, &item).await? {
            pinned.push(resolved);
        }
    }
    Ok(pinned)
}

/// Split an `ext:<ext>/<navid>` pin ref into its two opaque segments, or `None` when the ref is any
/// other shape (a bare surface key, `dashboard:<id>`, a whole-extension `ext:<id>`).
///
/// A ref with MORE than one slash after the prefix is a runtime-published `bridge.setNav` child
/// (`ext:<ext>/<navid>/<childid>`). Those exist only while the extension is mounted and publishing,
/// so the server cannot resolve them and deliberately does NOT try: `None` here sends it down the
/// ordinary path, where the nonsense ext id matches no install and strips silently. Non-goal, not an
/// oversight (ext-subref-pins scope) — the shell drops the pin affordance on those rows to match.
fn split_ext_subref(pin: &str) -> Option<(&str, &str)> {
    let rest = pin.strip_prefix("ext:")?;
    let (ext, nav) = rest.split_once('/')?;
    if ext.is_empty() || nav.is_empty() || nav.contains('/') {
        return None;
    }
    Some((ext, nav))
}

/// Resolve one of an extension's DECLARED `[[ui.nav]]` destinations (ext-subref-pins scope). Two
/// opaque-string lookups, no id ever branched on (rule 10):
///   1. the install, through the generic `ext.list` discovery seam — uninstalled ⇒ stripped;
///   2. the declared nav item whose `id` matches — a manifest that no longer declares it ⇒ stripped.
///
/// The resulting KIND defers to what the destination itself declared. A destination carrying a
/// `dashboard` ref resolves to a **`dashboard`** item with its `vars`, so the pinned entry opens the
/// board var-bound exactly as clicking the sidebar row does — and it runs through the SAME
/// `resolve_dashboard` as any other dashboard item, so an unreadable board cap-strips the pin for
/// free. Anything else resolves to an **`ext`** item. Either way `nav` echoes the destination id, so
/// `item_ref` reconstructs the ref the caller pinned.
///
/// An `admin`-gated destination is NOT filtered here: that flag is presentation-only chrome the
/// extension owns (the verbs remain the wall), and the shell already applies it when rendering.
async fn resolve_ext_nav(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    ext: &str,
    nav: &str,
) -> Result<Option<ResolvedItem>, NavError> {
    // A caller who cannot even LIST extensions cannot reach this destination — strip the one pin
    // rather than faulting the whole menu. This is the pin path: a member who lacks `ext.list`
    // would otherwise get a blank sidebar on every resolve because of one stale favorite, which is
    // precisely the "a strip is silent, never a fault" invariant `resolve_pins` documents.
    let Ok(installed) = ext_list(node, principal, ws).await else {
        return Ok(None);
    };
    let Some(row) = installed.iter().find(|row| row.ext == ext) else {
        return Ok(None); // uninstalled → stripped silently, exactly like a whole-ext pin.
    };
    let Some(decl) = row
        .ui
        .as_ref()
        .and_then(|ui| ui.nav.iter().find(|n| n.id == nav))
    else {
        return Ok(None); // the extension no longer declares this destination → stripped.
    };
    // A destination bound to a host dashboard resolves AS that dashboard (ext-dashboard-nav
    // grammar), reusing the ordinary dashboard path so cap-strip/not-found behave identically.
    if let Some(dashboard) = decl.dashboard.as_ref().filter(|d| !d.is_empty()) {
        let item = NavItem {
            kind: "dashboard".into(),
            dashboard: dashboard.clone(),
            label: decl.label.clone(),
            icon: decl.icon.clone(),
            vars: decl.vars.clone(),
            // The extension's declared heading override rides the synthetic item so the ordinary
            // dashboard path relays it — one relay, not an ext-shaped second one (rule 10).
            title_template: decl.title_template.clone(),
            ..NavItem::default()
        };
        return Ok(resolve_item(node, principal, ws, &item)
            .await?
            .map(|mut r| {
                // Carry the ext identity so `item_ref` still reconstructs `ext:<ext>/<navid>` — the
                // entry OPENS a board but is PINNED as the ext destination it is.
                r.ext = ext.to_string();
                r.nav = nav.to_string();
                r
            }));
    }
    Ok(Some(ResolvedItem {
        kind: "ext".into(),
        label: label_or(&decl.label, &decl.id),
        icon: decl.icon.clone(),
        ext: ext.to_string(),
        nav: nav.to_string(),
        title_template: decl.title_template.clone(),
        ..ResolvedItem::default()
    }))
}

/// Map a pin ref to the synthetic authored item the resolver understands. `dashboard:<id>` and
/// `ext:<id>` select their kinds; anything else is a core surface key. All opaque data.
///
/// The `ext:<ext>/<navid>` sub-ref never reaches here — `resolve_pins` routes it to
/// `resolve_ext_nav` first (ext-subref-pins scope), because it resolves against the install's
/// declared nav list rather than mapping to a synthetic authored item.
fn pin_to_item(pin: &str) -> NavItem {
    if pin.starts_with("dashboard:") {
        NavItem {
            kind: "dashboard".into(),
            dashboard: pin.to_string(),
            ..NavItem::default()
        }
    } else if let Some(ext) = pin.strip_prefix("ext:") {
        NavItem {
            kind: "ext".into(),
            ext: ext.to_string(),
            ..NavItem::default()
        }
    } else {
        NavItem {
            kind: "surface".into(),
            surface: pin.to_string(),
            ..NavItem::default()
        }
    }
}
