# The host-mediated bridge (`/mcp/call`) cannot dispatch a host-native `series.*` verb

- Area: extensions
- Status: resolved
- First seen: 2026-06-27
- Resolved: 2026-06-27
- Session: ../../sessions/extensions/proof-panel-session.md
- Regression test: rust/crates/host/tests/proof_panel_test.rs (`grant_intersection_denies_the_unapproved_verb_at_the_bridge`, `workspace_isolation_series_and_ping`) + ui/src/features/ext-host/ProofPanel.gateway.test.tsx

## Symptom

A federated extension page reaches platform data through the host-mediated bridge:
`bridge.call("series.find", …)` → `invoke("mcp_call", …)` → `POST /mcp/call` →
`lb_host::call_tool` → `lb_mcp::call`. Building `proof-panel` (whose page lists series via
`series.find` and reads `series.latest`) surfaced that **this path silently could not reach the
series verbs at all**: every bridged `series.*` call resolved to `ToolError::NotFound` (surfaced
as a 403 at the gateway), so the page could never list or read a series — it only ever showed an
error/empty state, no matter what was in the workspace.

## Reproduce

1. Boot a node; seed a real series in workspace `ws`.
2. Call `lb_host::call_tool(&node, &granted_principal, ws, "series.find", r#"{"facets":[…]}"#)`.
3. It returns `Err(ToolError::NotFound)` even though the caller holds `mcp:series.find:call` and
   the series exists.

## Investigation

- `lb_mcp::call` (the single MCP entry) does `authorize → resolve → dispatch`, and `resolve`
  looks the qualified name up in the **runtime `Registry`** (`registry.get(ext_id)`), which holds
  only loaded **wasm/remote extensions**.
- Host-native verbs (`series.*` / `ingest.*` / `tags.*` / asset verbs) are **not** in that
  registry — they are host functions over the embedded store. The gateway serves them through
  **dedicated REST routes** (`GET /series`, `POST /ingest`, …), each calling `lb_host::series_*`
  directly. There is a separate dispatcher, `call_ingest_tool`, that maps `series.*`/`ingest.*` to
  those host verbs — but **nothing on the `/mcp/call` path called it.**
- So the bridge (`/mcp/call` → `call_tool` → `lb_mcp::call`) had no branch for host-native verbs.
  The ui-federation bridge contract is *defined* in terms of `series.find`/`series.latest`, yet
  the one transport a page uses to call them couldn't dispatch them.
- Why it was never caught: the only existing bridge test (`ext_ui_test::bridge_denies_an_ungranted_tool`)
  asserts merely `res.is_err()` — which a `NotFound` satisfies just as well as a `Denied`, masking
  the gap. `ExtHost.gateway.test.tsx` only asserts the *out-of-scope local* rejection, never an
  in-scope forward.

## Fix

`lb_host::call_tool` (the host's bridge entry — the SAME function `POST /mcp/call` forwards through)
now recognizes host-native `series.*`/`ingest.*` verbs and dispatches them over the store: it runs
the **same MCP authorize gate** first (`authorize_tool`, workspace-first then `mcp:<tool>:call`, so
a denied caller stays opaque), then delegates to the existing `call_ingest_tool`. Extension tools
(`<ext>.<tool>`) still route through `lb_mcp::call`/the registry unchanged. No new verb, no WIT
change — only the bridge dispatcher learned to reach the host verbs that already existed.

See `rust/crates/host/src/tool_call.rs`.

## Regression

- Rust: `grant_intersection_denies_the_unapproved_verb_at_the_bridge` calls `series.find` (granted,
  must succeed and list the seeded series) and `series.latest` (ungranted, must be opaque `Denied`)
  through `call_tool`; `workspace_isolation_series_and_ping` proves ws-B's `series.find` sees none
  of ws-A's series through the same entry.
- Frontend: `ProofPanel.gateway.test.tsx` drives the real `makeBridge(scope).call(...)` over a real
  spawned gateway: empty → seed → find lists → latest shows → ungranted denied. Both fail-before
  (NotFound) / pass-after.
