# Flows — node descriptor + merged registry (slice 1)

- Area: flows
- Status: shipped (green)
- Scope: [`scope/flows/node-descriptor-scope.md`](../../scope/flows/node-descriptor-scope.md)
- Spine: [`scope/flows/flows-scope.md`](../../scope/flows/flows-scope.md) — Decisions **1, 3, 6, 7**.
- Session: this file. Next slices: flow-run (slice 2) → extension-nodes (3) → triggers (4).

## What this slice is

The **keystone contract** of the flow engine: the `NodeDescriptor` shape (one shape for built-in
and extension node alike), the additive `[[node]]` manifest block + its validation, the five
built-in descriptors, the **merged `flows.nodes` registry** (built-ins ∪ every installed extension's
validated node blocks — a read-time union over the workspace's `install` records), the JSON-Schema
2020-12 config gate, and the typed `Flow` graph model + DAG math (generalised from the chain `Step`,
the chain binding grammar lifted verbatim). The editor palette + the engine both key off this.

This is the contract everything else consumes. It ships no execution, no run engine — those land in
slice 2 (`flow-run-scope.md`).

## What shipped

### New pure crate `lb-flows` (`crates/flows/`) — one responsibility per file
- `descriptor.rs` — `NodeDescriptor` (type/title/category/kind/tool/inputs/outputs/config_version/
  config) + `NodeKind` (trigger/transform/sink/source). The keystone shape.
- `node_block.rs` — the additive `[[node]]` raw manifest table + `validate_node_block` (the bound
  `tool` must exist in the manifest's `[[tools]]`, the `config` must compile as JSON-Schema 2020-12)
  → a canonical `NodeDescriptor` with global type `<ext_id>.<type>`.
- `builtins.rs` — the five built-in descriptors (`trigger`/`tool`/`rhai`/`subflow`/`sink`) in the
  identical shape, incl. the `trigger` inject `fire|retain` sub-mode (Decision 9) and the `subflow`
  `flows.run` binding (parks on the child run, Decision 11 — detail in slice 2).
- `registry.rs` — `merge_registry(builtins, extension)`: built-ins first, ext sorted by type.
- `config_schema.rs` — `compile_schema` + `validate_config` via the `jsonschema` crate (Boon),
  JSON-Schema 2020-12 (Decision 3). The one dialect the platform standardises config on.
- `model.rs` — the typed `Flow` graph (`Node`/`needs`/`with`/`config` + `version` + `failure_policy`)
  + DAG math (Kahn cycle-detect, indegrees/dependents/frontier), mirroring the chain `Chain::validate`
  verbatim. `version` is the load-bearing pin (Decision 1).
- `binding.rs` — `resolve_bindings`: the chain binding grammar over JSON — whole-value
  `${steps.x.output|findings}` / `${params.y}` / literal, **no templating mini-language**.
- `table.rs` (consts in `lib.rs`) — the SurrealDB tables a flow owns (`flow`/`flow_run`/
  `flow_step_output`/`flow_node_state`/`flow_input`).

### Manifest addition (additive, the §11.2 forever-ish gate)
- `lb-ext-loader` gains the `[[node]]` array-of-tables → `Manifest.nodes: Vec<NodeBlock>`. Each block
  is validated at parse: a dangling `tool` binding or a non-schema `config` is a load-time reject
  (`ManifestError::InvalidNodeBlock`).

### Install record addition (additive, serde-defaulted)
- `lb-assets::Install` gains `nodes: Vec<NodeBlock>` + a `.with_nodes(...)` builder. The wasm
  (`host/install.rs`) and native (`host/native/install.rs`) install paths propagate
  `manifest.nodes` onto the durable install, so `flows.nodes` is a **read-time union** — no new
  table, descriptors ride the existing `install:{ext_id}` record (node-descriptor-scope).

### Host `flows` service (`crates/host/src/flows/`)
- `nodes.rs` — `flows.nodes`: the merged registry for the calling workspace. Derived: walks
  `list_installs`, re-validates each block's config schema + reconstructs the bindable tool set from
  the install's granted `mcp:<ext>.<tool>:call` caps (a node whose tool the install grant omits is
  dropped — it could not run anyway), unions with built-ins. Read-only; `mcp:flows.nodes:call`.
- `mod.rs` — `call_flows_tool` dispatch (the one verb so far; run/CRUD/triggers verbs land in 2/4).
- Wired: `lib.rs` module + re-export, `tool_call.rs` `is_host_native` + dispatch arm.

## How it fits the core (the platform checklist)
- **One datastore / no new persistence** — the registry is a read-time union over `install` records.
  No new table (node-descriptor-scope "Derived, not stored"). ✔
- **Symmetric nodes** — built-in vs extension is data in the union, never an `if native` branch. ✔
- **MCP is the contract** — `flows.nodes` is an MCP verb; editor/agents/extensions read it the same. ✔
- **Workspace is the hard wall** — `list_installs` is ws-scoped; ws-B's registry excludes ws-A's ext. ✔
- **Capability-first** — `mcp:flows.nodes:call` gates the read; the descriptor declares no caps (the
  executing tool's own cap gates run time — `caller ∩ install-grant`, slice 3). ✔
- **One responsibility per file** — every file ≤400 lines, named by concept. ✔

## Testing (real infra, seeded via the real write path — no mocks)
- `lb-flows` **24** unit: descriptor/builtins/registry/node_block (dangling tool, non-schema config,
  optional defaults)/config_schema (required, enum, additionalProperties, non-schema)/model (linear,
  cycle, dangling, self-edge, dup, empty, diamond frontier, builtin-type)/binding (literal, step
  output/findings, missing→null, param, partial-interpolation-as-literal).
- `lb-ext-loader` **16**: incl. `[[node]]` parse + dangling-tool reject + non-schema-config reject.
- `host flows_nodes_test` **5** (real `mem://` store + real caps + real `record_install` write path):
  five-builtins / install→registry / **workspace-isolation** / **capability-deny** /
  grant-omitted-node-dropped.

```
cargo test -p lb-flows -p lb-ext-loader        → 24 + 16 green
cargo test -p lb-host --test flows_nodes_test  → 5 green
cargo build --workspace                        → green
cargo fmt --check                              → clean
```

## Decisions made this slice (consistent with the spine)
- **Descriptor-driven node model.** A flow `Node` instance carries `{id, node_type, config, needs,
  with}` — the typed payload (Trigger/Tool/Rhai/Subflow/Sink) is the descriptor's `kind` + the
  config, NOT a Rust enum on the node. This realises "no hardcoded UI" (the form renders from the
  schema) and "everything keyed off the descriptor". The spine's `Node = Trigger | Tool | …` is the
  conceptual model; the five built-in descriptors ARE those five variants. (Traces to the spine's
  "node model" + node-descriptor-scope "the descriptor is the join".)
- **Bindable tool set reconstructed from granted caps.** The install record persists `granted` caps,
  not the manifest's tool list, so `flows.nodes` recovers the runnable tools from
  `mcp:<ext>.<tool>:call`. A node whose tool lacks an install grant is dropped from the palette (it
  would be denied at run time anyway) — keeps the palette honest without a separate tool-list store.
- `lb-flows` stays pure (serde + serde_json + thiserror + jsonschema only) — no store/bus/jobs/host
  seam, mirroring `lb-rules`/`lb-reminders`. The DAG math is mirrored (not a dep on `lb-rules`) so
  the crate doesn't pull rhai; the binding grammar is lifted into JSON (a flow node output is JSON,
  not `RuleOutput`).

## Open questions / next
- Slice 2 (`flow-run`) builds the run engine over `lb-jobs` on this model: `flow_run` coordinator +
  one `flow-step` job per node, frontier driver ported from chains, CAS exactly-once, version-
  pinning, suspend/resume, `flows.patch_run`, `ResumePointDrift`, subflow-parks-on-child, the
  `flows.*` run MCP surface + `flows.runs.list`, the canonical `coalesce` enum.
- The editor palette filtering (show-but-gate a node whose tool the user lacks) is a Wave-3 canvas
  concern; the host already drops un-runnable nodes here.
