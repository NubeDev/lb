# Flows — the ext & store node pack + the reserved-table wall

- Area: flows (+ store, host store-mutate/dispatch, dbview, authz roles)
- Status: shipped (green) — awaiting release (tag `node-v0.10.0` + rubix-ai pin bump), see below
- Scope: [`scope/flows/ext-store-nodes-scope.md`](../../scope/flows/ext-store-nodes-scope.md)
- Spine: [`scope/flows/flows-scope.md`](../../scope/flows/flows-scope.md) — Decisions **6, 10**;
  descriptor shape from [`node-descriptor-scope.md`](../../scope/flows/node-descriptor-scope.md);
  node-pack precedent [`data-nodes-scope.md`](../../scope/flows/data-nodes-scope.md).
- Cross-repo: this feature spans **lb** (backend, this doc) + **rubix-ai/ui** (the editor pickers).

## What this is

Five new built-in flow nodes over the platform's own MCP surface — **`ext-list`** (enumerate installed
extensions), one generic **`ext-call`** (pick ext → pick tool → args form from the tool's own
`input_schema`), and **`store-read`/`store-write`/`store-delete`** over a picked table — plus the one
genuinely new host guard: the **reserved-table wall**. Content + one guard, in the existing molds; no
new MCP verb, no new capability, no new table, no per-extension special-case (rule 10).

## What shipped (lb)

### The reserved-table wall
- **`crates/store/src/reserved.rs`** — one canonical const `RESERVED_TABLES` + `is_reserved(table)`.
  Every host-owned table (flow family, install/registry, dashboard/UI, auth/identity, agent/rules/
  insights, data/media, ingest/series, motion inbox/outbox/jobs, undo, prefs/i18n, tags, telemetry).
  Re-exported from `lb_store` and `lb_host`.
- **`host/src/store_mutate/run.rs`** — `reject_reserved()` runs **before** the capability gate in both
  `store_write_run` and `store_delete_run`, returning a typed `StoreMutateError::ReservedTable{table}`.
  So even the editor bundle's `store:*:write` wildcard cannot pierce the wall, and there is **no
  override cap**. Host internals writing through the direct `lb_store::write` handle are untouched.
- **`host/src/store_mutate/error.rs`** — the `ReservedTable{table}` variant (deliberately **not**
  opaque: the reserved set is a public const, so naming it is author feedback, not a leak).
- **`host/src/store_mutate/tool.rs`** — maps `ReservedTable` → `ToolError::BadInput("reserved table:
  <t>")` over the MCP bridge (a clear 400, not the opaque 403 deny).
- **`host/src/dbview/tables.rs`** — `store.tables` rows gain `system: bool` (⇔ `is_reserved`), a
  global const property identical across workspaces; the writable picker excludes system rows.
- **`host/src/authz/builtin_roles.rs`** — `mcp:store.tables:call` moved into the editor/member bundle
  (it reveals table names + counts only; an editor holding `store.query` can enumerate them anyway).

### The five descriptors + executors
- **`crates/flows/src/builtins/platform.rs`** — the five `NodeDescriptor`s in the identical built-in
  shape, carrying the `lb:extension`/`lb:ext-tool`/`lb:store-table`/`lb:store-table-writable` picker
  `format` hints. Registry count **33 → 38**; the `builtins/mod.rs` count assertion moved with it.
- **`host/src/flows/execute_node/ext_call.rs`** — `ext-list` (dispatch `ext.list`, host-side
  `running_only` filter) + `ext-call` (dispatch the picked `<ext>.<tool>`, `config.args` deep-merged
  with an object payload via the shared `merge_tool_args` — the `tool` node's exact rule).
- **`host/src/flows/execute_node/store_crud.rs`** — `store-read` builds a **parameterized** SELECT
  host-side (`type::table($tb)`, `$`-bound filter/id values, identifier-checked field names, clamped
  integer limit — never string-spliced from user text) → `store.query`; `store-write` → `store.write`;
  `store-delete` → `store.delete`. Uniform config-vs-payload precedence; `{data, rev}` unwrap on read.
- **`host/src/flows/execute_node/mod.rs`** — dispatch arms for the five types through the one
  `call_tool` chokepoint under the caller's principal (so each node re-checks its dispatched verb's
  cap; a runner lacking it is denied **at that node**, no widening).

### The dispatcher gap I found + fixed (inline decision — see below)
- **`host/src/tool_call.rs`** — added the `ext.*` lifecycle family (`ext.list`/`enable`/`disable`/
  `uninstall`) to `HOST_NATIVE_EXACT` + an `ext.` dispatch branch → `call_ext_tool`. **`ext.list` was
  never wired into the MCP `call_tool` chokepoint** — it was only reachable via the gateway REST route
  (`ext_list` command), so a flow's `ext-list` node dispatching `ext.list` through `call_tool` got
  "no such tool". Fixed so `ext.*` rides the one MCP bridge like every other host-native verb (rule 7
  — "MCP is the contract", the scope's own decision).
- **`host/src/system/catalog.rs`** — the four `ext.*` rows in the static host inventory (the
  `host_catalog_covers_dispatch_prefixes` test asserts every `HOST_NATIVE_EXACT` verb has a catalog
  row, and this makes `ext.list` discoverable in `tools.catalog` for agents too).

## Inline decisions made this session (scope had none open)

1. **`ext.list` reachability — wire the whole `ext.*` family onto the MCP bridge by EXACT name, not
   an `ext.` prefix.** The gap: `ext.list` was gateway-REST-only; the flow node needs it through
   `call_tool`. **Why exact-name (not a prefix):** a `HOST_NATIVE_PREFIXES` entry `"ext."` would
   reserve the *whole* `ext.` namespace, shadowing a hypothetical extension whose id is literally
   `ext` — a rule-10 smell. Listing the four reserved lifecycle verbs by exact name in
   `HOST_NATIVE_EXACT` fixes `ext.list` (and consistently exposes enable/disable/uninstall over the
   bridge, each self-gated) while an extension's own `ext.<other>` still routes to the runtime
   registry. The scope's "MCP is the contract" decision already implied `ext.list` must be an MCP
   verb; this is that decision made real. **Rejected:** an `ext-list`-only side channel (would fork
   the surface the scope explicitly says is shared).

2. **Reserved set is broader than the scope's enumerated list.** The scope named the obvious families;
   the code owns more host tables (the full authz/identity plane, ingest/series, insight internals,
   durable motion, undo/history, prefs/i18n, tags, telemetry). All added, with a **drift test** that
   walks every known `TABLE` const (and literals for crate-private consts) and asserts membership — so
   adding a host table without touching `reserved.rs` fails CI (scope Risk 1). `skill` is deliberately
   **excluded** (it rides its own `store:skill/**` cap grammar, not the generic table CRUD) — the
   drift test documents that exception so a future move is conscious.

## Testing (verbatim, real store `mem://`, real registry, real hello wasm — no mocks)

```
# platform node pack (integration, real flows.save/flows.run):
test result: ok. 10 passed; 0 failed; 0 ignored   (flows_platform_nodes_test)
# reserved-table wall + drift + store.tables system flag + member-role open:
test result: ok. 7 passed; 0 failed; 0 ignored     (store_reserved_wall_test)
# unit:
test result: ok. 100 passed; 0 failed   (lb-store lib — incl. reserved::tests)
test result: ok. 332 passed; 0 failed   (lb-host lib — incl. system::catalog coverage + tool_call)
test result: ok.   3 passed; 0 failed   (lb-flows lib — incl. platform descriptors compile)
# dispatch-adjacent regression (my ext.* bridge change):
test result: ok.   6 passed; 0 failed   (ext_lifecycle_test)
test result: ok.   4 passed; 0 failed   (authz_mcp_dispatch_test)
```

Coverage hits every mandatory scope category: reserved-wall reject for a `store:*:write` holder
(wildcard does NOT pierce), reject over the MCP bridge as `BadInput`, non-reserved table succeeds for
the same principal, host-internal direct writes unaffected, the drift test, `store.tables` system flag
+ member-role open + still-denies-without-cap; capability-deny per node type (store-write/store-read/
ext-list — denied at the node, no partial write); workspace-isolation (ext-list omits other-ws
installs, store-read reads none of other-ws rows for the same table); store round-trips with the
`{data, rev}` unwrap; `store-delete` idempotency; `ext-call` end-to-end against the real hello
component; payload-vs-config precedence; `store-read` SQL construction table + hostile-value binding +
invalid-identifier rejection.

`cargo fmt --all --check`: my changed files are clean (`rustfmt --check` on `tool_call.rs` +
`system/catalog.rs` is empty). NOTE: the workspace-wide check reports **pre-existing** violations in
three files unrelated to this feature and unmodified in HEAD (`crates/assets/src/install/model.rs`,
`crates/ext-loader/src/lib.rs`, `crates/ext-loader/src/manifest.rs`) — left untouched to avoid
clobbering concurrent work; they predate this session.

## rubix-ai/ui side

See the rubix-ai session log. In brief: three picker inputs in the `lb:datasource` mold
(`SchemaFormPickersExt.tsx`, `SchemaFormPickersStore.tsx`), a new `store.tables` api wrapper
(`ui/src/lib/store/store.api.ts`), the nested `ext-call` args sub-form (`SchemaFormArgsField.tsx`),
the one typed **`SiblingContext`** mechanism in `SchemaForm.tsx` (paid the dbschema stub's debt once),
and unknown-`lb:*`-format degrade to a text input. Vitest: **21 passed** (SchemaForm + SchemaFormPickers).

## Release step (awaiting go — do NOT push/tag without the user)

Bottom-up (WORKFLOW-LB.md §4/§5):
1. **lb:** commit these changes on a branch, PR into master, cut tag **`node-v0.10.0`** ("ext & store
   node pack + reserved-table wall").
2. **rubix-ai:** bump `lb-node = { git, tag = "node-v0.10.0" }` in `Cargo.toml`, `cargo update -p
   lb-node`, drop the local `[patch]` in `.cargo/config.toml` (already staged there as the in-flight
   dev override), commit the bump + the UI picker work.

Local `[patch]` (rubix-ai `.cargo/config.toml` → `lb-node = { path = ".../lb/rust/node" }`) is present
now and proves the whole feature end to end against the local lb checkout.
