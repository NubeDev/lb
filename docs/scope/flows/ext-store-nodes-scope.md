# Flows scope — the ext & store node pack (call any extension API, CRUD the store, safely)

Status: **shipped (green), awaiting release** — lb backend done & tested (tag `node-v0.10.0`
pending), rubix-ai/ui pickers done & tested (pin bump pending). Session:
[`sessions/flows/ext-store-nodes-session.md`](../../sessions/flows/ext-store-nodes-session.md).
Promotes to `public/flows/flows.md` on release.

> Read the spine first: [`flows-scope.md`](./flows-scope.md) owns the canonical **Decisions
> (v1)**; [`node-descriptor-scope.md`](./node-descriptor-scope.md) owns the descriptor shape
> every node here wears. Like [`data-nodes-scope.md`](./data-nodes-scope.md), this is a
> *content + one guard* ask, not a new mechanism: **five new built-in node descriptors** in the
> existing mold, **three new editor picker formats**, and **one genuinely new host guard**
> (reserved-table write protection on the store mutate surface).

A flow today can call any MCP verb only through the generic `tool` node, whose `verb` field is
**free text** — the author must already know the exact `<ext>.<tool>` string and hand-type the
args JSON. And a flow that wants to read or write a store table must hand-write `store.query`
SQL or a raw `store.write` call — with **nothing** stopping it from overwriting `flow`,
`install`, or `dashboard` rows and bricking the node, because no reserved-table concept exists
today (`store:*:write`, held by the editor role, satisfies the per-table gate for *every*
table). This scope ships (a) first-class **extension nodes** — list installed extensions, and
call any extension's API with a picker-driven form (pick the extension → pick its tool → the
args form renders from that tool's own schema); (b) first-class **store CRUD nodes** — read /
write / delete rows on a picked table; and (c) the **reserved-table wall** that makes (b) safe
to hand to a flow author.

**Owning repos.** The node descriptors, executors, the reserved-table guard, and the
`store.tables` changes land **upstream in `lb`** (released as a `node-v*` tag). The editor
pickers land in the **product shell** (`rubix-ai/ui` — `lb/ui` is retired), which then bumps
its lb pin to the released tag. One feature, two repos, the standard `WORKFLOW-LB.md` flow.

## Goals

- **`ext-list`** — a built-in node returning the workspace's installed extensions (the
  `ext.list` rows: id, version, tier, enabled, running, health), so a flow can branch on
  "is modbus running?" without an agent in the loop.
- **`ext-call`** — a built-in node that calls **any** extension tool with a fully picker-driven
  editor UX: an **extension dropdown** (from `ext.list`), a **tool dropdown scoped to that
  extension** (from `tools.catalog`, which already advertises only what the caller may call),
  and an **args form rendered from the selected tool's own `input_schema`** — no hand-typed
  verb string, no hand-typed args JSON. This is the "get/set/delete against an extension"
  ask, generalized (see the decision below).
- **`store-read` / `store-write` / `store-delete`** — built-in CRUD nodes over a **picked
  table**: read rows by id or filter, upsert `{table, id, value}`, delete `{table, id}` —
  dispatching the existing `store.query` / `store.write` / `store.delete` verbs under the
  caller's caps.
- **The reserved-table wall.** One canonical, host-owned reserved-table set; `store.write` and
  `store.delete` **reject** a reserved table regardless of capability grants (even
  `store:*:write`); `store.tables` flags each row `system: true|false`; the writable-table
  picker excludes system tables. A flow physically cannot brick the platform through the store
  mutate surface.
- **Three new picker formats** in the schema-form renderer — `lb:extension`, `lb:ext-tool`,
  `lb:store-table` / `lb:store-table-writable` — as opaque JSON-Schema `format` hints in the
  exact `lb:datasource` mold. No per-node-type branch in the form renderer.

## Non-goals

- **No new execution mechanism.** Every node here dispatches through the one `call_tool`
  chokepoint under the caller's principal, exactly as the `tool` node does
  (`host/src/flows/execute_node/core.rs`). No new gate, no new transport, no new WIT.
- **No new MCP verbs and no new capabilities.** The verbs (`ext.list`, `tools.catalog`,
  `store.query`, `store.write`, `store.delete`, `store.tables`) and their caps all exist. The
  only surface changes are *inside* existing verbs: the reserved-table reject and the
  `system` flag on `store.tables` rows (both additive/narrowing, detailed below).
- **No per-extension anything** (CLAUDE rule 10). No node, picker, or code path names a
  specific extension. `ext-call` reaches every extension identically through the generic
  catalog; swapping an extension changes zero core code.
- **No SQL-builder mini-language.** `store-read`'s filter is a flat field-equality map + limit
  + order — the 80% case. A flow needing a real query uses the `tool` node with `store.query`
  (that door stays open); a query *language* in a node config is the templating-mini-language
  smell Decision 4 already rejected.
- **Not schema-designer territory.** Table *creation*/migration, the `lb:table` (dbschema)
  picker, and datasource tables stay with
  [`../datasources/schema-designer-scope.md`](../datasources/schema-designer-scope.md). These
  nodes operate on existing store tables only.
- **No agent/AI surface.** These are canvas nodes; the AI plane already reaches the same verbs
  directly.

## Intent / approach

**Decision — one generic `ext-call`, not three verb-suffix nodes.** The literal ask was
"nodes to get/set/delete as APIs to extensions". Shipping `ext-get`/`ext-set`/`ext-delete` as
separate node types would require classifying an extension's tools into CRUD buckets — and the
`ToolDescriptor` carries no such classification, so the only classifier would be a **tool-name
naming convention** (`.get`/`.set`/`.delete` suffixes). That is a rule-10 leak in spirit: core
behaviour keyed off what an extension happens to name things, silently wrong for any extension
that doesn't follow the convention. Instead we ship **one `ext-call`** whose UX delivers what
the three nodes were really for — *no typing, pick everything* — and covers every extension
API (get, set, delete, and everything else) uniformly. `ext-list` stays its own node because
"enumerate what's installed" is a genuinely different shape (no target extension). *Rejected:*
the three-node convention split, for the reason above; revisit only if the manifest ever grows
a first-class CRUD classification on `[[tools]]`.

**Decision — store CRUD is three nodes, not one.** Read, write, and delete have different
ports, different failure modes, and different picker posture (read shows every table;
write/delete show only non-system tables). Folding them into one node with a `mode` switch
makes the form conditional and the wiring ambiguous. Three small descriptors in the
`data-nodes` mold, each executor its own file (FILE-LAYOUT).

**Decision — the reserved-table wall is a host-side reject, not a capability convention.**
The capability grammar already supports `store:<table>:write`, but the editor role's
`store:*:write` wildcard satisfies it for every table — so "just don't grant it" does not
protect `flow`/`install`/`dashboard` from any editor-held flow today. Protection that depends
on every deployment curating grants correctly is not protection. The wall is a **hard reject
in the store mutate surface** (`store_mutate/run.rs`), checked *before* the capability gate,
returning a typed `ReservedTable { table }` error. The host's own internals are unaffected —
they write through the direct `Store` handle, not through `store.write`. There is **no
override capability**: a legitimate need to mutate a system table is an admin/host feature
(packs, migrations, the owning verb family), never a flow. *Rejected:* an
`store:reserved:<table>:write` escape-hatch cap (an invitation to grant it once and forget)
and a UI-only filter (the picker hides it but the verb still obeys — theater).

**Decision — the reserved set is one canonical module in `lb`.** Today the host-owned table
names are scattered across `TABLE` consts (`flows/src/lib.rs`'s `table` module is the only
canonicalized family). We introduce **one** module — `store/src/reserved.rs`,
`reserved::is_reserved(table) -> bool` over a single const slice — listing every host-owned
table: the flow family (`flow`, `flow_run`, `flow_step_output`, `flow_node_state`,
`flow_input`, `flow_trigger_state`, `flow_node_memory`, `flow_node_buffer`), install/registry
(`install`, `registry_catalog`, `registry_cache`, `native_status`, `pack_receipt`),
dashboard/UI (`dashboard`, `panel`, `nav`, `nav_pref`, `nav_hidden`,
`workspace_nav_default`, `ui_layout`, `channel_registry`, `channel_chart_pref`,
`render_template`, `report`, `brand`), auth/identity (`workspace`, `user`, `apikey`,
`credential`, `member`, `share`, `webhook`), agent/rules (`agent_definition`, `agent_memory`,
`agent_policy`, `agent_decision`, `workspace_agent_config`, `persona`, `rule`, `insight`,
`insight_occ`, `approval_held_change`, `proof_sim_change`), and data/media (`doc`, `asset`,
`media`, `media_chunk`, `datasource`, `db_schema`, `extraction`, `query`, `device`,
`push_delivered`). Owning modules that already export a `TABLE` const are refactored to
*reference* their name from (or assert it against) the reserved module so the list cannot
drift silently — a unit test walks the known `TABLE` consts and asserts membership.
Extension-owned and user/pack tables (e.g. `site`, `point_reading`, `ems_*`) are **not**
reserved — that is exactly the data these nodes exist to CRUD.

**Decision — pickers are `format` hints, resolved editor-side.** The schema-form renderer
already dispatches dynamic pickers off the JSON-Schema `format` keyword
(`format: "lb:datasource"` → a live dropdown; `rubix-ai/ui/src/features/flows/SchemaForm.tsx`,
`SchemaFormPickers.tsx`). We add three formats in that exact mold — the descriptor stays pure
JSON-Schema 2020-12 (a `format` an editor doesn't know degrades to a text input; host-side
`jsonschema` validation is untouched), and the renderer stays free of node-type branches:

- `lb:extension` — dropdown from the existing `listExtensions()` (`ext.list`): label
  `id vX (health)`, value the ext id.
- `lb:ext-tool` — dropdown from `tools.catalog`, filtered to tools whose qualified name is
  prefixed by the **sibling `ext` field's** current value. `tools.catalog` already runs the
  authorize gate per tool, so the dropdown shows only what this author may call — the deny is
  visible at authoring time, and re-checked at run time. Selecting a tool also hands the form
  the tool's `input_schema`.
- `lb:store-table` / `lb:store-table-writable` — dropdown from `store.tables`; the
  `-writable` variant excludes `system: true` rows (used by `store-write`/`store-delete`);
  the plain variant shows all with a `system` badge (used by `store-read`). Two format
  strings rather than a custom `x-*` parameter, so the schema stays annotation-free and the
  ajv gate stays clean.

The **`ext-call` args sub-form** is the one renderer capability addition: when the sibling
`tool` field names a catalog tool with an `input_schema`, the `args` object field renders as a
**nested `SchemaForm` over that tool's schema** (the same renderer, recursively, same
unsupported-shape fail-loud posture) instead of a raw JSON textarea; a tool without an
`input_schema` falls back to the JSON editor. Sibling-field awareness (ext → tool → args) is
the same pattern the dbschema table picker already stubbed ("thread the sibling schema name
through") — this scope pays that debt properly with a small typed context the form passes to
picker inputs, instead of a one-off.

**Decision — `store.tables` opens to flow authors.** The table picker needs `store.tables`,
which is admin-gated today. It reveals table names + row counts only — acceptable for anyone
already holding the editor role (who can `store.query` arbitrary SQL anyway). We add
`mcp:store.tables:call` to the editor role bundle. *Rejected:* a second names-only verb
(duplicate surface for the same data).

## The five descriptors

All in `flows/src/builtins/` (a new `platform.rs`, in the merged registry alongside
`core`/`data`/`parse`/…), all host-resolved, all speaking the `{payload, topic}` envelope
(Decision 6). Executors in `host/src/flows/execute_node/` (`ext_call.rs`, `store_crud.rs`),
each dispatching through `call_tool_node` under the caller's principal.

| `type` | `kind` | in | out | config (JSON-Schema sketch) | dispatches |
|---|---|---|---|---|---|
| `ext-list` | transform | `payload` (ignored trigger) | `payload` = the `extensions` array | `{ running_only?: bool }` | `ext.list` |
| `ext-call` | transform | `payload` (deep-merged into `args`, the `tool`-node rule) | `payload` = tool result | `{ ext: string ⟨format lb:extension⟩ (req), tool: string ⟨format lb:ext-tool⟩ (req), args: object ⟨rendered from the tool's input_schema⟩, timeout_ms?: int }` | `<ext>.<tool>` |
| `store-read` | transform | `payload` (may carry `id` / filter values) | `payload` = `{rows: [...]}` unwrapped from the `{data, rev}` envelope; single-`id` reads emit `{row}` | `{ table: string ⟨format lb:store-table⟩ (req), id?: string, filter?: object (flat field=value), limit?: int (default 100, max 1000), order_by?: string, desc?: bool }` | `store.query` (a host-built parameterized SELECT — never string-spliced from user text) |
| `store-write` | transform | `payload` (the value when config `value` omitted; may carry `id`) | `payload` = `{table, id}` | `{ table: string ⟨format lb:store-table-writable⟩ (req), id?: string (default: generated), value?: object }` | `store.write` |
| `store-delete` | sink | `payload` (may carry `id`) | — (ack) | `{ table: string ⟨format lb:store-table-writable⟩ (req), id?: string }` | `store.delete` |

Registry count 33 → **38**; the `mod.rs` count assertion moves with it. Config-vs-payload
precedence is uniform: an explicit config field wins; a missing config field reads the
incoming `payload` (so a wire can drive the `id`/`value` dynamically while the table stays
pinned by the author). `store-read` builds its SQL host-side from the validated config —
table name checked against `store.tables`-legal identifiers, values bound as parameters —
so the node introduces no injection surface beyond what `store.query` itself already gates.

## How it fits the core

- **Tenancy / isolation.** Nothing new to hold: every dispatched verb is already
  workspace-scoped (`ext.list` walks this ws's installs; store verbs operate in the caller's
  ws; the callback `ws` is host-set, un-spoofable). The nodes hold no durable state
  (stateless, rule 4).
- **Capabilities & the deny path.** No new caps. Each node's dispatch re-enters `call_tool`,
  so the existing two-layer gate applies per node execution: `mcp:<verb>:call` outer plus the
  inner surface cap (`store:<table>:write` for mutate). A runner lacking `mcp:store.write:call`
  has its `store-write` node **denied at that node** — no widening, same as the `tool` node
  today. The named deny for the new guard: `store-write` to table `flow` →
  `ReservedTable{table:"flow"}`, even for a principal holding `store:*:write` and
  `mcp:store.write:call`.
- **Symmetric nodes / placement.** Either role; the nodes run wherever the run's owner node
  runs (Decision 10). No `if cloud`, no `if native` — `ext-call` doesn't know or care whether
  the target extension is wasm or native (that's the install record's business,
  [`extension-nodes-scope.md`](./extension-nodes-scope.md)).
- **One datastore / state vs motion.** No new table, no new record. The reserved module is a
  const list, not state.
- **API / MCP surface (§6.1).** No new verb. Two in-place changes, flagged: (1)
  `store.write`/`store.delete` gain the reserved-table reject — a **narrowing** of an existing
  verb's accepted input, breaking only a caller that was already doing the dangerous thing;
  (2) `store.tables` rows gain `system: bool` — additive.
- **MCP is the contract.** The pickers consume `ext.list` / `tools.catalog` / `store.tables`
  — the same verbs an AI agent or another client would use. No editor-private side channel.
- **SDK/WIT impact.** **None.** No manifest change, no WIT change, no SDK change. The
  `rubix-ai/ui` schema-form work is shell code, not `@nube/ext-ui-sdk` surface (extension UIs
  don't render flow config forms).
- **One responsibility per file.** `store/src/reserved.rs` (the wall), `flows/src/builtins/platform.rs`
  (the five descriptors), `host/src/flows/execute_node/ext_call.rs` + `store_crud.rs` (the
  arms, split further if any nears the line limit), `SchemaFormPickers.tsx` gains the three
  picker inputs (split the file if it outgrows itself), one new `store.tables` UI api wrapper
  in `rubix-ai/ui/src/lib`.

## Example flow

A worked path — *nightly sanity: if the modbus extension is down, log it; else read its device
table, stamp a heartbeat row*:

1. A `trigger` (mode=`cron`) fires nightly.
2. **`ext-list`** (`running_only: false`) emits the install rows; a `switch` routes on
   `payload[?].health` for the row whose `ext == "modbus"` (authored with the picker — the
   author never typed "modbus" into a code field, and core never knew the name).
3. Down-branch: a `store-write` appends `{status:"ext down"}` to the user table
   `ops_heartbeat` (picked from the writable dropdown — `flow`, `install`, `dashboard` were
   never offered).
4. Up-branch: **`ext-call`** — the author picked ext `modbus`, picked its `points.read` tool
   from the scoped dropdown, and filled the args form that rendered from `points.read`'s own
   `input_schema`. The node dispatches `modbus.points.read` under the runner's caps
   (`caller ∩ install-grant` narrowing applies as ever).
5. **`store-read`** on table `site` (`filter: {region: "nsw"}`, limit 50) joins reference
   rows; a `merge` shapes the payload; **`store-write`** upserts the heartbeat row.
6. A later, malicious-or-confused edit points `store-write` at table `flow` by hand-editing
   the saved config JSON (bypassing the picker). The run reaches the node and fails it with
   `ReservedTable{table:"flow"}` — the wall is the verb's, not the UI's.

## Testing plan

Per [`../testing/testing-scope.md`](../testing/testing-scope.md): real store (`mem://`), real
registry, seeded installs — no mocks (CLAUDE §9).

- **Reserved-table wall (the headline).** (a) `store.write` and `store.delete` against every
  name in the reserved set → `ReservedTable`, asserted for a principal holding
  `store:*:write` + the mcp caps (the wildcard does **not** pierce the wall); (b) a non-reserved
  table with the same principal succeeds; (c) the drift test — every known host `TABLE` const
  is a member of the reserved set; (d) host internal writes (e.g. a flow save) still succeed —
  the wall gates the MCP mutate surface only.
- **Capability-deny (mandatory).** Each node type, run by a principal lacking the dispatched
  verb's cap → denied at that node, run reflects the failure, no partial write. `ext-call` to
  a tool outside the install grant → denied (the existing narrowing test, exercised via the
  node).
- **Workspace-isolation (mandatory).** `ext-list` in ws-B omits ws-A's installs; `store-read`
  in ws-B reads none of ws-A's rows for the same table name; `store.tables`' `system` flags
  are identical across workspaces (the set is global, not per-ws).
- **Node round-trips (integration, real flow runs).** `store-write` → `store-read` round-trips
  a row including the `{data, rev}` unwrap; `store-delete` then `store-read` finds nothing and
  delete is idempotent; `ext-call` end-to-end against a seeded test extension tool;
  payload-vs-config precedence (wire-driven `id`/`value` with pinned table).
- **`store-read` SQL construction (unit).** Table-driven: id / filter / limit / order
  combinations produce the expected parameterized query; a filter value containing quotes/`;`
  is bound, never spliced; an invalid table identifier is rejected before dispatch.
- **Picker + form (UI, `rubix-ai/ui` vitest).** The three formats render dropdowns from
  seeded api responses; `lb:ext-tool` re-filters when the sibling `ext` changes and clears a
  now-invalid selection; `lb:store-table-writable` excludes `system` rows; the `ext-call`
  args field renders a nested form from a tool's `input_schema` and falls back to the JSON
  editor without one; an unknown `lb:*` format degrades to a text input (forward-compat).
- **Regression.** Any bug found → `docs/debugging/flows/<symptom>.md` + a regression test.

## Risks & hard problems

1. **The reserved list going stale.** A future host feature adds a table and forgets the
   module → the wall silently doesn't cover it. The drift unit test (every `TABLE` const ∈
   reserved) is the guard; adding a table without touching `reserved.rs` fails CI. Keep the
   test the enforcement, not reviewer memory.
2. **The reject as a narrowing change.** Any existing caller legitimately writing a reserved
   table via `store.write` breaks. Audit before shipping: packs and internal writers use the
   direct `Store` handle, but sweep the repo (and rubix-ai) for `store.write` callers naming
   reserved tables; anything found is refactored to its owning verb family first.
3. **Sibling-field form context.** ext → tool → args and the dbschema stub both need "read a
   sibling field's live value" in the renderer. Design it once as a small typed context; a
   second ad-hoc prop-drill is the `utils.rs` of forms.
4. **`store-read` scope creep.** The flat filter will attract "just add OR / ranges / joins".
   Hold the line: the escape hatch is the `tool` node + `store.query`, which exists precisely
   so this node can stay small.
5. **Catalog latency in the editor.** Three dropdowns each fetch a verb on form open; cache
   per editor session (the datasource picker precedent) so a canvas with ten store nodes
   doesn't refetch `store.tables` ten times.

## Open questions

None — the four structural choices (one generic `ext-call`; three store nodes; host-side
reserved wall with no override cap; `format`-hint pickers with editor-role `store.tables`)
are decided above with their rejections recorded. Anything discovered at build time that
would reopen one of them is a finding to raise against this doc, not a silent divergence.

## Decisions recorded inline (at build time — did not reopen the four structural choices)

- **`ext.list` was not on the MCP bridge — wired the `ext.*` family on by EXACT name.** Build-time
  finding: the `ext-list` node dispatches `ext.list` through the flow chokepoint `call_tool`, but the
  `ext.*` lifecycle family was only ever reachable via the gateway REST route — never registered in
  `host/src/tool_call.rs`'s host-native dispatcher — so the node got "no such tool". This scope's own
  "MCP is the contract" decision already required `ext.list` to be an MCP verb; the fix makes it real.
  Listed by **exact name** in `HOST_NATIVE_EXACT` (`ext.list`/`enable`/`disable`/`uninstall`) rather
  than an `ext.` prefix, so the host does **not** reserve the whole `ext.` namespace against a
  hypothetical extension whose id is `ext` (rule 10). See
  [`debugging/flows/ext-list-node-no-such-tool-not-on-mcp-bridge.md`](../../debugging/flows/ext-list-node-no-such-tool-not-on-mcp-bridge.md).
- **The reserved set is broader than the families this doc enumerated.** The code owns more host
  tables than the "Decision — the reserved set is one canonical module" list named (the full
  authz/identity plane, ingest/series, insight internals, durable motion inbox/outbox/jobs, undo/
  history, prefs/i18n, tags, telemetry). All are in `store/src/reserved.rs`, and the drift test
  (every known `TABLE` const ∈ reserved) is the enforcement. `skill` is deliberately **excluded** (it
  rides its own `store:skill/**` cap grammar, not generic table CRUD) — the drift test documents the
  exception so a future move is a conscious one.

## Related

- Spine & contract: [`flows-scope.md`](./flows-scope.md) (Decisions 6, 10),
  [`node-descriptor-scope.md`](./node-descriptor-scope.md) (descriptor shape, `flows.nodes`),
  [`data-nodes-scope.md`](./data-nodes-scope.md) (the node-pack precedent + `builtins/` split).
- Execution & caps: [`extension-nodes-scope.md`](./extension-nodes-scope.md) (the
  `caller ∩ install-grant` narrowing `ext-call` inherits),
  [`flow-run-scope.md`](./flow-run-scope.md) (per-node failure semantics).
- Pickers: [`../datasources/schema-designer-scope.md`](../datasources/schema-designer-scope.md)
  (owns the `lb:datasource` / `lb:table` format-hint precedent these extend).
- Store surface: [`../store/store-scope.md`](../store/store-scope.md); code:
  `host/src/store_mutate/`, `host/src/dbview/tables.rs`, `host/src/tools/catalog.rs`,
  `host/src/ext/list.rs`, `flows/src/builtins/`, `host/src/flows/execute_node/`;
  UI: `rubix-ai/ui/src/features/flows/SchemaForm.tsx`, `SchemaFormPickers.tsx`.
- Cross-repo: rubix-ai `docs/WORKFLOW-LB.md` (the PR→tag→pin-bump flow this ships through).
