//! `dashboard.access_check` — the read-only dependency-closure **preflight** (access-model scope).
//! "Assign a user to a dashboard" is not a primitive and does not make the dashboard *work*: a
//! dashboard is a composite with a transitive closure — panels, datasources, the query verbs, the
//! per-endpoint connect caps, and required variables — and access is real only when the *whole
//! closure* resolves for the assignee. A live session proved it: `user:bob` held the dashboard
//! record + every cap yet still hit a private `panel:aidan` (403) and a missing datasource. This
//! verb walks that closure for a subject/team and returns a per-dependency verdict so "assigned"
//! provably means "renders". It **grants nothing** — it is the "will it work?" answer.
//!
//! **No preflight/live divergence (the cardinal sin).** The verdict must match what a live query by
//! that subject actually does. So we do NOT reimplement any predicate: we build a synthetic
//! [`Principal`] carrying the subject's REAL resolved caps ([`resolve_caps`]) and feed it through the
//! SAME gate-3 visibility fns the live routes use ([`may_read_dashboard`], [`may_read_panel`]), the
//! SAME `authorize_tool` cap gate every dispatch runs, and the SAME [`enforce_endpoint`] net check
//! `federation.query` runs pre-connect. If the preflight says green, the live call passes; if red, it
//! 403s — one source of truth.
//!
//! **Depth (v1).** Covers dashboard + panels + direct cell/panel sources + the `federation.query`
//! endpoint cap + saved-query (`query:<id>`) underlying-tool/datasource + required variables. Deeper
//! hops (panel→panel, a saved-query whose datasource fans further) are marked `unchecked` — NEVER
//! silently green (under-scoping the closure gives false-green, the worst outcome). Cycles are
//! detected via a visited-set over `panel:`/`query:` ids.

use std::collections::BTreeSet;

use lb_auth::Principal;
use lb_authz::Subject;
use lb_mcp::authorize_tool;
use lb_store::Store;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::authorize::authorize_dashboard;
use super::error::DashboardError;
use super::model::{Cell, Target, Variable};
use super::store::read_dashboard;
use super::visibility::may_read_dashboard;
use crate::authz::{resolve_caps_live, resolve_subject_caps_live};
use crate::federation::{enforce_endpoint, resolve_datasource};
use crate::panel::{may_read_panel, Panel};
use crate::query::{resolve_query, QueryTarget};

/// The kind of a dependency in the closure — so the UI can group and a test can assert simply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DepKind {
    /// The dashboard record itself (gate-3 read).
    Dashboard,
    /// A referenced library panel (`panel:<id>`, gate-3 read).
    Panel,
    /// A cell/panel data source — the `mcp:<tool>:call` cap for a `sources[].tool`.
    SourceCap,
    /// A registered datasource record (`federation.query` target) — must exist + be resolvable.
    Datasource,
    /// The `net:tls:<host>:<port>:connect` endpoint cap a datasource needs (federation install grant).
    Endpoint,
    /// A saved query (`query:<id>`) — its underlying verb + target cap.
    SavedQuery,
    /// A required (page-parameter) variable — must be bindable.
    Variable,
}

/// One dependency's verdict. `ok=false` names the exact missing grant/share/record in `reason`;
/// `unchecked=true` marks a hop v1 does not walk (deeper closure) — reported, never silently green.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DepVerdict {
    /// The dependency id (`dashboard:site-health`, `panel:site-map`, `mcp:federation.query:call`,
    /// `datasource:plant-telemetry`, `net:tls:10.0.0.5:5432:connect`, `query:q1`, `var:site`).
    pub dep: String,
    pub kind: DepKind,
    /// True iff the subject can resolve this dependency (green). False = a real gap.
    pub ok: bool,
    /// A hop v1 does not walk (a deeper panel→panel or query fan-out) — `ok` is false and this is
    /// true; the report says "unchecked here", distinct from a resolved "not permitted".
    #[serde(default)]
    pub unchecked: bool,
    /// The cell (`Cell.i`) this dependency belongs to, so the UI can group; empty for the dashboard
    /// record itself and workspace-level deps.
    #[serde(default)]
    pub cell: String,
    /// A one-line, non-secret explanation — the missing cap/share/record, or "reachable here".
    pub reason: String,
}

impl DepVerdict {
    fn ok(dep: impl Into<String>, kind: DepKind, cell: &str, reason: &str) -> Self {
        Self {
            dep: dep.into(),
            kind,
            ok: true,
            unchecked: false,
            cell: cell.to_string(),
            reason: reason.to_string(),
        }
    }
    fn bad(dep: impl Into<String>, kind: DepKind, cell: &str, reason: &str) -> Self {
        Self {
            dep: dep.into(),
            kind,
            ok: false,
            unchecked: false,
            cell: cell.to_string(),
            reason: reason.to_string(),
        }
    }
    fn unchecked(dep: impl Into<String>, kind: DepKind, cell: &str, reason: &str) -> Self {
        Self {
            dep: dep.into(),
            kind,
            ok: false,
            unchecked: true,
            cell: cell.to_string(),
            reason: reason.to_string(),
        }
    }
}

/// The whole preflight report. `ok` is the AND of every verdict's `ok` (an `unchecked` hop keeps the
/// overall answer honest: not-all-green). Flat list + a `cell` field per the scope's recommended shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccessReport {
    pub dashboard: String,
    /// The subject the closure was resolved for (`user:bob` / `team:ops`).
    pub subject: String,
    /// True iff every dependency is `ok` (no gap, nothing unchecked) — "the page will render".
    pub ok: bool,
    pub dependencies: Vec<DepVerdict>,
}

/// Walk `dashboard`'s cell dependency closure for `subject` in `ws` and report a per-dependency
/// verdict. `principal` is the CALLER (the admin/member running the preflight): reporting another
/// subject's effective caps is admin-ish, so a preflight for a *team* requires
/// `mcp:authz.resolve:call`; a self-preflight (subject == caller) is member-level. Grants nothing.
///
/// The verb itself is gated by `mcp:dashboard.access_check:call` (member — it reads a dashboard) and,
/// for a foreign subject, additionally `mcp:authz.resolve:call` (admin). It reuses `dashboard.get`'s
/// gate-1+2 to read the dashboard as the caller (no existence leak to an ungranted caller).
pub async fn dashboard_access_check(
    store: &Store,
    principal: &Principal,
    ws: &str,
    dashboard_id: &str,
    subject: &Subject,
) -> Result<AccessReport, DashboardError> {
    // Gate 1+2 as the CALLER: workspace + `mcp:dashboard.access_check:call`. A ws-B caller or one
    // without the cap is denied before any read (no existence signal).
    authorize_dashboard(principal, ws, "dashboard.access_check")?;

    // Reporting ANOTHER subject's effective caps is the `authz.resolve` authority (access-console
    // scope) — a self-preflight ("why is MY tile broken?") is member-level, a preflight for a team or
    // another user is admin. Same gate the access console uses; no parallel rule.
    if subject.as_key() != principal.owner_sub() && subject.as_key() != principal.sub() {
        authorize_tool(principal, ws, "authz.resolve").map_err(|_| DashboardError::Denied)?;
    }

    // Resolve the SUBJECT's real effective caps and build a synthetic principal that carries them —
    // this is what makes the preflight match live behavior (same caps, same predicates below).
    let subject_caps = resolve_subject_effective_caps(store, ws, subject).await?;
    let subject_principal = Principal::routed(subject.as_key(), ws, subject_caps);

    let mut deps: Vec<DepVerdict> = Vec::new();
    // Track visited composite ids so a panel/query cycle terminates (bounded closure).
    let mut visited: BTreeSet<String> = BTreeSet::new();

    // The dashboard record: read it (it must exist) and run the SAME gate-3 the live route runs, for
    // the subject. A tombstoned/absent dashboard is a hard NotFound to the caller.
    let dashboard = read_dashboard(store, ws, dashboard_id)
        .await?
        .filter(|d| !d.deleted)
        .ok_or(DashboardError::NotFound)?;
    let dash_dep = format!("dashboard:{dashboard_id}");
    match may_read_dashboard(store, &subject_principal, ws, &dashboard).await {
        Ok(()) => deps.push(DepVerdict::ok(
            &dash_dep,
            DepKind::Dashboard,
            "",
            "shared/visible to the subject",
        )),
        Err(_) => deps.push(DepVerdict::bad(
            &dash_dep,
            DepKind::Dashboard,
            "",
            "not shared to the subject (private/unshared)",
        )),
    }

    // Each cell contributes: its panel ref (gate-3 + the panel's own sources), its direct sources
    // (cap + datasource + endpoint), and any saved-query references.
    for cell in &dashboard.cells {
        check_cell(store, &subject_principal, ws, cell, &mut deps, &mut visited).await?;
    }

    // Required (page-parameter) variables: each must be bindable, or the page renders the "select a
    // <label>" gate instead of firing cells. v1 checks a bindable SOURCE exists; deeper per-value
    // validation is deferred (marked below).
    for var in &dashboard.variables {
        if var.required {
            deps.push(check_required_var(&subject_principal, ws, var));
        }
    }

    let ok = deps.iter().all(|d| d.ok);
    Ok(AccessReport {
        dashboard: dashboard_id.to_string(),
        subject: subject.as_key(),
        ok,
        dependencies: deps,
    })
}

/// Resolve a subject's effective caps: a `user:` folds the full session projection (direct ∪ roles ∪
/// team-inherited) via [`resolve_caps_live`]; any other subject (team/role/key) folds its own direct
/// grants + roles via [`resolve_subject_caps_live`] — the SAME split `authz.resolve` uses, so the caps
/// the preflight tests are exactly the caps a live token would carry. Both bake in the live built-in
/// bundles (builtin-role-freshness) so the preflight matches the mint.
async fn resolve_subject_effective_caps(
    store: &Store,
    ws: &str,
    subject: &Subject,
) -> Result<Vec<String>, DashboardError> {
    match subject {
        Subject::User(user) => Ok(resolve_caps_live(store, ws, user).await?),
        other => {
            let mut caps = BTreeSet::new();
            resolve_subject_caps_live(store, ws, other, &mut caps).await?;
            Ok(caps.into_iter().collect())
        }
    }
}

/// Walk one cell's dependencies. A `panel_ref` cell: gate-3 the panel for the subject, then (one hop)
/// walk the panel's own `sources[]`. An inline cell: walk its `sources[]` directly. Saved-query
/// (`query:<id>`) targets recurse with cycle detection.
async fn check_cell(
    store: &Store,
    subject: &Principal,
    ws: &str,
    cell: &Cell,
    deps: &mut Vec<DepVerdict>,
    visited: &mut BTreeSet<String>,
) -> Result<(), DashboardError> {
    if !cell.panel_ref.is_empty() {
        // The referenced panel — gate-3 read for the subject (reusing the live `may_read_panel`).
        let panel_id = cell.panel_ref.trim_start_matches("panel:");
        if !visited.insert(format!("panel:{panel_id}")) {
            deps.push(DepVerdict::unchecked(
                &cell.panel_ref,
                DepKind::Panel,
                &cell.i,
                "cycle — already walked this panel",
            ));
            return Ok(());
        }
        match read_panel_record(store, ws, panel_id).await? {
            None => deps.push(DepVerdict::bad(
                &cell.panel_ref,
                DepKind::Panel,
                &cell.i,
                "panel not found (deleted or never existed)",
            )),
            Some(panel) => {
                match may_read_panel(store, subject, ws, &panel).await {
                    Ok(()) => {
                        deps.push(DepVerdict::ok(
                            &cell.panel_ref,
                            DepKind::Panel,
                            &cell.i,
                            "shared/visible to the subject",
                        ));
                        // One hop into the panel's own sources (the panel is a lens; its sources
                        // re-check under the subject's caps at render).
                        for target in &panel.spec.sources {
                            check_target(store, subject, ws, target, &cell.i, deps, visited)
                                .await?;
                        }
                    }
                    Err(_) => deps.push(DepVerdict::bad(
                        &cell.panel_ref,
                        DepKind::Panel,
                        &cell.i,
                        "not shared to the subject (private/unshared)",
                    )),
                }
            }
        }
        return Ok(());
    }

    // Inline cell: walk its direct sources (v3 `sources[]`, falling back to the v2 single `source`).
    if cell.sources.is_empty() && !cell.source.tool.is_empty() {
        let target = Target {
            tool: cell.source.tool.clone(),
            args: cell.source.args.clone(),
            ..Default::default()
        };
        check_target(store, subject, ws, &target, &cell.i, deps, visited).await?;
    }
    for target in &cell.sources {
        check_target(store, subject, ws, target, &cell.i, deps, visited).await?;
    }
    Ok(())
}

/// Check one data-source target: the `mcp:<tool>:call` cap (the SAME `authorize_tool` gate the live
/// dispatch runs), plus — for a `federation.query` target — the datasource record + its `net:`
/// endpoint cap, and — for a `query.run` target naming `query:<id>` — the saved query's underlying
/// verb + datasource. A hidden target is skipped (Grafana parity — it fires no query).
async fn check_target(
    store: &Store,
    subject: &Principal,
    ws: &str,
    target: &Target,
    cell: &str,
    deps: &mut Vec<DepVerdict>,
    visited: &mut BTreeSet<String>,
) -> Result<(), DashboardError> {
    if target.hide || target.tool.is_empty() {
        return Ok(());
    }
    let cap = format!("mcp:{}:call", target.tool);
    let cap_ok = authorize_tool(subject, ws, &target.tool).is_ok();
    deps.push(if cap_ok {
        DepVerdict::ok(
            &cap,
            DepKind::SourceCap,
            cell,
            "the subject holds the source cap",
        )
    } else {
        DepVerdict::bad(
            &cap,
            DepKind::SourceCap,
            cell,
            "the subject lacks the source cap",
        )
    });

    // federation.query → resolve the datasource named in the args + check its endpoint cap.
    if target.tool == "federation.query" {
        check_datasource(store, ws, target, cell, deps).await?;
    }
    // query.run → the saved query's underlying verb + its target (one hop, cycle-detected).
    if target.tool == "query.run" {
        check_saved_query(store, subject, ws, target, cell, deps, visited).await?;
    }
    Ok(())
}

/// The `federation.query` datasource hop: the record must exist + resolve (in THIS workspace — a
/// cross-tenant name resolves to `None`, never leaks), and its `host:port` endpoint must be permitted
/// by the federation install's `net:*` grant (the SAME [`enforce_endpoint`] the live query runs). We
/// distinguish "not permitted" (a real cap gap) from "not reachable here" only at the report level —
/// `enforce_endpoint` reports the cap; physical reachability (edge vs cloud) is a datasource concern
/// this preflight notes, not decides.
async fn check_datasource(
    store: &Store,
    ws: &str,
    target: &Target,
    cell: &str,
    deps: &mut Vec<DepVerdict>,
) -> Result<(), DashboardError> {
    let Some(name) = datasource_name(&target.args) else {
        // No datasource named in the args → nothing to resolve (a platform-shaped federation call).
        return Ok(());
    };
    let dep = format!("datasource:{name}");
    match resolve_datasource(store, ws, &name).await? {
        None => {
            deps.push(DepVerdict::bad(
                &dep,
                DepKind::Datasource,
                cell,
                "datasource not found in this workspace (absent or removed)",
            ));
        }
        Some(ds) => {
            deps.push(DepVerdict::ok(
                &dep,
                DepKind::Datasource,
                cell,
                "datasource exists and resolves",
            ));
            // The endpoint net cap — the SAME pre-connect gate the live `federation.query` runs.
            let endpoint_dep = format!("net:tls:{}:connect", ds.endpoint);
            match enforce_endpoint(store, ws, &ds.endpoint).await {
                Ok(()) => deps.push(DepVerdict::ok(
                    &endpoint_dep,
                    DepKind::Endpoint,
                    cell,
                    "endpoint permitted by the federation install grant",
                )),
                Err(_) => deps.push(DepVerdict::bad(
                    &endpoint_dep,
                    DepKind::Endpoint,
                    cell,
                    "endpoint NOT permitted (federation install lacks the net: grant) — or not reachable here",
                )),
            }
        }
    }
    Ok(())
}

/// The `query.run` saved-query hop: resolve `query:<id>` (cycle-detected), report the underlying verb
/// cap (`store.query` / `federation.query`) the subject must ALSO hold (no-widening, rule 5), and for
/// a datasource-backed query, the datasource + endpoint. Deeper query→query chains are `unchecked`.
async fn check_saved_query(
    store: &Store,
    subject: &Principal,
    ws: &str,
    target: &Target,
    cell: &str,
    deps: &mut Vec<DepVerdict>,
    visited: &mut BTreeSet<String>,
) -> Result<(), DashboardError> {
    let Some(id) = saved_query_id(&target.args) else {
        return Ok(());
    };
    let dep = format!("query:{id}");
    if !visited.insert(dep.clone()) {
        deps.push(DepVerdict::unchecked(
            &dep,
            DepKind::SavedQuery,
            cell,
            "cycle — already walked this saved query",
        ));
        return Ok(());
    }
    let Some(q) = resolve_query(store, ws, &id).await? else {
        deps.push(DepVerdict::bad(
            &dep,
            DepKind::SavedQuery,
            cell,
            "saved query not found in this workspace",
        ));
        return Ok(());
    };
    let Ok(parsed) = QueryTarget::parse(&q.target) else {
        deps.push(DepVerdict::bad(
            &dep,
            DepKind::SavedQuery,
            cell,
            "saved query has an unparseable target",
        ));
        return Ok(());
    };
    // The underlying verb cap the subject must additionally hold.
    let underlying = parsed.underlying_tool();
    let cap = format!("mcp:{underlying}:call");
    let cap_ok = authorize_tool(subject, ws, underlying).is_ok();
    deps.push(if cap_ok {
        DepVerdict::ok(
            &cap,
            DepKind::SourceCap,
            cell,
            "the subject holds the saved-query verb cap",
        )
    } else {
        DepVerdict::bad(
            &cap,
            DepKind::SourceCap,
            cell,
            "the subject lacks the saved-query verb cap",
        )
    });
    // A datasource-backed saved query fans into the datasource + endpoint (one hop).
    if let QueryTarget::Datasource(name) = parsed {
        let synthetic = Target {
            tool: "federation.query".to_string(),
            args: serde_json::json!({ "datasource": name }),
            ..Default::default()
        };
        check_datasource(store, ws, &synthetic, cell, deps).await?;
    }
    Ok(())
}

/// A required variable is bindable iff it carries a static value/option-source (`custom`/`text`/
/// `const`/`interval`) OR its `query`/`source` resolver tool cap is held by the subject. v1 checks
/// "a default or option-source exists (or the resolver cap is held)"; full per-value validation
/// (does the option-source actually return a value?) is deferred and reported honestly.
fn check_required_var(subject: &Principal, ws: &str, var: &Variable) -> DepVerdict {
    let dep = format!("var:{}", var.name);
    // A static source is trivially bindable.
    let has_static = !var.custom.is_empty()
        || !var.text.is_empty()
        || !var.const_.is_empty()
        || !var.interval.is_empty();
    if has_static {
        return DepVerdict::ok(
            &dep,
            DepKind::Variable,
            "",
            "bindable (static default/options)",
        );
    }
    // A query/source-backed var: the subject needs the resolver tool's cap. Full option-resolution is
    // a mini-closure deferred to a later phase — marked unchecked when the cap IS held (so we never
    // claim green on an un-validated resolver), bad when the cap is missing (a definite gap).
    if let Some(tool) = var.query.get("tool").and_then(Value::as_str) {
        if tool.is_empty() {
            return DepVerdict::bad(
                &dep,
                DepKind::Variable,
                "",
                "required var has no bindable source",
            );
        }
        if authorize_tool(subject, ws, tool).is_ok() {
            return DepVerdict::unchecked(
                &dep,
                DepKind::Variable,
                "",
                "resolver cap held; per-value option resolution not validated in v1",
            );
        }
        return DepVerdict::bad(
            &dep,
            DepKind::Variable,
            "",
            "the subject lacks the required var's resolver cap",
        );
    }
    DepVerdict::bad(
        &dep,
        DepKind::Variable,
        "",
        "required var has no bindable source",
    )
}

/// Read a panel record directly (crate-internal, no gate — the caller has already gated the DASHBOARD
/// read; gate-3 for the panel is applied via `may_read_panel` for the SUBJECT). `None` if absent/deleted.
async fn read_panel_record(
    store: &Store,
    ws: &str,
    id: &str,
) -> Result<Option<Panel>, DashboardError> {
    Ok(crate::panel::read_panel(store, ws, id)
        .await?
        .filter(|p| !p.deleted))
}

/// Pull the datasource name a `federation.query` target's args name (`datasource` or `source`), if any.
fn datasource_name(args: &Value) -> Option<String> {
    args.get("datasource")
        .or_else(|| args.get("source"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(|s| s.trim_start_matches("datasource:").to_string())
}

/// Pull the saved-query id a `query.run` target's args name (`id` or `query`), if any.
fn saved_query_id(args: &Value) -> Option<String> {
    args.get("id")
        .or_else(|| args.get("query"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(|s| s.trim_start_matches("query:").to_string())
}
