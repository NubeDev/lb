# `ext-list` flow node fails "no such tool" — `ext.list` was never on the MCP bridge

- Area: flows / host dispatch
- Found: ext-store-nodes session (2026-07-25)
- Symptom: the `ext-list` built-in node settles `err: "no such tool"` on every run; `ext-call` and
  the three `store-*` nodes work.

## Symptom

`flows_platform_nodes_test::ext_list_returns_seeded_installs_and_filters_running_only` (and the ws-B
isolation variant) failed:

```
[f_el] err step: {"outcome":"err","error":"no such tool"}
assertion `left == right` failed: left: "err", right: "ok"
```

The other eight platform-node tests passed — `ext-call` dispatches `<ext>.<tool>` (routed through the
runtime registry) and `store-read/write/delete` dispatch `store.query/write/delete` (host-native, in
the dispatcher), so only the `ext.list` leg was broken.

## Root cause

`ext-list` dispatches the `ext.list` verb through the one flow chokepoint `call_tool`. But the
`ext.*` lifecycle family (`ext.list`/`enable`/`disable`/`uninstall`) was **never registered in the
central MCP dispatcher** (`host/src/tool_call.rs`): `ext.` was absent from both `HOST_NATIVE_PREFIXES`
and `HOST_NATIVE_EXACT`, and `run_host_verb` had no `ext.` arm. `call_ext_tool` existed but was only
reached by the **gateway REST route** (the UI calls a dedicated `ext_list` command, not `mcp_call`
with `tool: "ext.list"`), so nothing had exercised `ext.list` over the bridge before a flow node did.

Because `is_host_native("ext.list")` was false, the call fell to the `<ext>.<tool>` routing path
(extension id `"ext"`, tool `"list"`) → no such extension/tool → the host-native fallthrough returned
"no such tool".

## Fix

Wire the `ext.*` family onto the MCP bridge by **exact name** (not an `ext.` prefix — a prefix would
reserve the whole `ext.` namespace against a hypothetical extension whose id is `ext`, a rule-10
smell). In `host/src/tool_call.rs`:

- add `"ext.list"`, `"ext.enable"`, `"ext.disable"`, `"ext.uninstall"` to `HOST_NATIVE_EXACT`;
- add an `else if qualified_tool.starts_with("ext.")` arm in `run_host_verb` → `call_ext_tool`
  (`is_host_native` admits only the four exact verbs, so the arm never sees an extension's own
  `ext.<tool>`; `call_ext_tool` returns `NotFound` for anything else).

Plus the four `ext.*` rows in `system/catalog.rs` (the `host_catalog_covers_dispatch_prefixes` test
asserts every `HOST_NATIVE_EXACT` verb has a catalog entry, and it makes `ext.list` discoverable in
`tools.catalog`).

## Regression test

`flows_platform_nodes_test::ext_list_returns_seeded_installs_and_filters_running_only` +
`ext_list_in_ws_b_omits_ws_a_installs` (both now green) run the `ext-list` node end-to-end through
`flows.run` → `call_tool` → `ext.list`, so a future un-wiring of the family from the bridge fails here.

## Lesson

A host verb "reachable via a gateway route" is **not** the same as "reachable over the MCP bridge" —
the flow engine, AI agents, and native sidecars all dispatch through `call_tool`, not the REST routes.
Any host verb a flow node / agent must call has to be in `HOST_NATIVE_{PREFIXES,EXACT}` with a
`run_host_verb` arm, not just wired to its gateway route.
