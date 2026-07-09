//! `dashboard.pin(dashboard, envelope, title?, now)` — mint a persisted `dashboard:{id}` cell from an
//! `x-lb-render` envelope (widget-platform scope, Slice B — closes G2). The keystone for "widgets are
//! system-wide": a GENERIC path that takes ANY render envelope (a tool's `ToolDescriptor.result`, or a
//! live channel `rich_result` body) and mints a v3 `Cell` host-side, persisted through the SAME
//! validation + write chain `dashboard.save` uses. The reminder widget (`reminder.list`, which already
//! declares a `result = table` render) becomes dashboard-addable with ZERO reminder-specific code in
//! this path — the envelope is OPAQUE DATA. No branch on a tool id (rule 10).
//!
//! **Why a server-side mint (not client-compose).** The umbrella leaned client-compose "unless a
//! server-side mint proves necessary"; this is the proof of necessity — the SAME argument Slice A used
//! to put save-validation server-side: a pin produces *persisted state* (`dashboard:{id}` cell), and a
//! headless external agent over `POST /mcp/call` (no shell, no `ResponseView.buildCell`) must be able to
//! pin a tool's `result` envelope. With client-compose every client (web, RN app, AI agent, external
//! agent) re-implements the envelope→cell mapping; the host can't enforce fidelity. One mint function in
//! the host = ONE mapping reused by the shell path, the headless `POST /mcp/call` path, and (later) Slice
//! D's channel-origin authoring. The channel render path (`ResponseView.buildCell`) is UNTOUCHED — it
//! keeps doing ephemeral envelope→cell for render; this is the persist-time twin.
//!
//! **Idempotent.** The minted cell `i` is `pin-{slug(envelope.source.tool || envelope.view || "cell")}`
//! — pure string ops, no `match` on the tool id (rule 10). Re-pinning the SAME envelope (same source.tool)
//! REPLACES the cell in place (keeps its layout); pinning a DIFFERENT envelope appends. So "pin the
//! reminder widget to my Ops dashboard" is one cell that refreshes on re-pin, not N duplicates.
//!
//! **Reuses the Slice A validation chain.** The minted cell + the dashboard's existing cells run through
//! `check_cells_bounds` → `check_genui_cells` → `check_view_cells` → `validate_and_strip_refs` — the SAME
//! authority `dashboard.save` uses (so a hallucinated view is rejected loudly here too, for every writer).
//! The pin does NOT call `dashboard_save` (it has its own cap gate `mcp:dashboard.pin:call`, distinct from
//! `mcp:dashboard.save:call`); it reuses the validation PRIMITIVES (they're `pub`).
//!
//! **Owner-only update.** A pin against an existing dashboard is allowed only for its owner — a non-owner
//! with the pin cap still cannot overwrite someone else's dashboard (mirrors `dashboard.save`'s owner
//! check). Create on a fresh id (owner = principal, visibility = private).

use lb_auth::Principal;
use lb_mcp::ToolDescriptor;
use lb_store::Store;
use serde_json::Value;

use super::authorize::authorize_dashboard;
use super::bounds::check_cells_bounds;
use super::error::DashboardError;
use super::genui::check_genui_cells;
use super::model::{Action, Cell, Dashboard, Source, Target, Visibility, SCHEMA_VERSION};
use super::store::{read_dashboard, write_dashboard};
use super::views::check_view_cells;

/// The cell-id prefix for a pinned cell — `pin-{slug}`. Stable across re-pins so the same envelope
/// refreshes the cell in place (idempotent), not duplicates.
const PIN_PREFIX: &str = "pin-";

/// `dashboard.pin` — mint a cell from `envelope` and upsert it into dashboard `id` as `principal` at
/// logical time `now`. `title` is used only when creating a fresh dashboard (mirrors `dashboard.save`'s
/// create path); an existing dashboard keeps its title. Returns the HYDRATED record (mirrors
/// `dashboard.save`'s return — a client that `setCurrent`s the return renders ref cells correctly without
/// a reload).
pub async fn dashboard_pin(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    title: &str,
    envelope: &Value,
    now: u64,
) -> Result<Dashboard, DashboardError> {
    // Gate 1+2: workspace wall, then `mcp:dashboard.pin:call`. Denials opaque.
    authorize_dashboard(principal, ws, "dashboard.pin")?;
    if id.is_empty() {
        return Err(DashboardError::BadInput("empty dashboard id".into()));
    }

    // Mint the cell from the envelope FIRST (cheap, pure). A malformed envelope is a loud `BadInput`
    // before any read — the same "reject at the boundary" stance Slice A's validator takes.
    let minted = mint_cell_from_envelope(envelope, None)?;

    // Read the existing dashboard (tombstoned = absent → create). Owner-only update; create on a fresh id.
    let (mut dashboard, is_create) =
        match read_dashboard(store, ws, id).await?.filter(|d| !d.deleted) {
            Some(existing) => {
                if existing.owner != principal.owner_sub() {
                    return Err(DashboardError::Denied);
                }
                (existing, false)
            }
            None => (
                Dashboard {
                    id: id.to_string(),
                    title: title.to_string(),
                    description: String::new(),
                    icon: String::new(),
                    color: String::new(),
                    owner: principal.owner_sub().to_string(),
                    visibility: Visibility::Private,
                    cells: Vec::new(),
                    variables: Vec::new(),
                    schema_version: SCHEMA_VERSION,
                    updated_ts: now,
                    deleted: false,
                },
                true,
            ),
        };

    // Re-mint with the EXISTING cell's layout if a cell with the same `i` is already on the dashboard —
    // a re-pin preserves position (x/y/w/h). Append otherwise; place at the next free y.
    let existing = dashboard.cells.iter().find(|c| c.i == minted.i);
    let mut minted = mint_cell_from_envelope(envelope, existing)?;
    if existing.is_none() {
        minted.y = next_free_y(&dashboard.cells);
    }

    // LIBRARY-PANELS (the "pin saves a reusable widget" half): the minted spec is also PERSISTED as a
    // `panel:{slug}` record — a workspace-namespaced reusable widget the user can later browse
    // (`panel.list`), drop onto other dashboards, or open in the data studio. The dashboard cell then
    // REFERENCES the panel (`panel_ref`), so an edit to the panel propagates to every dashboard that
    // references it (the "one source of truth" the library-panels scope is about). The panel-write goes
    // through the same bounds check + owner-only-update contract `panel.save` enforces; the cap gate is
    // `mcp:dashboard.pin:call` (already passed above) — pin is a PRIVILEGED internal path into the panel
    // table, exactly as it is into the dashboard table (it writes via `write_dashboard`, not
    // `dashboard_save`). Bypassing `panel_save` here is the decided seam: a pin caller does NOT need
    // `mcp:panel.save:call` separately.
    let panel_id = minted
        .i
        .strip_prefix(PIN_PREFIX)
        .filter(|s| !s.is_empty())
        .unwrap_or(&minted.i)
        .to_string();
    let spec = panel_spec_from_cell(&minted);
    crate::panel::check_spec_bounds(&spec, &panel_id)
        .map_err(|e| DashboardError::BadInput(e.to_string()))?;
    let (owner, visibility) = match crate::panel::read_panel(store, ws, &panel_id)
        .await?
        .filter(|p| !p.deleted)
    {
        // Owner-only update on the panel too — a non-owner pinning to a dashboard they own must not
        // overwrite someone else's reusable widget. A tombstoned record resurrects under the new owner.
        Some(existing) if existing.owner != principal.owner_sub() => {
            return Err(DashboardError::Denied);
        }
        Some(existing) => (existing.owner, existing.visibility),
        None => (
            principal.owner_sub().to_string(),
            crate::panel::Visibility::Private,
        ),
    };
    let panel = crate::panel::Panel {
        id: panel_id.clone(),
        title: panel_title_from_envelope(envelope, &minted),
        owner,
        visibility,
        spec,
        schema_version: crate::panel::SCHEMA_VERSION,
        updated_ts: now,
        deleted: false,
    };
    crate::panel::write_panel(store, ws, &panel).await?;
    // NOTE: `minted.panel_ref` is set AFTER the validation chain runs, not here — the chain's
    // `validate_and_strip_refs` re-resolves every `panel_ref` through `panel_get`'s three gates, which
    // would deny a pin caller who has `mcp:dashboard.pin:call` but not `mcp:panel.get:call`. The pin
    // path is a PRIVILEGED internal writer (it just wrote the panel itself via `write_panel`), so it
    // strips its own cell below instead of re-resolving.

    // Replace the cell with the same `i` if present; else append.
    if let Some(slot) = dashboard.cells.iter_mut().find(|c| c.i == minted.i) {
        *slot = minted.clone();
    } else {
        dashboard.cells.push(minted.clone());
    }
    dashboard.title = if is_create {
        title.to_string()
    } else {
        dashboard.title
    };
    dashboard.schema_version = SCHEMA_VERSION;
    dashboard.updated_ts = now;

    // The SAME validation chain `dashboard.save` runs — the host is the boundary (Slice A's authority).
    // Order matches `save.rs`: bounds → genui IR → view-name → ref-strip. A hallucinated view in the
    // envelope is rejected HERE, for the shell path AND a headless `POST /mcp/call` writer alike.
    check_cells_bounds(&dashboard.cells)?;
    check_genui_cells(&dashboard.cells)?;
    check_view_cells(&dashboard.cells)?;
    let persisted = crate::panel::validate_and_strip_refs(
        store,
        principal,
        ws,
        std::mem::take(&mut dashboard.cells),
    )
    .await
    .map_err(DashboardError::BadInput)?;
    // PRIVILEGED STRIP: the freshly minted cell still carries its full spec (validate_and_strip_refs
    // left it inline because `panel_ref` is unset). The pin path WROTE the panel itself one call ago,
    // so it is its own authority — strip the cell to layout + `panel_ref` + title-override here, exactly
    // as `panel::validate::stripped_ref` would, bypassing the three-gate `panel_get` re-resolution.
    let persisted = persisted
        .into_iter()
        .map(|c| {
            if c.i == minted.i {
                let mut stripped = c;
                stripped.panel_ref = format!("panel:{panel_id}");
                stripped.v = 0;
                stripped.widget_type.clear();
                stripped.view.clear();
                stripped.binding = Value::Null;
                stripped.source = Default::default();
                stripped.action = Default::default();
                stripped.options = Value::Null;
                stripped.description.clear();
                stripped.sources.clear();
                stripped.transformations.clear();
                stripped.field_config = Value::Null;
                stripped.plugin_version.clear();
                stripped.panel_missing = false;
                stripped
            } else {
                c
            }
        })
        .collect();
    dashboard.cells = persisted;

    write_dashboard(store, ws, &dashboard).await?;

    // Return a HYDRATED record (mirrors `dashboard.save`). Ref cells were just stripped to layout+ref;
    // re-hydrate the returned value (not the persisted record) so a client that `setCurrent`s the return
    // renders ref cells without a reload.
    let mut dashboard = dashboard;
    dashboard.cells =
        crate::panel::hydrate_cells(store, principal, ws, std::mem::take(&mut dashboard.cells))
            .await;
    Ok(dashboard)
}

/// Mint a v3 [`Cell`] from an `x-lb-render` envelope. Mirrors `ResponseView.buildCell` (the shipped
/// channel render adapter) field-for-field so a pinned cell renders identically to the channel response
/// (the cross-surface fidelity invariant): `view`/`source`/`action`/`options`/`fieldConfig` are copied
/// verbatim, the envelope's extra `tools[]` (row-control write verbs) become hidden `sources[]` so
/// `cellTools(cell)` covers `render.tools` (the bridge leash), and a stable `i = pin-{slug}` is derived
/// by PURE STRING OPS from `source.tool` (opaque data — rule 10; no `match`/`if` on the id).
///
/// `existing` (a found cell with the same `i`) supplies the layout a re-pin should preserve; `None` uses
/// the default `0,0,6,4` placement. The envelope is OPAQUE — no tool id is special-cased, so this same
/// function mints a `reminder.list` cell, a `federation.query` cell, or a hypothetical `__test__.x` cell
/// identically. That is the "generic over tool id" claim the headline test asserts.
pub fn mint_cell_from_envelope(
    envelope: &Value,
    existing: Option<&Cell>,
) -> Result<Cell, DashboardError> {
    let view = envelope
        .get("view")
        .and_then(Value::as_str)
        .ok_or_else(|| DashboardError::BadInput("envelope missing string 'view'".into()))?;
    if view.is_empty() {
        return Err(DashboardError::BadInput("envelope has empty 'view'".into()));
    }

    // The stable cell id — `pin-{slug}`. The slug derives from `source.tool` (opaque string) if present,
    // else `view`, else a fallback. Pure string ops: non-alphanumeric → `-`, lowercased. NO branch on the
    // tool id (rule 10) — a tool id is DATA here, never a special case.
    let tool = envelope
        .get("source")
        .and_then(|s| s.get("tool"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let slug_src = if !tool.is_empty() { tool } else { view };
    let i = format!("{PIN_PREFIX}{}", slug(slug_src));

    // `source { tool, args }` — the envelope shape matches `Source` field-for-field.
    let source: Source = envelope
        .get("source")
        .filter(|v| !v.is_null())
        .map(|s| {
            serde_json::from_value(s.clone())
                .map_err(|e| DashboardError::BadInput(format!("envelope.source: {e}")))
        })
        .transpose()?
        .unwrap_or_default();

    // `action { tool, argsTemplate }` — the envelope uses camelCase `argsTemplate`; `Action` stores
    // `args_template` (snake). Map explicitly (no serde rename so the on-wire cell shape stays unchanged).
    let action: Action = envelope
        .get("action")
        .filter(|v| !v.is_null())
        .map(|a| Action {
            tool: a
                .get("tool")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            args_template: a.get("argsTemplate").cloned().unwrap_or(Value::Null),
        })
        .unwrap_or_default();

    // The envelope's extra `tools[]` (row-control write verbs) become hidden `sources[]` so the bridge
    // leash (`cellTools(cell)`) = `render.tools`. Drop the source/action tools (they're already on the
    // cell). Mirrors `ResponseView.buildCell`'s `extraTools` fold exactly.
    let extra_tools: Vec<String> = envelope
        .get("tools")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .filter(|t| t != &source.tool && t != &action.tool)
                .collect()
        })
        .unwrap_or_default();
    // Real multi-ref targets declared by the envelope (`sources[]`, the v3 shape a `genui` view binds
    // its `/data/{refId}` pointers against — channel-widgets slice) carry through VERBATIM, first; the
    // extra `tools[]` then ride as hidden trailing targets (leash widening only, as before). A
    // malformed entry is dropped, not a pin failure (the cell validators still gate the result).
    let mut sources: Vec<Target> = envelope
        .get("sources")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|t| serde_json::from_value::<Target>(t.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    // A tool already covered by a declared target needs no hidden duplicate — the leash reads it there.
    let extra_tools: Vec<String> = extra_tools
        .into_iter()
        .filter(|t| !sources.iter().any(|s| &s.tool == t))
        .collect();
    sources.extend(
        extra_tools
            .into_iter()
            .enumerate()
            .map(|(idx, tool)| Target {
                ref_id: format!("T{idx}"),
                datasource: serde_json::json!({ "type": "surreal" }),
                tool,
                args: Value::Null,
                hide: true,
            }),
    );

    let options = envelope.get("options").cloned().unwrap_or(Value::Null);
    let field_config = envelope.get("fieldConfig").cloned().unwrap_or(Value::Null);

    // The envelope's `title` names the minted cell (and, via `panel_title_from_envelope`, the reusable
    // panel). The pin UI lets the user type this; an agent may set it in the fenced envelope too.
    let title = envelope
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    // Layout: preserve an existing cell's geometry on re-pin (idempotent), else default.
    let (x, y, w, h) = match existing {
        Some(c) => (c.x, c.y, c.w, c.h),
        None => (0, 0, 6, 4),
    };

    Ok(Cell {
        i,
        x,
        y,
        w,
        h,
        v: 3,
        widget_type: view.to_string(),
        title,
        view: view.to_string(),
        binding: Value::Null,
        source,
        action,
        sources,
        options,
        description: String::new(),
        transformations: Vec::new(),
        field_config,
        plugin_version: String::new(),
        panel_ref: String::new(),
        panel_vars: Value::Null,
        panel_missing: false,
    })
}

/// The lowest free y for a new cell — one past the bottom of the tallest existing cell. A dashboard with
/// no cells places at y=0. Pure (no mutation); used only for the append path.
fn next_free_y(cells: &[Cell]) -> u32 {
    cells
        .iter()
        .map(|c| c.y.saturating_add(c.h))
        .max()
        .unwrap_or(0)
}

/// Build the reusable-panel spec from a freshly minted cell — the non-layout half of the v3 cell, by
/// field. Mirrors `PanelSpec`'s `Default`-on-missing semantics so a v1/v2 envelope produces a valid v3
/// panel. Pure (no mutation of `cell`); the caller still has the full cell for validation + strip.
fn panel_spec_from_cell(cell: &Cell) -> crate::panel::PanelSpec {
    crate::panel::PanelSpec {
        v: cell.v,
        widget_type: cell.widget_type.clone(),
        title: cell.title.clone(),
        view: cell.view.clone(),
        binding: cell.binding.clone(),
        source: cell.source.clone(),
        action: cell.action.clone(),
        options: cell.options.clone(),
        description: cell.description.clone(),
        sources: cell.sources.clone(),
        transformations: cell.transformations.clone(),
        field_config: cell.field_config.clone(),
        plugin_version: cell.plugin_version.clone(),
    }
}

/// The panel's human-readable title — the rendered widget's title if the envelope carried one, else a
/// short label derived from the bound tool (e.g. "reminder.list widget"). The asset name lives on
/// `Panel::title`; the slug (the id) is forever.
fn panel_title_from_envelope(envelope: &Value, cell: &Cell) -> String {
    if !cell.title.is_empty() {
        return cell.title.clone();
    }
    if let Some(t) = envelope.get("title").and_then(Value::as_str) {
        if !t.is_empty() {
            return t.to_string();
        }
    }
    let tool = envelope
        .get("source")
        .and_then(|s| s.get("tool"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if !tool.is_empty() {
        return format!("{tool} widget");
    }
    format!("{} widget", cell.view)
}

/// Slugify an opaque id string into a stable cell-key segment — lowercase, non-alphanumeric → `-`,
/// collapsed trailing runs. Pure string ops; the input is treated as DATA (a tool id, a view id) never
/// branched on. `reminder.list` → `reminder-list`; `ext:acme/heat` → `ext-acme-heat`.
fn slug(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = false;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    // Trim BOTH ends — leading separators (a tool id starting with `_`/`.`) must not produce a leading
    // dash that doubles up with the `pin-` prefix, and a trailing dash is unsightly.
    out.trim_matches('-').to_string()
}

/// The `dashboard.pin` descriptor — a write verb the catalog lists so an AI discovers it can pin.
/// `envelope` is an opaque object (the `x-lb-render` shape); `dashboard` + `now` are required.
pub fn pin_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        name: "dashboard.pin".to_string(),
        title: "Pin a tool result to a dashboard".to_string(),
        group: "dashboard".to_string(),
        input_schema: Some(serde_json::json!({
            "type": "object",
            "properties": {
                "dashboard": { "type": "string", "x-lb": { "label": "Dashboard id", "description": "The target dashboard id (idempotent UPSERT: fresh id creates, existing id updates owner-only)" } },
                "title": { "type": "string", "x-lb": { "label": "Dashboard title", "description": "Used only when creating a fresh dashboard" } },
                "envelope": { "type": "object", "x-lb": { "label": "Render envelope", "description": "The x-lb-render envelope (a tool's descriptor.result or a channel rich_result body minus kind/v): { view, title?, source?, action?, options?, tools?, fieldConfig? } — title names the pinned widget/panel" } }
            },
            "required": ["dashboard", "envelope"]
        })),
        result: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Minting from `reminder.list`'s declared `result` envelope produces a v3 cell whose fields mirror
    /// the envelope (the headline fidelity invariant, unit-level). The tool id is OPAQUE — no branch.
    #[test]
    fn mint_mirrors_reminder_list_envelope() {
        let env = json!({
            "v": 2, "view": "table",
            "source": { "tool": "reminder.list", "args": {} },
            "options": { "rowControls": [
                { "kind": "switch", "label": "enabled", "action": { "tool": "reminder.update", "argsTemplate": { "id": "${id}", "enabled": "{{value}}" } } },
                { "kind": "button", "buttonLabel": "Run now", "action": { "tool": "reminder.fire", "argsTemplate": { "id": "${id}" } } },
                { "kind": "button", "buttonLabel": "Delete", "action": { "tool": "reminder.delete", "argsTemplate": { "id": "${id}" } } }
            ] },
            "fieldConfig": { "defaults": {}, "overrides": [] },
            "tools": ["reminder.list", "reminder.update", "reminder.fire", "reminder.delete"]
        });
        let cell = mint_cell_from_envelope(&env, None).expect("reminder.list envelope mints");
        assert_eq!(cell.i, "pin-reminder-list");
        assert_eq!(cell.view, "table");
        assert_eq!(cell.widget_type, "table");
        assert_eq!(cell.v, 3);
        assert_eq!(cell.source.tool, "reminder.list");
        assert_eq!(cell.source.args, json!({}));
        assert_eq!(cell.options["rowControls"].as_array().unwrap().len(), 3);
        assert_eq!(cell.field_config["defaults"], json!({}));
        // The `tools` fold: reminder.list is the source, the other three are hidden extra targets.
        assert_eq!(cell.sources.len(), 3);
        let tools: Vec<&str> = cell.sources.iter().map(|t| t.tool.as_str()).collect();
        assert!(tools.contains(&"reminder.update"));
        assert!(tools.contains(&"reminder.fire"));
        assert!(tools.contains(&"reminder.delete"));
        for t in &cell.sources {
            assert!(t.hide, "extra tool {} is hidden", t.tool);
            assert!(t.ref_id.starts_with('T'));
        }
    }

    /// The mint is GENERIC over the tool id (rule 10) — an arbitrary/unknown tool id mints a valid cell;
    /// no `match`/`if` on the id. This is the unit-level assertion the headline test scales to a real pin.
    #[test]
    fn mint_is_generic_over_an_arbitrary_tool_id() {
        let env = json!({
            "view": "table",
            "source": { "tool": "__test__.frobnicate", "args": { "x": 1 } },
            "tools": ["__test__.frobnicate"]
        });
        let cell = mint_cell_from_envelope(&env, None).expect("arbitrary tool id mints");
        assert_eq!(cell.i, "pin-test-frobnicate");
        assert_eq!(cell.source.tool, "__test__.frobnicate");
        assert_eq!(cell.source.args, json!({ "x": 1 }));
        assert!(
            cell.sources.is_empty(),
            "source.tool is the only tool → no extra targets"
        );
    }

    /// A re-pin preserves the existing cell's layout (idempotent position); a fresh pin uses the default.
    #[test]
    fn re_pin_preserves_layout() {
        let env = json!({ "view": "table", "source": { "tool": "reminder.list" } });
        let existing = Cell {
            i: "pin-reminder-list".into(),
            x: 7,
            y: 9,
            w: 3,
            h: 2,
            v: 3,
            widget_type: "table".into(),
            title: String::new(),
            view: "table".into(),
            binding: Value::Null,
            source: Default::default(),
            action: Default::default(),
            options: Value::Null,
            description: String::new(),
            sources: Vec::new(),
            transformations: Vec::new(),
            field_config: Value::Null,
            plugin_version: String::new(),
            panel_ref: String::new(),
            panel_vars: Value::Null,
            panel_missing: false,
        };
        let cell = mint_cell_from_envelope(&env, Some(&existing)).expect("re-pin mints");
        assert_eq!(cell.i, "pin-reminder-list");
        assert_eq!(
            (cell.x, cell.y, cell.w, cell.h),
            (7, 9, 3, 2),
            "layout preserved"
        );
    }

    /// Malformed envelopes are rejected loudly at the boundary (before any read), naming the gap.
    #[test]
    fn mint_rejects_missing_or_empty_view() {
        let err = mint_cell_from_envelope(&json!({ "source": { "tool": "x" } }), None)
            .expect_err("missing view rejected");
        assert!(matches!(err, DashboardError::BadInput(m) if m.contains("view")));
        let err =
            mint_cell_from_envelope(&json!({ "view": "" }), None).expect_err("empty view rejected");
        assert!(matches!(err, DashboardError::BadInput(m) if m.contains("view")));
    }

    /// The slug is pure string ops: non-alphanumeric → `-`, collapsed, lowercased.
    #[test]
    fn slug_is_pure_string_ops() {
        assert_eq!(slug("reminder.list"), "reminder-list");
        assert_eq!(slug("ext:acme/heat"), "ext-acme-heat");
        assert_eq!(slug("__test__.x"), "test-x");
        assert_eq!(slug("table"), "table");
        assert_eq!(slug("---"), "");
    }

    /// `next_free_y` is one past the tallest cell; an empty dashboard places at y=0.
    #[test]
    fn next_free_y_is_one_past_the_tallest() {
        let mk = |i: &str, y: u32, h: u32| Cell {
            i: i.into(),
            x: 0,
            y,
            h,
            w: 6,
            v: 3,
            widget_type: String::new(),
            title: String::new(),
            view: String::new(),
            binding: Value::Null,
            source: Default::default(),
            action: Default::default(),
            options: Value::Null,
            description: String::new(),
            sources: Vec::new(),
            transformations: Vec::new(),
            field_config: Value::Null,
            plugin_version: String::new(),
            panel_ref: String::new(),
            panel_vars: Value::Null,
            panel_missing: false,
        };
        assert_eq!(next_free_y(&[]), 0);
        assert_eq!(next_free_y(&[mk("a", 0, 4)]), 4);
        assert_eq!(next_free_y(&[mk("a", 0, 4), mk("b", 4, 3)]), 7);
    }
}
