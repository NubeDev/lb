//! `nav.resolve()` — the one composite read (nav scope, "A resolver verb"). Returns the caller's
//! **effective** menu: their active nav **picked** (personal pick → team-shared → workspace-default →
//! built-in `SURFACES` fallback), with **tag-group entries expanded** (via `tags.find`) and every
//! item the caller can't reach **already stripped**. The UI renders one payload and re-implements no
//! filtering.
//!
//! **The lens, not a grant (the whole design).** Resolve is a PURE FILTER over caps the caller
//! already holds — it never writes a cap, never widens reach. A `surface` item survives iff the caller
//! holds the surface's gate cap ([`surface_gate_cap`]); a `dashboard`/`tag-group` dashboard survives
//! iff the three-gate read passes ([`may_read_nav`]-style, via `nav_get`'s dashboard analog); an `ext`
//! item survives iff its opaque id is still installed (`ext.list`) — an uninstalled ext is stripped
//! silently, exactly like a cap-stripped item (nav scope, resolved open question). The server
//! re-checks every verb on click regardless, so a stale/over-eager nav can only *show a link that then
//! 403s* — never *grant* (the "nav never widens" headline test).
//!
//! Member-level: gated by `mcp:nav.resolve:call` (every member resolves their own menu).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use lb_auth::Principal;
use lb_store::Store;
use lb_tags::Facet;

use super::admin_lens::is_workspace_admin;
use super::authorize::authorize_nav;
use super::bounds::BUILTIN_PICK;
use super::error::NavError;
use super::model::{
    Nav, NavFacet, NavItem, ResolvedItem, ResolvedNav, ResolvedSource, Visibility, MAX_TAG_GROUP,
};
use super::resolve_template_group::resolve_template_group;
use super::store::{read_default, read_hidden, read_nav, read_pref, scan_navs};
use super::surfaces::surface_gate_cap;
use super::visibility::may_read_nav;
use crate::authz::holds_cap;
use crate::boot::Node;
use crate::dashboard::{dashboard_get, DashboardError};
use crate::ext::ext_list;
use crate::tags::tags_find;

/// Resolve `principal`'s effective menu in `ws`. Picks the nav (4-tier precedence), expands
/// tag-groups, and strips every unreachable item. A `Fallback` result carries no items — the UI
/// renders its built-in `SURFACES` (never a blank rail).
pub async fn nav_resolve(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
) -> Result<ResolvedNav, NavError> {
    authorize_nav(principal, ws, "nav.resolve")?;

    // The workspace hidden-set (hide-and-pins scope) — the THIRD strip filter, applied after
    // cap-strip and uninstalled-ext-strip, at EVERY tier. Echoed back so the UI can subtract the
    // one tier the server can't: its own built-in `SURFACES`/ext-slot fallback menu.
    let hidden_record = read_hidden(&node.store, ws).await?.unwrap_or_default();
    let hidden: BTreeSet<String> = hidden_record.hidden.iter().cloned().collect();

    // The caller's pins, resolved through the SAME item pipeline (cap-strip + ext-strip), then
    // hidden-stripped (hide beats pin). A stale/stripped pin never mutates the stored record.
    let pinned = resolve_pins(node, principal, ws, &hidden).await?;

    let (nav, source) = match pick_nav(&node.store, principal, ws).await? {
        Some(picked) => picked,
        // No nav applies — the caller renders its built-in fallback (never blank), minus `hidden`,
        // with `pinned` above it.
        None => {
            return Ok(ResolvedNav {
                source: ResolvedSource::Fallback,
                nav_id: String::new(),
                title: String::new(),
                items: Vec::new(),
                hidden: hidden_record.hidden,
                pinned,
            })
        }
    };

    let mut items = Vec::new();
    for item in &nav.items {
        if let Some(resolved) = resolve_item(node, principal, ws, item).await? {
            if let Some(kept) = strip_hidden(resolved, &hidden) {
                items.push(kept);
            }
        }
    }

    Ok(ResolvedNav {
        source,
        nav_id: nav.id.clone(),
        title: nav.title.clone(),
        items,
        hidden: hidden_record.hidden,
        pinned,
    })
}

/// The hidden-set filter (hide-and-pins scope) — drop a resolved item whose ref the admin hid, and
/// recurse into a `group`'s children at EVERY depth (nested-nav scope). Pruning is **post-order**: a
/// group whose descendants all get hidden-stripped disappears too (never a folder that expands to
/// nothing — the same invariant `resolve_group` enforces for cap-strip). A group with ≥1 surviving
/// descendant stays. The ref grammar mirrors [`item_ref`]: bare surface key, `ext:<id>`,
/// `dashboard:<id>` — all matched as opaque strings (rule 10).
fn strip_hidden(item: ResolvedItem, hidden: &BTreeSet<String>) -> Option<ResolvedItem> {
    if hidden.is_empty() {
        return Some(item);
    }
    if item.kind == "group" {
        let mut kept = item;
        kept.items = kept
            .items
            .into_iter()
            .filter_map(|c| strip_hidden(c, hidden))
            .collect();
        // Post-order prune: a group left empty by hidden-strip vanishes, at any depth.
        if kept.items.is_empty() {
            return None;
        }
        return Some(kept);
    }
    if hidden.contains(&item_ref(&item)) {
        return None; // hidden — declutter only; the route stays reachable by deep link.
    }
    Some(item)
}

/// A resolved item's ref in the shared hide/pin grammar. An item that resolved to one of an
/// extension's DECLARED `[[ui.nav]]` destinations carries that destination in `nav`, and its ref is
/// the sub-ref `ext:<ext>/<navid>` — the same string the shell pinned (ext-subref-pins scope). This
/// is what lets the hidden-set target ONE extension destination, and what makes a sub-ref pin light
/// its own row instead of every row of that extension. A destination carrying a `dashboard` keeps
/// its ext identity here (it resolved as a `dashboard`-kind item so it OPENS the board, but it is
/// still pinned/hidden as the ext destination it is) — so the round-trip is stable in both grammars.
fn item_ref(item: &ResolvedItem) -> String {
    if !item.ext.is_empty() && !item.nav.is_empty() {
        return format!("ext:{}/{}", item.ext, item.nav);
    }
    match item.kind.as_str() {
        "ext" => format!("ext:{}", item.ext),
        "dashboard" => item.dashboard.clone(),
        _ => item.surface.clone(),
    }
}

/// Resolve the caller's pinned refs (`nav_pref.pinned`) to rendered items, in the member's order.
/// Each ref maps to a synthetic [`NavItem`] and runs through the SAME `resolve_item` pipeline as a
/// menu entry — so a pin the caller can't reach (cap), that no longer exists (deleted dashboard,
/// uninstalled ext), or that the admin hid (hide beats pin) strips silently. The stored record is
/// never mutated by a strip, so a later un-hide/regrant restores the pin for free.
async fn resolve_pins(
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

/// The 4-tier pick: personal pick → first team-shared nav → workspace-default → `None` (fallback).
/// A pick/default pointing at a deleted/unreadable nav falls through to the next tier (nav scope,
/// "Stale pick").
async fn pick_nav(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Option<(Nav, ResolvedSource)>, NavError> {
    // Tier 1 — the member's personal pick. Only if it still resolves + is still readable.
    // The reserved `__builtin__` sentinel (no-lockout scope) is an EXPLICIT "force the built-in
    // sidebar" pick — return `None` immediately so tiers 2/3 are skipped and the caller renders its
    // fallback rail. This is the escape hatch: anyone handed a too-narrow nav can bail to all the pages
    // they can reach, via their own member-owned `nav.pref.set`.
    if let Some(pref) = read_pref(store, ws, principal.sub()).await? {
        if pref.active == BUILTIN_PICK {
            return Ok(None);
        }
        if !pref.active.is_empty() {
            if let Some(nav) = readable_nav(store, principal, ws, &pref.active).await? {
                return Ok(Some((nav, ResolvedSource::Pick)));
            }
        }
    }

    // No-lockout (nav-no-lockout scope): the auto-apply tiers (team share / workspace default) NEVER
    // narrow a workspace admin. A curated nav shapes a MEMBER's menu; it must not silently replace an
    // administrator's console (a team-shared 1-page nav, or any workspace default, would otherwise
    // subtract the whole admin console from the rail with no in-app way back). An admin is narrowed
    // ONLY by their own explicit tier-1 pick above; here they fall straight through to the built-in
    // fallback. Members are unaffected — tiers 2/3 still apply to them.
    if is_workspace_admin(principal, ws) {
        return Ok(None);
    }

    // Tier 2 — the first team-shared nav readable by the caller (deterministic: id-ordered scan).
    // A `team`-visible nav the caller is a member-of-a-shared-team for is a candidate.
    let all = scan_navs(store, ws).await?;
    for nav in &all {
        if nav.deleted || nav.visibility != Visibility::Team {
            continue;
        }
        if may_read_nav(store, principal, ws, nav).await.is_ok() {
            return Ok(Some((nav.clone(), ResolvedSource::Team)));
        }
    }

    // Tier 3 — the workspace-default pointer, if set + readable.
    if let Some(default_id) = read_default(store, ws).await? {
        if let Some(nav) = readable_nav(store, principal, ws, &default_id).await? {
            return Ok(Some((nav, ResolvedSource::WorkspaceDefault)));
        }
    }

    // Tier 4 — no nav applies; the caller falls back to built-in `SURFACES`.
    Ok(None)
}

/// Read nav `id` and return it only if present, not tombstoned, AND readable by the caller (gate 3).
/// Any miss returns `None` (the fall-through the pick tiers rely on) rather than erroring.
async fn readable_nav(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<Option<Nav>, NavError> {
    // A bare `nav:{id}` or plain `{id}` both address the same record (the pick may store either).
    let key = id.strip_prefix("nav:").unwrap_or(id);
    match read_nav(store, ws, key).await? {
        Some(nav) if !nav.deleted && may_read_nav(store, principal, ws, &nav).await.is_ok() => {
            Ok(Some(nav))
        }
        _ => Ok(None),
    }
}

/// Resolve one item to its rendered form, or `None` if the caller can't reach it (the strip). A
/// `tag-group` expands to a `group` of readable dashboards; a `group` recurses to any depth and prunes
/// empty subtrees post-order (nested-nav scope); every other kind maps 1:1 iff reachable.
async fn resolve_item(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    item: &NavItem,
) -> Result<Option<ResolvedItem>, NavError> {
    match item.kind.as_str() {
        "surface" => Ok(resolve_surface(principal, ws, item)),
        "dashboard" => resolve_dashboard(node, principal, ws, item).await,
        "ext" => resolve_ext(node, principal, ws, item).await,
        "tag-group" => resolve_tag_group(node, principal, ws, item).await,
        // reusable-pages scope: ONE dashboard fanned out per option value (`?var-<var>=<value>`).
        // Depth 0 — the outermost resolve entry; the query option source re-enters at depth+1.
        "template-group" => resolve_template_group(node, principal, ws, item, 0).await,
        "group" => resolve_group(node, principal, ws, item).await,
        // Unknown kind — drop it (defensive; `nav.save` bounds already reject unknown kinds).
        _ => Ok(None),
    }
}

/// A `surface` item survives iff the caller holds its gate cap (the mirror of `allowedSurfaces`). The
/// label defaults to the surface key when the author left it empty.
fn resolve_surface(principal: &Principal, ws: &str, item: &NavItem) -> Option<ResolvedItem> {
    if let Some(cap) = surface_gate_cap(&item.surface) {
        if !holds_cap(principal, ws, cap) {
            return None; // stripped — the caller can't reach this page (the lens).
        }
    }
    Some(ResolvedItem {
        kind: "surface".into(),
        label: label_or(&item.label, &item.surface),
        icon: item.icon.clone(),
        icon_color: item.icon_color.clone(),
        surface: item.surface.clone(),
        dashboard: String::new(),
        ext: String::new(),
        nav: String::new(),
        items: Vec::new(),
        vars: BTreeMap::new(),
        title_template: item.title_template.clone(),
    })
}

/// A `dashboard` item survives iff the three-gate dashboard read passes (`dashboard.get`). A denied /
/// absent dashboard is stripped silently (the lens); anything else is a real store error.
async fn resolve_dashboard(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    item: &NavItem,
) -> Result<Option<ResolvedItem>, NavError> {
    let id = item
        .dashboard
        .strip_prefix("dashboard:")
        .unwrap_or(&item.dashboard);
    if id.is_empty() {
        return Ok(None);
    }
    match dashboard_get(&node.store, principal, ws, id).await {
        Ok(d) => Ok(Some(ResolvedItem {
            kind: "dashboard".into(),
            label: label_or(&item.label, &d.title),
            icon: item.icon.clone(),
            icon_color: item.icon_color.clone(),
            surface: String::new(),
            dashboard: format!("dashboard:{id}"),
            ext: String::new(),
            nav: String::new(),
            items: Vec::new(),
            // reusable-pages scope: a pinned binding rides through to the href as `?var-<name>=…`.
            vars: item.vars.clone(),
            // nav-context-builtins scope: the heading override rides through beside the binding,
            // verbatim — the client interpolates it, the host expands nothing.
            title_template: item.title_template.clone(),
        })),
        // Denied / not-found → stripped (the caller can't read it). Any other is a real fault.
        // (`ManagedDenied` is a WRITE refusal — a read never produces it — but it IS a denial, so it
        // strips the nav item exactly like the opaque one rather than faulting the whole menu.)
        Err(DashboardError::Denied)
        | Err(DashboardError::ManagedDenied(_))
        | Err(DashboardError::NotFound) => Ok(None),
        Err(DashboardError::Store(e)) => Err(NavError::Store(e)),
        Err(DashboardError::BadInput(m)) => Err(NavError::BadInput(m)),
    }
}

/// An `ext` item survives iff its opaque id is still installed (`ext.list`). An uninstalled extension
/// is stripped silently, exactly like a cap-stripped item (nav scope, resolved open question). The id
/// is treated as OPAQUE data — never branched on (rule 10).
async fn resolve_ext(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    item: &NavItem,
) -> Result<Option<ResolvedItem>, NavError> {
    if item.ext.is_empty() {
        return Ok(None);
    }
    // `ext.list` is the generic discovery seam — we compare ids as opaque strings, no special-casing.
    let installed = ext_list(node, principal, ws)
        .await
        .map_err(|_| NavError::Denied)?;
    let found = installed.iter().find(|row| row.ext == item.ext);
    match found {
        Some(row) => Ok(Some(ResolvedItem {
            kind: "ext".into(),
            icon: item.icon.clone(),
            icon_color: item.icon_color.clone(),
            // The extension's own declared label (via `ext.list`) when the author left it empty,
            // falling back to the opaque id itself.
            label: label_or(
                &item.label,
                row.ui
                    .as_ref()
                    .map(|u| u.label.as_str())
                    .unwrap_or(&row.ext),
            ),
            surface: String::new(),
            dashboard: String::new(),
            ext: item.ext.clone(),
            nav: String::new(),
            items: Vec::new(),
            vars: BTreeMap::new(),
            title_template: item.title_template.clone(),
        })),
        None => Ok(None), // uninstalled → stripped silently.
    }
}

/// A `tag-group` expands to a `group` of the dashboards matching ALL its facets (via `tags.find`),
/// each filtered to what the caller can read (a dashboard the caller lacks is dropped). Bounded by
/// [`MAX_TAG_GROUP`]. An empty result yields an empty group (still rendered as a header).
async fn resolve_tag_group(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    item: &NavItem,
) -> Result<Option<ResolvedItem>, NavError> {
    let facets = to_facets(&item.facets);
    if facets.is_empty() {
        return Ok(None); // a tag-group must constrain something.
    }
    let hits = tags_find(&node.store, principal, ws, &facets)
        .await
        .map_err(|_| NavError::Denied)?;

    let mut children = Vec::new();
    for entity in &hits {
        if children.len() >= MAX_TAG_GROUP {
            break;
        }
        // Only dashboard entities become nav items (`dashboard:{id}` references). Other tagged
        // entities (series, channels) are not menu pages — skipped.
        let id = match entity.strip_prefix("dashboard:") {
            Some(id) => id,
            None => continue,
        };
        // Reachability: only surface a dashboard the caller can actually read (the tag-group lens).
        if let Ok(d) = dashboard_get(&node.store, principal, ws, id).await {
            children.push(ResolvedItem {
                kind: "dashboard".into(),
                label: d.title.clone(),
                icon: String::new(),
                // A dynamically expanded child has no authored icon of its own, but it INHERITS the
                // group's color so the whole fan-out reads as one branch rather than as uncolored
                // strays under a colored parent.
                icon_color: item.icon_color.clone(),
                surface: String::new(),
                dashboard: format!("dashboard:{id}"),
                ext: String::new(),
                nav: String::new(),
                items: Vec::new(),
                vars: BTreeMap::new(),
                // A tag-group expands to MANY distinct boards, each with its own stored heading —
                // unlike a template-group's one-board-many-bindings fan-out, so the group's override
                // does not descend onto a child that is a different record entirely.
                title_template: None,
            });
        }
    }

    Ok(Some(ResolvedItem {
        kind: "group".into(),
        label: label_or(&item.label, "Tagged"),
        icon: item.icon.clone(),
        icon_color: item.icon_color.clone(),
        surface: String::new(),
        dashboard: String::new(),
        ext: String::new(),
        nav: String::new(),
        items: children,
        vars: BTreeMap::new(),
        title_template: item.title_template.clone(),
    }))
}

/// A `group` recurses to ANY depth (nested-nav scope, capped at save by `MAX_GROUP_DEPTH`): resolve
/// each child through the same `resolve_item` pipeline — nested groups included — then **prune empty
/// groups post-order**. Children are resolved first; this group is then dropped (`None`) iff its
/// resolved `items[]` is empty. A group with ≥1 surviving descendant, even one several levels down,
/// stays: because pruning runs post-order, a deep survivor keeps every ancestor group alive. This is
/// what stops a permitted user from ever seeing a folder that expands to nothing.
async fn resolve_group(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    item: &NavItem,
) -> Result<Option<ResolvedItem>, NavError> {
    let mut children = Vec::new();
    for child in &item.items {
        // Recurse uniformly — a nested `group` returns `None` here iff its own subtree is empty, so
        // an empty inner folder never contributes a survivor to this group (the post-order prune).
        if let Some(resolved) = Box::pin(resolve_item(node, principal, ws, child)).await? {
            children.push(resolved);
        }
    }
    // Post-order prune: an author `group` that resolves to nothing the caller can reach disappears
    // entirely (never a folder that expands to nothing). A tag-group/template-group expansion keeps
    // its own semantics (it resolves via its own kind, not here).
    if children.is_empty() {
        return Ok(None);
    }
    Ok(Some(ResolvedItem {
        kind: "group".into(),
        label: label_or(&item.label, "Group"),
        icon: item.icon.clone(),
        icon_color: item.icon_color.clone(),
        surface: String::new(),
        dashboard: String::new(),
        ext: String::new(),
        nav: String::new(),
        items: children,
        vars: BTreeMap::new(),
        title_template: item.title_template.clone(),
    }))
}

/// Map the wire `NavFacet`s to `tags::Facet`s (value present → exact; absent → key-only).
fn to_facets(facets: &[NavFacet]) -> Vec<Facet> {
    facets
        .iter()
        .filter(|f| !f.key.is_empty())
        .map(|f| match &f.value {
            Some(v) => Facet::exact(&f.key, v.clone()),
            None => Facet::key_only(&f.key),
        })
        .collect()
}

/// The author label, or a fallback derived from the target when the author left it empty.
fn label_or(label: &str, fallback: &str) -> String {
    if label.is_empty() {
        fallback.to_string()
    } else {
        label.to_string()
    }
}
