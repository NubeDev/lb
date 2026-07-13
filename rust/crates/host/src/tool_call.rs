//! `call_tool` — the host's generic MCP bridge entry, exposed so a *transport* (the gateway's
//! `POST /mcp/call`) can forward an extension page/widget's `{tool, args}` through the **one** MCP
//! contract (rule 7, ui-federation scope). It runs the workspace-first, then `mcp:<tool>:call`
//! authorize gate and then dispatches — so a bridged caller is denied exactly like any other.
//!
//! Two dispatch families share the one contract:
//!   - **extension tools** (`<ext>.<tool>`, wasm or routed native) — resolved in the runtime
//!     `Registry` and run via `lb_mcp::call`.
//!   - **host-native tools** (`series.*` / `ingest.*`) — NOT in the runtime registry (they are host
//!     verbs over the embedded store, not components), so `lb_mcp::call` alone would `NotFound` them.
//!     The bridge must reach them too: a federated page reads platform data through `series.find` /
//!     `series.latest` here exactly as it would any extension tool. We authorize with the SAME MCP
//!     gate first (opaque `Denied`), then delegate to `call_ingest_tool` (which re-checks its own
//!     store-surface gate). This is the seam the `proof-panel` page exercises end to end; before it,
//!     `/mcp/call` could not dispatch a host-native verb at all (see
//!     debugging/extensions/bridge-cannot-dispatch-host-native-series.md).

use std::sync::Arc;

use lb_assets::read_install;
use lb_auth::Principal;
use lb_inbox::Decision;
use lb_mcp::{authorize_tool, ToolError};
use lb_runtime::{CallContext, Caller};
use serde_json::{json, Value};

use crate::boot::Node;
use crate::callback::Bridge;
use crate::ingest::{call_ingest_tool, publish_sample};
use crate::undo::{history_compensations, history_list, redo, undo, UndoSvcError};
use crate::{
    enqueue_held_outbox, enqueue_outbox, list_inbox, outbox_due, outbox_mark_delivered,
    outbox_mark_failed, outbox_status, record_inbox, resolve_inbox,
};

/// The host-native verb prefixes the bridge dispatches over the embedded store (not the runtime
/// registry). Kept narrow on purpose — the read-only series surface a federated page reads, `ingest.*`
/// for symmetry, plus the durable-workflow surface (reads `outbox.status`, `inbox.list`; writes that
/// PRODUCE motion `inbox.record`, `outbox.enqueue`; resolve `inbox.resolve`) the proof-panel demo
/// exercises. Each still passes the per-verb MCP gate first (the bridge scope filter is only defense in
/// depth).
// The prefix/exact lists are shared consts so the static host inventory (`system/catalog.rs`)
// can assert it covers every dispatched family — the hand-maintained mirror drifting is exactly
// how whole verb families went missing from `tools.catalog` (and thus from the agent's menu); see
// debugging/agent/persona-menu-missing-tools-catalog-descriptor-only.md.
//
// Notes on individual families:
//   - `layout.` — data-studio scope v2: the member-owned per-surface layout record (`get`/`set`).
//   - `channel.` — the per-viewer chart-preference verbs (a query-result's plot override) + the
//     channel read/write MCP surface (post/history/edit/delete/list — rules-messaging-scope): a
//     channel is a host-native plane over the embedded store + bus, not a runtime-registry
//     extension. (A future channel *extension* tool would carry its own `<ext>.` id and still
//     route to the registry — `channel.` here is the host's own channel service, one owner.)
pub(crate) const HOST_NATIVE_PREFIXES: &[&str] = &[
    "series.",
    "ingest.",
    "outbox.",
    "inbox.",
    "insight.",
    "authz.",
    // authz admin verbs (authz-verbs-mcp-dispatch scope): `call_authz_tool` already implements
    // every `grants.*`/`roles.*`/`teams.*` verb; these prefixes route them through the one MCP
    // bridge so a native (Tier-2) extension can MINT a scoped grant over the host callback, not
    // just READ it (`authz.check_scoped`/`scope_filter` were already reachable under `authz.`).
    "grants.",
    "roles.",
    "teams.",
    "invite.",
    "media.",
    "device.",
    "notify.",
    "dashboard.",
    "nav.",
    "layout.",
    "panel.",
    "report.",
    "brand.",
    "channel.",
    "viz.",
    "template.",
    "devkit.",
    "agent.",
    "rules.",
    "federation.",
    "flows.",
    "datasource.",
    "dbschema.",
    "secret.",
    "host.",
    "weather.",
    "prefs.",
    "message.",
    "bus.",
    "reminder.",
    "query.",
    "assets.",
    // doc-extraction scope: the `docs.*` doc-derived verb family (v1: `docs.extract`). A NEW
    // native prefix, NOT an `assets.` arm — doc CRUD lives under `assets.` (`assets.put_doc`), while
    // `docs.` is the home for operations that DERIVE docs (extract now; the embeddings scope's
    // `docs.search`/`docs.reindex` next). Reached over the one MCP bridge like every host-native verb.
    "docs.",
    "telemetry.",
    "history.",
    "tools.",
    // login-hardening scope: the admin credential-management verb (`identity.set_credential`) rides
    // the one MCP bridge like every other admin action, gated `mcp:identity.manage:call`. The other
    // `identity.*` verbs also have dedicated admin REST routes; reaching them here too is uniform.
    "identity.",
];

/// The prefix-less host-native verbs (`undo`/`redo`) + the four `store.*` verbs dispatched by exact
/// name (the rest of `store.` is not a bridge family).
pub(crate) const HOST_NATIVE_EXACT: &[&str] = &[
    "undo",
    "redo",
    "store.query",
    "store.schema",
    "store.write",
    "store.delete",
];

pub(crate) fn is_host_native(qualified_tool: &str) -> bool {
    HOST_NATIVE_PREFIXES
        .iter()
        .any(|p| qualified_tool.starts_with(p))
        || HOST_NATIVE_EXACT.contains(&qualified_tool)
}

/// The capability a call to `qualified_tool` actually gates on. Usually the tool's own name; the
/// exceptions are verbs that deliberately ride an EXISTING grant (same privilege, no new cap).
/// ONE mapping, two callers: the dispatcher's outer gate ([`call_tool_at_depth`]) and the
/// `tools.catalog` visibility gate — the catalog's cardinal rule ("advertise a tool only if the
/// call would allow it, never hide one that would pass") only holds if both consult the same alias.
///
/// - `federation.schema` / `federation.sample` — the no-SQL discovery verb and the AI-context
///   snapshot are the SAME read privilege as a live query (datasources-ux / datasource-samples
///   scopes): both gate under `mcp:federation.query:call`, the cap their service layer re-checks.
///   Without the alias the gate demanded a per-verb grant no role carries (the Datasources browse
///   panel was denied opaquely; the palette hid the verbs from callers who could run them).
/// - `outbox.enqueue_held` — rules-approvals scope: staging a gated effect is the SAME authority as
///   staging any effect (the *release* on approval is the gated step), so it rides
///   `mcp:outbox.enqueue:call`; no `enqueue_held` cap exists. The host fn re-checks inside.
/// - `telemetry.*` — telemetry-console scope: the read verbs (query/trace/tail) collapse onto the
///   ONE `mcp:telemetry.read:call` grant; purge keeps `mcp:telemetry.purge:call`. Re-checked inside.
/// - `nav.pref.*` — nav scope: the member-owned active pick gates on the SAME `mcp:nav.resolve:call`
///   read grant its verb re-checks; curating which nav you use is part of resolving your own menu.
/// - `nav.set_default` — nav scope: the workspace-default pointer is an authoring action — it gates
///   on the `mcp:nav.save:call` grant that creates the navs it points at. Re-checked inside.
pub(crate) fn gate_tool_for(qualified_tool: &str) -> &str {
    if qualified_tool == "federation.schema" || qualified_tool == "federation.sample" {
        "federation.query"
    } else if qualified_tool == "identity.set_credential" {
        // login-hardening scope: setting a user's password is the SAME admin authority as managing
        // identities — it rides the existing `mcp:identity.manage:call` grant (the scope's MCP §6.1
        // decision), not a new per-verb cap. The verb re-checks `identity.manage` inside.
        "identity.manage"
    } else if qualified_tool == "outbox.enqueue_held" {
        "outbox.enqueue"
    } else if qualified_tool.starts_with("telemetry.") {
        crate::read_or_admin_cap(qualified_tool)
    } else if qualified_tool.starts_with("nav.pref.") || qualified_tool == "nav.hidden.get" {
        // hide-and-pins scope: reading the hidden-set is part of resolving one's own menu (the
        // resolver echoes it to every member anyway) — same `mcp:nav.resolve:call` read grant.
        "nav.resolve"
    } else if qualified_tool == "nav.set_default" || qualified_tool == "nav.hidden.set" {
        // hide-and-pins scope: curating the workspace hidden-set is the SAME authoring authority as
        // the workspace-default pointer — it rides `mcp:nav.save:call`, no separate cap.
        "nav.save"
    } else if qualified_tool == "grants.revoke" {
        // authz-verbs-mcp-dispatch scope: assign/revoke MUTATE the same grant surface and share the
        // ONE cap `mcp:grants.assign:call` — the verb's inner gate (`authz/grants.rs`) checks that
        // cap for both. No `mcp:grants.revoke:call` exists in any role bundle, so without this alias
        // the outer gate would deny revoke for every caller, admins included.
        "grants.assign"
    } else if qualified_tool == "grants.list_scoped" {
        // authz-verbs-mcp-dispatch scope: listing scoped grants is the SAME read privilege as
        // `grants.list` — the inner gate checks `mcp:grants.list:call`; no per-verb cap exists.
        "grants.list"
    } else if qualified_tool == "teams.create" {
        // authz-verbs-mcp-dispatch scope: the inner gate + admin role bundle use
        // `mcp:teams.manage:call` (there is no `mcp:teams.create:call`); align the outer gate.
        "teams.manage"
    } else if qualified_tool == "roles.delete" {
        // authz-verbs-mcp-dispatch scope: deleting a role is the SAME authority as defining/managing
        // one — the inner gate checks `mcp:roles.manage:call`; no `mcp:roles.delete:call` exists.
        "roles.manage"
    } else {
        qualified_tool
    }
}

/// Call `qualified_tool` as `principal` in `ws` with a JSON input string, returning the tool's JSON
/// output. Authorization runs first; the workspace is the caller's (the gateway derives it from the
/// token, never the page). Host-native `series.*`/`ingest.*` verbs dispatch over the store; everything
/// else (`<ext>.<tool>`) routes through the runtime registry / bus.
///
/// This is the outermost (depth-0) entry — the page bridge (`POST /mcp/call`) and the gateway reach
/// it. A re-entrant guest→host callback re-enters at [`call_tool_at_depth`] one level deeper.
pub async fn call_tool(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input_json: &str,
) -> Result<String, ToolError> {
    // Observability/audit/undo share this one chokepoint (the "shared seam"). The telemetry emission
    // (observability scope) records the redacted dispatch decision here: outcome = allow/deny/error,
    // params as a DIGEST (never raw), a per-call trace_id. Emitted at the OUTERMOST entry only (depth
    // 0) — nested guest→host callbacks (depth > 0) contribute to the enclosing call's span, not a new
    // one. A tracing no-op when no layer is installed, so this is free on a node without the sink.
    let trace_id = lb_store::new_ulid();
    let source = qualified_tool.split('.').next().unwrap_or("host");
    let ts = now_ts();
    let input: Value = serde_json::from_str(input_json).unwrap_or(Value::Null);
    let result = call_tool_at_depth(node, principal, ws, qualified_tool, input_json, 0).await;
    emit_dispatch_decision(
        &result,
        principal,
        ws,
        qualified_tool,
        source,
        &trace_id,
        &input,
        ts,
    );
    result
}

/// The wall-clock-free logical timestamp the dispatch tags the event with (the same `ts` the caller
/// threads elsewhere). Uses `SystemTime` ONCE at the chokepoint for the event's `ts` field — the
/// ordering key is the ULID `seq`, not this; `ts` is only the human-facing time the console filters
/// on by range. (Core paths stay clock-free; this is the boundary.)
fn now_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Emit the redacted dispatch decision through the telemetry layer. The outcome is derived from the
/// call's result (`Denied` → deny, other error → error, ok → allow); the level tracks severity. The
/// raw params NEVER reach the event — `record_dispatch` digests them.
fn emit_dispatch_decision(
    result: &Result<String, ToolError>,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    source: &str,
    trace_id: &str,
    input: &Value,
    ts: u64,
) {
    use lb_telemetry::{Level, Outcome};
    let (level, outcome, msg) = match result {
        Ok(_) => (
            Level::Info,
            Outcome::Allow,
            format!("{qualified_tool} allowed"),
        ),
        Err(ToolError::Denied) => (
            Level::Warn,
            Outcome::Deny,
            format!("{qualified_tool} denied"),
        ),
        Err(_) => (
            Level::Error,
            Outcome::Error,
            format!("{qualified_tool} errored"),
        ),
    };
    lb_telemetry::record_dispatch(
        level,
        ws,
        principal.sub(),
        qualified_tool,
        source,
        trace_id,
        outcome,
        input,
        ts,
        &msg,
    );
}

/// The depth-tracked core of [`call_tool`]. `depth` is 0 for an outermost call and incremented by
/// the host callback on each guest→host hop ([`crate::callback`]). For a **wasm extension** target,
/// the host derives the guest's effective principal (`caller ∩ install-grant`) and installs a
/// [`Bridge`] into the instance so the guest's `host.call-tool` import can re-enter HERE — under that
/// narrowed authority, in this workspace. Host-native verbs and routed/remote targets need no
/// callback (they carry no guest).
pub async fn call_tool_at_depth(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input_json: &str,
    depth: u32,
) -> Result<String, ToolError> {
    // Auto-capture-on-dispatch (undo scope): at the OUTERMOST entry (depth 0), wrap the dispatch in
    // a runtime taint scope and journal the call into the undo journal — reversible mutations get an
    // undoable before-image, anything that reaches the outbox is journaled not-undoable (derived
    // from taint, the max-composition rule). Nested host-callback calls (depth > 0) run as-is: they
    // contribute their taint to the still-open enclosing scope, but only the outermost call journals
    // (one tool call = one undoable step). The undo/redo/history verbs are EXEMPT — they journal
    // their own `kind:undo` entries; auto-capturing them would double-journal and recurse.
    let is_undo_verb = qualified_tool == "undo"
        || qualified_tool == "redo"
        || qualified_tool.starts_with("history.");
    if depth == 0 && !is_undo_verb {
        let input: Value = serde_json::from_str(input_json)
            .map_err(|e| ToolError::BadInput(format!("input json: {e}")))?;
        // Optional batch/job group id the caller threads through for grouped-undo groundwork; a
        // standalone call leaves it None (the step's own seq becomes its group).
        let group = input
            .get("undo_group")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        return crate::undo_capture::capture_dispatch(
            &node.store,
            principal,
            ws,
            qualified_tool,
            &input,
            group,
            None, // declared compensation: a manifest field is a deferred additive ABI change
            dispatch_at_depth(node, principal, ws, qualified_tool, input_json, depth),
        )
        .await;
    }
    dispatch_at_depth(node, principal, ws, qualified_tool, input_json, depth).await
}

/// The raw dispatch (no undo capture) — host-native verbs over the store, or `<ext>.<tool>` routed
/// through the runtime registry / bus. Wrapped by [`call_tool_at_depth`] at depth 0 for auto-capture.
async fn dispatch_at_depth(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input_json: &str,
    depth: u32,
) -> Result<String, ToolError> {
    // The grant-free utility tier (prefs scope): `format.*` / `convert.unit` are pure CLDR/unit math
    // over NO tenant data, so they carry NO capability and are dispatched WITHOUT the authorize gate
    // (the caller passes resolved prefs/axes inline; the host reads no store). This branch sits
    // BEFORE the gated host-native block on purpose — routing them through `authorize_tool` would
    // wrongly require a `mcp:format.*:call` grant the scope says must not exist.
    if qualified_tool.starts_with("format.") || qualified_tool.starts_with("convert.") {
        let input: Value = serde_json::from_str(input_json)
            .map_err(|e| ToolError::BadInput(format!("input json: {e}")))?;
        let out = crate::call_format_tool(qualified_tool, &input)?;
        return serde_json::to_string(&out).map_err(|e| ToolError::Extension(e.to_string()));
    }

    // Defense-in-depth arg validation (channels-command-palette scope): validate `input` against the
    // tool's declared JSON Schema BEFORE dispatch — a structurally bad request is a clean
    // `BadInput`, never a panic deep in a handler. The handler still does its own checks; this is
    // the early, schema-driven gate. A tool without a declared schema passes (additive) — so verbs
    // without a schema (the majority, incl. undo) are unaffected. Skipped for the format/convert
    // tier above (no tenant data, no schema).
    {
        let input: Value = serde_json::from_str(input_json)
            .map_err(|e| ToolError::BadInput(format!("input json: {e}")))?;
        if let Some(schema) = descriptor_schema(node, qualified_tool) {
            crate::tools::validate_args(Some(&schema), &input)?;
        }
    }

    if is_host_native(qualified_tool) {
        // Same MCP gate as any tool (workspace-first, then `mcp:<tool>:call`) so a denied bridged
        // caller is opaque and indistinguishable from a missing tool — then delegate to the host verb.
        //
        // The per-verb cap aliases (verbs that ride an EXISTING grant — federation.schema/sample,
        // outbox.enqueue_held, telemetry.*, nav.pref.*/set_default) live in ONE mapping,
        // `gate_tool_for`, shared with the `tools.catalog` visibility gate so the palette can never
        // hide a verb this gate would pass (each aliased verb documents its rationale there).
        authorize_tool(principal, ws, gate_tool_for(qualified_tool))?;
        let input: Value = serde_json::from_str(input_json)
            .map_err(|e| ToolError::BadInput(format!("input json: {e}")))?;
        let out = if qualified_tool.starts_with("outbox.") || qualified_tool.starts_with("inbox.") {
            call_inbox_outbox_tool(node, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("insight.") {
            // insights scope: the durable insight + occurrences + subscriptions + policy surface.
            // The outer gate ran `mcp:insight.<verb>:call`; the verb re-runs it inside (defense in
            // depth). `insight.raise` needs the full `&Node` (bus event + tag graph + channel
            // delivery for matched subs); the read/act verbs use `node.store`. The matcher + ladder
            // state machine + digest reactor are pure / reactor-driven (no MCP arm of their own).
            crate::call_insight_tool(node, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("authz.")
            || qualified_tool.starts_with("grants.")
            || qualified_tool.starts_with("roles.")
            || qualified_tool.starts_with("teams.")
        {
            // entity-scoped-grants scope: `authz.check_scoped` / `authz.scope_filter` — the scoped
            // read API extensions reach via `host.call-tool` so an extension verb asks the wall
            // "what can this principal reach?" instead of re-implementing the filter. The outer
            // gate ran `mcp:authz.<verb>:call`; the verb resolves the CALLING principal's own reach
            // by default. native-caller-identity scope: an optional `subject` lets a caller that
            // holds `mcp:authz.delegate_reach:call` ask about ANOTHER subject (a native sidecar
            // answering "does the guardian reach this child?"); absent `subject` is unchanged, and a
            // `subject` without the delegation cap fails closed (see `authz::scoped`). `authz.resolve` /
            // `authz.revoke-tokens` are the access-console admin verbs (already in `call_authz_tool`).
            //
            // authz-verbs-mcp-dispatch scope: `grants.*`/`roles.*`/`teams.*` route here too — the
            // same `call_authz_tool` implements them, gated by the same admin caps (with the outer
            // gate aliased to the inner cap for the four verbs `gate_tool_for` maps). This makes the
            // WRITE half of the scoped-grant surface reachable over the callback, symmetric with the
            // READ half above.
            crate::call_authz_tool(&node.store, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("invite.") {
            // invites scope: the admin verbs (create/list/revoke/resend). The pre-auth `accept` is
            // a gateway route (POST /public/invite/accept), NOT an MCP verb — it has no principal.
            crate::call_invite_tool(&node.store, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("media.") {
            // media scope: upload_begin/commit/get/list/delete MCP verbs. The chunk upload (PUT)
            // and serve (GET) are HTTP routes — bytes over HTTP, not MCP payloads.
            crate::call_media_tool(&node.store, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("device.") || qualified_tool.starts_with("notify.") {
            // push-target scope: device.register/list/remove + notify.send.
            crate::call_notify_tool(&node.store, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool == "dashboard.catalog" {
            // widget-catalog scope: the palette read needs the full `&Node` (ext-tile discovery via
            // `ext.list`, like `nav.resolve`), so it is dispatched HERE — before the generic store-only
            // `dashboard.` branch. Workspace-first; folds only the caller's installed `[[widget]]` tiles.
            let cat = crate::dashboard_catalog(node, principal, ws).await?;
            serde_json::to_value(cat).unwrap_or(Value::Null)
        } else if qualified_tool.starts_with("dashboard.") {
            crate::call_dashboard_tool(&node.store, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("nav.") {
            // nav scope: the user-/team-authored menu asset. `resolve` + `pref.*` need the `&Node`
            // (ext discovery for `ext` items); the bridge takes it for all verbs.
            crate::call_nav_tool(node, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("layout.") {
            // data-studio scope v2: the member-owned per-surface ui-layout record (store-only).
            crate::call_layout_tool(&node.store, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("panel.") {
            // library-panels scope: the reusable panel asset. Same store-only surface as dashboards.
            crate::call_panel_tool(&node.store, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("report.") {
            // reports scope: the report-builder asset. Store-only surface (export is a gateway route).
            crate::call_report_tool(&node.store, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("brand.") {
            // reports scope: the reusable brand-profile asset. Store-only surface.
            crate::call_brand_tool(&node.store, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("channel.chart_pref.") {
            // channel query charts: a viewer's per-item plot override. The outer gate ran
            // `mcp:channel.chart_pref.<verb>:call`; the verb re-checks the channel `sub` gate.
            crate::call_channel_chart_pref_tool(&node.store, principal, ws, qualified_tool, &input)
                .await?
        } else if qualified_tool.starts_with("channel.") {
            // rules-messaging scope: the channel read/write MCP surface (post/history/edit/delete/
            // list). Thin wrappers over the existing host fns, each gate-identical to the WS path —
            // the outer gate ran `mcp:channel.<verb>:call`; the host fn re-runs the `bus:chan/{cid}:
            // {Pub|Sub}` gate inside. Reached here AFTER `channel.chart_pref.` (matched above).
            crate::call_channel_tool(node, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("viz.") {
            // The panel-data resolver. It RE-ENTERS this dispatcher (`call_tool_at_depth`) per target
            // under the caller's authority — so `depth` is threaded through to re-enter at depth+1
            // (no render-path cap bypass; the workspace wall + each target tool's own cap re-checked).
            crate::call_viz_tool(node, principal, ws, qualified_tool, &input, depth).await?
        } else if qualified_tool.starts_with("template.") {
            crate::call_template_tool(&node.store, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("devkit.") {
            crate::call_devkit_tool(node, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool == "agent.def.test" {
            // agent-catalog test-and-secrets scope: the context-proving diagnostic. Handled HERE
            // (not in `call_agent_tool`) because it needs the `&Arc<Node>` this dispatcher holds — it
            // assembles the caller's reachable tool menu (which needs the Arc) and runs one model turn
            // over the node's default model. Its own `mcp:agent.def.test:call` gate runs inside.
            let id = input.get("id").and_then(Value::as_str);
            let result = crate::agent_def_test(node, principal, ws, id).await?;
            serde_json::to_value(result).map_err(|e| ToolError::Extension(e.to_string()))?
        } else if qualified_tool.starts_with("agent.") {
            // agent-run scope Part 2: the policy/decision verbs (`agent.policy.set`, `agent.decide`).
            // One branch; `call_agent_tool` matches the verb and delegates. `agent.watch` (Part 3)
            // is added inside `call_agent_tool` by that worker — its arm is currently `NotFound`.
            crate::call_agent_tool(node, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("tools.") {
            // channels-command-palette scope: the `tools.catalog` read, reached over the same MCP
            // bridge as any verb (rule 7). The verb re-runs its own `authorize_tool` gate inside the
            // service, so the outer gate above and the inner one agree (one gate, two callers).
            crate::call_tools_tool(node, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("assets.") {
            // document-store / files scope: the asset verbs (`assets.put_doc`, `assets.get_doc`,
            // `assets.put_asset`, …) over the SAME MCP bridge every host-native verb uses. The
            // outer gate already ran `mcp:assets.<verb>:call`; `call_asset_tool` re-runs it
            // (defense in depth, and it is the tested bridge the UI/gateway calls directly too).
            // Routing here — rather than leaving `assets.*` as a side call — is what makes a
            // markdown **save** flow through the undo auto-capture wrapper at depth 0
            // (document-store scope: "save participates in undo/redo") and lets a guest
            // extension reach the store over the same dispatch path as everyone else.
            crate::call_asset_tool(&node.store, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("docs.") {
            // doc-extraction scope: the `docs.*` doc-derived verbs (v1: `docs.extract`). Its own
            // native family (see the prefix note above). The outer gate ran `mcp:docs.<verb>:call`;
            // `call_docs_tool` re-runs it, then the service adds the per-item media-read + doc-write
            // gates — a `docs.extract` grant never bypasses those.
            crate::call_docs_tool(&node.store, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("rules.") {
            crate::call_rules_tool(node, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("flows.") {
            // Type-erase this dispatch edge to a boxed `dyn Future + Send`. A `flows.run` reached from
            // INSIDE a running flow (a `tool` node invoking `flows.run`) is an async recursion through
            // here; without erasure the opaque future types cycle (`flows_run_async` → drive → tool →
            // dispatch → `flows_run_async`) and the compiler can neither size them nor prove `Send` —
            // which the background-run `tokio::spawn` requires. The output type is concrete
            // (`Result<String, ToolError>`), so the `dyn` cast is clean and cuts the cycle.
            crate::call_flows_tool_boxed(node, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("federation.")
            || qualified_tool.starts_with("datasource.")
            || qualified_tool.starts_with("dbschema.")
        {
            // datasources scope: the federation host service (resolve source → net:* → mediate DSN →
            // route to the supervised sidecar) + the `dbschema.*` designed-record CRUD (schema-
            // designer scope — store-only, no sidecar). The per-verb gate runs inside the service.
            crate::call_federation_tool(node, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("host.") {
            crate::call_host_tool(node, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("weather.") {
            // weather scope: the keyless Open-Meteo current-conditions read verb. Store-free —
            // no `&Node` needed, unlike most host-native families.
            crate::call_weather_tool(principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("secret.") {
            // secrets scope: the `secret.*` CRUD surface (set/get/set_visibility/delete/list),
            // reached over the same MCP bridge as any verb. The per-verb MCP gate + the
            // three-gate secret read run inside `call_secret_tool` / `lb-secrets`.
            crate::call_secret_tool(node, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("prefs.") {
            crate::call_prefs_tool(&node.store, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("message.") {
            // i18n-catalogs scope: `message.render` / `message.set_catalog`. The outer gate ran the
            // base `mcp:message.<verb>:call`; the render verb adds the `message.render_recipient`
            // grant for a foreign-recipient fan-out, and set_catalog publishes the "catalog changed"
            // hint (needs the bus).
            crate::call_catalog_tool(
                &node.store,
                &node.bus,
                principal,
                ws,
                qualified_tool,
                &input,
            )
            .await?
        } else if qualified_tool.starts_with("query.") {
            // query scope: the saved-PRQL-query service (compile→dispatch to store.query /
            // federation.query). query.run adds the no-widening target cap inside its service.
            crate::call_query_tool(node, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("telemetry.") {
            // telemetry-console scope: the gated, workspace-walled read surface over the capped
            // telemetry ring (query/trace/purge). Writes come from the SurrealCappedLayer only —
            // there is no telemetry.write verb; the ws wall is enforced inside each read.
            crate::call_telemetry_tool(node, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool == "store.query" || qualified_tool == "store.schema" {
            crate::call_store_query_tool(&node.store, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool == "store.write" || qualified_tool == "store.delete" {
            // The generic per-table mutation surface (the write half of the direct-store contract).
            // The outer gate above already ran `mcp:store.<verb>:call`; the verb re-runs the
            // per-table `store:<table>:write` gate inside. A write lands at depth 0, so it flows
            // through the undo auto-capture wrapper like every other store mutation.
            crate::call_store_mutate_tool(&node.store, principal, ws, qualified_tool, &input)
                .await?
        } else if qualified_tool == "identity.set_credential" {
            // login-hardening scope: set/rotate a user's password hash. Gated `mcp:identity.manage:call`
            // (the outer gate above ran it); the verb hashes argon2 before any write and returns no hash.
            crate::call_credential_tool(&node.store, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("identity.") {
            // The other identity directory verbs (create/get/list/workspaces), reachable over the same
            // bridge as their dedicated admin REST routes. Each re-checks `mcp:identity.manage:call`.
            crate::call_identity_tool(&node.store, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("bus.") {
            crate::call_bus_tool(&node.bus, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool.starts_with("reminder.") {
            crate::call_reminder_tool(node, principal, ws, qualified_tool, &input).await?
        } else if qualified_tool == "undo"
            || qualified_tool == "redo"
            || qualified_tool.starts_with("history.")
        {
            call_undo_tool(node, principal, ws, qualified_tool, &input).await?
        } else {
            let out = call_ingest_tool(&node.store, principal, ws, qualified_tool, &input).await?;
            // Parity with the gateway's `POST /ingest` route (routes/ingest.rs): after the durable
            // write+drain, publish each committed sample onto its series motion subject so a live
            // subscriber (a dashboard widget, or the `GET /series/{s}/stream` SSE) advances without
            // polling (state vs motion, rule 3). Without this, the MCP `ingest.write` path was
            // durable-only — a sample written via MCP never surfaced on the live feed. Best-effort:
            // a publish failure never fails the durable write. Generic + domain-free; no CE knowledge.
            if qualified_tool == "ingest.write" {
                publish_ingest_motion(node, principal, ws, &input).await;
            }
            out
        };
        return serde_json::to_string(&out).map_err(|e| ToolError::Extension(e.to_string()));
    }

    // An `<ext>.<tool>` target: build the guest's host-callback context so its backend can call
    // host tools under its DELEGATED, INTERSECTED authority (host-callback scope).
    let ctx = build_call_context(node, principal, ws, qualified_tool, depth).await;

    // depth > 0 means this call ORIGINATED from a guest's host-callback (re-entrant): dispatch must
    // not block on the instance lock (it may be the in-flight guest's own) — fail fast instead.
    lb_mcp::call_with_ctx(
        &node.registry,
        &node.bus,
        principal,
        ws,
        qualified_tool,
        input_json,
        ctx,
        depth > 0,
    )
    .await
}

/// Build the [`CallContext`] for a wasm guest call: derive the effective principal
/// `caller ∩ install-grant` and wrap it in a [`Bridge`] the guest's `host.call-tool` dispatches
/// through. Returns `None` when the target isn't an installed extension in this workspace (a routed
/// remote, or an ext with no install record) — the callback is simply unavailable, never widened.
async fn build_call_context(
    node: &Arc<Node>,
    caller: &Principal,
    ws: &str,
    qualified_tool: &str,
    depth: u32,
) -> Option<CallContext> {
    let ext_id = qualified_tool.split_once('.').map(|(e, _)| e)?;
    // The install grant (`requested ∩ admin_approved`, persisted at install) for THIS ext in THIS
    // workspace — the upper bound on what the guest's callback may reach.
    let install = read_install(&node.store, ws, ext_id).await.ok().flatten()?;
    // effective = caller ∩ install-grant: `derive` sets the grant as caps and the caller's caps as
    // the constraint, so `caps::check` enforces the intersection both ways. The sub records the ext
    // acted on the caller's behalf; the workspace is inherited (delegation never crosses the wall).
    let effective = caller.derive(format!("ext:{ext_id}"), install.granted.clone());
    let bridge = Bridge::new(Arc::clone(node), effective, ws);
    Some(CallContext {
        bridge: Arc::new(bridge),
        depth,
        // Stamp the ROUTED caller (not the derived-for-the-ext principal) — the sidecar must learn
        // who the human/agent behind the call is, to attribute its row-filter decision. The wasm
        // guest ignores this; the native adapter serializes it into the frame. Projected here from
        // the already-authorized principal — a read, no new trust (native-caller-identity scope).
        caller: Some(Caller {
            sub: caller.sub().to_string(),
            ws: caller.ws().to_string(),
            role: role_wire(caller.role()).to_string(),
            delegated: caller.owner_sub() != caller.sub(),
            // Admin is caps-based, not the (cosmetic) role enum (native-caller-identity scope).
            admin: crate::authz::caps_hold_admin(caller.caps()),
        }),
    })
}

/// The lower-cased wire spelling of a role (matches `#[serde(rename_all = "kebab-case")]` on
/// `lb_auth::Role`), so a native child reads the same token the gateway would serialize. Kept beside
/// the one `CallContext` construction that needs it; the native-tier dual lives in `native::caller`.
fn role_wire(role: lb_auth::Role) -> &'static str {
    match role {
        lb_auth::Role::SuperAdmin => "super-admin",
        lb_auth::Role::WorkspaceAdmin => "workspace-admin",
        lb_auth::Role::Member => "member",
    }
}

/// Dispatch the durable-workflow host verbs a federated page (or a wasm guest, via the host callback)
/// reaches through the bridge. Two families:
///   - **reads/resolve:** `outbox.status`, `inbox.list`, `inbox.resolve`.
///   - **writes that PRODUCE motion** (proof-workflow-sim scope): `inbox.record` (create an item),
///     `outbox.enqueue` (stage a pending effect) — so a guest can drive a full inbox→approval→outbox
///     round-trip, not just read one something else seeded.
/// Each host verb re-authorizes internally (workspace-first, then `mcp:<verb>:call`); a denial is
/// opaque (`ToolError::Denied`), indistinguishable from a missing tool. Both `inbox.record`'s author
/// and `inbox.resolve`'s actor are forced to the principal's `sub` — never caller-supplied (a guest
/// cannot forge another source's authorship/sign-off).
async fn call_inbox_outbox_tool(
    node: &Node,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "outbox.status" => {
            let status = outbox_status(&node.store, principal, ws)
                .await
                .map_err(|_| ToolError::Denied)?;
            serde_json::to_value(status).map_err(|e| ToolError::Extension(e.to_string()))
        }
        "inbox.list" => {
            let channel = input
                .get("channel")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::BadInput("missing arg: channel".into()))?;
            let items = list_inbox(&node.store, principal, ws, channel)
                .await
                .map_err(|_| ToolError::Denied)?;
            Ok(json!({ "items": items }))
        }
        "inbox.record" => {
            let channel = input
                .get("channel")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::BadInput("missing arg: channel".into()))?;
            let body = input.get("body").and_then(|v| v.as_str()).unwrap_or("");
            // The item id is caller-supplied for idempotency; default to a channel-scoped stable id so
            // a guest that omits one still upserts deterministically (no wall-clock in core).
            let id = input
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::BadInput("missing arg: id".into()))?;
            let ts = input.get("ts").and_then(|v| v.as_u64()).unwrap_or(0);
            // author is FORCED to the principal's sub inside record_inbox — `author` in the input is
            // ignored (never caller-spoofable).
            record_inbox(&node.store, principal, ws, channel, id, body, ts)
                .await
                .map_err(|_| ToolError::Denied)?;
            Ok(json!({ "ok": true }))
        }
        "outbox.enqueue" => {
            let id = input
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::BadInput("missing arg: id".into()))?;
            let target = input
                .get("target")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::BadInput("missing arg: target".into()))?;
            let action = input
                .get("action")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::BadInput("missing arg: action".into()))?;
            // payload is opaque to the host (the relay's target adapter interprets it); accept a string
            // or stringify a JSON value so a guest can pass either.
            let payload = match input.get("payload") {
                Some(Value::String(s)) => s.clone(),
                Some(v) => v.to_string(),
                None => String::new(),
            };
            let ts = input.get("ts").and_then(|v| v.as_u64()).unwrap_or(0);
            enqueue_outbox(&node.store, principal, ws, id, target, action, &payload, ts)
                .await
                .map_err(|_| ToolError::Denied)?;
            Ok(json!({ "ok": true }))
        }
        // Stage a gated effect in the `held` status — proposed by a rule's `inbox.request_approval`,
        // NOT deliverable until the matching `needs:approval` item is approved (rules-approvals scope).
        // Same cap as `outbox.enqueue` (staging is not the gated step; the *release* is). The caller
        // passes the `needs:approval` item id; the effect id is derived here (`held:{item_id}`) — the
        // SAME derivation the release reactor uses — so the rule never owns the id scheme.
        "outbox.enqueue_held" => {
            let item_id = input
                .get("item_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::BadInput("missing arg: item_id".into()))?;
            let id = crate::held_effect_id(item_id);
            let id = id.as_str();
            let target = input
                .get("target")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::BadInput("missing arg: target".into()))?;
            let action = input
                .get("action")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::BadInput("missing arg: action".into()))?;
            let payload = match input.get("payload") {
                Some(Value::String(s)) => s.clone(),
                Some(v) => v.to_string(),
                None => String::new(),
            };
            let ts = input.get("ts").and_then(|v| v.as_u64()).unwrap_or(0);
            enqueue_held_outbox(&node.store, principal, ws, id, target, action, &payload, ts)
                .await
                .map_err(|_| ToolError::Denied)?;
            Ok(json!({ "ok": true }))
        }
        // The sidecar-drivable relay surface (native-tier delivery): a driver pulls its own due
        // effects, delivers them through its own client, and marks the outcome. See
        // `outbox/relay_ops.rs` for the invariant (never lost, never double-sent).
        "outbox.due" => {
            let target = input.get("target").and_then(|v| v.as_str());
            let now = input.get("now").and_then(|v| v.as_u64()).unwrap_or(0);
            let effects = outbox_due(&node.store, principal, ws, target, now)
                .await
                .map_err(|_| ToolError::Denied)?;
            Ok(json!({ "effects": effects }))
        }
        "outbox.mark_delivered" => {
            let id = input
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::BadInput("missing arg: id".into()))?;
            outbox_mark_delivered(&node.store, principal, ws, id)
                .await
                .map_err(|_| ToolError::Denied)?;
            Ok(json!({ "ok": true }))
        }
        "outbox.mark_failed" => {
            let id = input
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::BadInput("missing arg: id".into()))?;
            let now = input.get("now").and_then(|v| v.as_u64()).unwrap_or(0);
            let status = outbox_mark_failed(&node.store, principal, ws, id, now)
                .await
                .map_err(|_| ToolError::Denied)?;
            Ok(json!({ "status": status }))
        }
        "inbox.resolve" => {
            let item_id = input
                .get("item_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::BadInput("missing arg: item_id".into()))?;
            let decision: Decision =
                serde_json::from_value(input.get("decision").cloned().unwrap_or(Value::Null))
                    .map_err(|e| ToolError::BadInput(format!("decision: {e}")))?;
            // Logical ordering timestamp (no wall-clock in core); the page supplies it. Idempotent on
            // `item_id` regardless — re-resolving upserts, last decision wins.
            let ts = input.get("ts").and_then(|v| v.as_u64()).unwrap_or(0);
            resolve_inbox(&node.store, principal, ws, item_id, decision, ts)
                .await
                .map_err(|_| ToolError::Denied)?;
            Ok(json!({ "ok": true }))
        }
        _ => Err(ToolError::NotFound),
    }
}

/// Dispatch the undo-journal verbs (`undo`, `redo`, `history.list`, `history.compensations`) the UI
/// reaches for its undo/redo affordance (undo scope). Each gates on its own MCP cap, plus the
/// no-escalation check (the original tool's cap) and `undo.any` for another actor's stack — all
/// inside the service layer. `actor` defaults to the caller's own `sub` (you undo your own stack);
/// `surface` defaults to the empty (per-(ws,actor)) stack.
///
/// The *surfaced* refusals — `Stale` ("the record changed since this step") and `NotUndoable`
/// (irreversible, with any declared compensation) — are returned as structured JSON outcomes, NOT
/// opaque denials: the UI must distinguish "you can't" (denied) from "this step can't be undone
/// right now" (a normal, explainable result). A true authorization failure stays opaque `Denied`.
async fn call_undo_tool(
    node: &Node,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    // `actor` defaults to the caller; a different actor triggers the `undo.any` gate in the service.
    let actor = input
        .get("actor")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| principal.sub());
    let surface = input.get("surface").and_then(|v| v.as_str()).unwrap_or("");

    match qualified_tool {
        "undo" => undo_outcome(undo(&node.store, principal, ws, actor, surface).await),
        "redo" => undo_outcome(redo(&node.store, principal, ws, actor, surface).await),
        "history.list" => {
            let items = history_list(&node.store, principal, ws, actor, surface)
                .await
                .map_err(undo_svc_to_tool_err)?;
            Ok(json!({ "items": items }))
        }
        "history.compensations" => {
            let seq = input
                .get("step")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| ToolError::BadInput("missing arg: step".into()))?;
            let comp = history_compensations(&node.store, principal, ws, seq)
                .await
                .map_err(undo_svc_to_tool_err)?;
            Ok(json!({ "compensation_tool": comp }))
        }
        _ => Err(ToolError::NotFound),
    }
}

/// Turn an `undo`/`redo` result into a UI-shaped JSON outcome. A success reports the reversed step;
/// the surfaced refusals report `ok:false` with a reason the UI can render; a true denial stays
/// opaque.
fn undo_outcome(result: Result<lb_undo::JournalEntry, UndoSvcError>) -> Result<Value, ToolError> {
    match result {
        Ok(entry) => Ok(json!({ "ok": true, "step": entry.seq, "tool": entry.tool })),
        Err(UndoSvcError::Stale) => Ok(
            json!({ "ok": false, "reason": "stale", "message": "the record changed since this step — undo refused" }),
        ),
        Err(UndoSvcError::NotUndoable { compensation_tool }) => Ok(json!({
            "ok": false,
            "reason": "not_undoable",
            "compensation_tool": compensation_tool,
        })),
        Err(UndoSvcError::Empty(what)) => {
            Ok(json!({ "ok": false, "reason": "empty", "message": format!("nothing to {what}") }))
        }
        Err(e) => Err(undo_svc_to_tool_err(e)),
    }
}

/// Map a service error to the MCP error. `Denied` is opaque; everything else is an extension error.
fn undo_svc_to_tool_err(e: UndoSvcError) -> ToolError {
    match e {
        UndoSvcError::Denied => ToolError::Denied,
        other => ToolError::Extension(other.to_string()),
    }
}

/// Look up the declared JSON-Schema `input_schema` for `qualified_tool`, for the defense-in-depth
/// arg validation the dispatcher runs (channels-command-palette scope). Host-native verbs are read
/// from the in-code descriptor table (`tools::host_descriptors`); extension tools from the runtime
/// registry. `None` when the tool declares no schema (validation is then skipped — additive).
fn descriptor_schema(node: &Node, qualified_tool: &str) -> Option<serde_json::Value> {
    // `reminder.create`'s descriptor schema is FORM-SHAPED (flat `action_kind` + per-kind fields). The
    // verb now accepts BOTH that flat form AND the nested `action:{kind,…}` wire form (backward compat),
    // so schema-gating here would wrongly reject the nested callers (they carry `action`, not the
    // `action_kind` the form schema requires). The verb's own handler is authoritative (it accepts
    // either shape via `create_action`). This is the one descriptor whose form and wire shapes differ.
    if qualified_tool == "reminder.create" {
        return None;
    }
    for d in crate::tools::host_descriptors() {
        if d.name == qualified_tool {
            return d.input_schema;
        }
    }
    let (ext_id, tool) = qualified_tool.split_once('.')?;
    for (id, descriptors) in node.registry.descriptor_entries() {
        if id == ext_id {
            if let Some(d) = descriptors.into_iter().find(|d| d.name == tool) {
                return d.input_schema;
            }
        }
    }
    None
}

/// Publish `ingest.write` samples onto their series motion subjects after the durable write — the
/// live-feed half of `ingest.write` over the MCP bridge, mirroring the gateway's `POST /ingest` route.
/// The producer is stamped to the authenticated principal (matching what `ingest_write` commits), so a
/// live frame is consistent with the committed `series` row. Best-effort (state vs motion, rule 3): a
/// malformed sample or a publish failure is skipped and never fails the call. Domain-free.
async fn publish_ingest_motion(
    node: &Node,
    principal: &lb_auth::Principal,
    ws: &str,
    input: &Value,
) {
    let Some(samples) = input.get("samples").and_then(Value::as_array) else {
        return;
    };
    for raw in samples {
        let Ok(mut sample) = serde_json::from_value::<crate::ingest::Sample>(raw.clone()) else {
            continue;
        };
        sample.producer = principal.sub().to_string();
        let _ = publish_sample(&node.bus, ws, &sample).await;
    }
}
