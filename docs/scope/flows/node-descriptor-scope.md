# Flows scope — the node descriptor (the keystone contract)

Status: scope (the ask). Promotes to `public/flows/flows.md` once shipped.

> Read the spine first: [`flows-scope.md`](./flows-scope.md) owns the canonical **Decisions
> (v1)** this doc references by number. This doc owns **the keystone contract**: the *node
> descriptor* — the single shape that describes a flow node to the editor and the engine,
> whether it's built into the host or contributed by an extension. Get this shape right and the
> palette, the settings form, the binding grammar, and the execution path all fall out of it;
> get it wrong and every sibling has to special-case.

A flow node is described by exactly one **descriptor**: a `type`, human metadata, named
**ports**, and an inline **config JSON-Schema** the editor renders a form from. The five
**built-in** node kinds ship that descriptor from the host; an extension ships the *same* shape
in an additive `[[node]]` block in its `extension.toml`. The editor reads one merged
**registry** (`flows.nodes`) and treats built-ins and extension nodes uniformly — there is no
"is this native?" branch in the palette. This doc fixes the descriptor fields, the `[[node]]`
manifest block, the built-in descriptors, the binding grammar on ports, the merged-registry
verb, the schema dialect, and the `config_version` evolution discipline.

## Goals

- One **descriptor shape** describing every flow node — built-in and extension — uniformly, so
  the editor renders palette + settings form from data, never from per-node hardcoded UI.
- An additive **`[[node]]` block** in `extension.toml` (alongside the existing `[[tools]]`),
  letting an extension contribute backend node types. This is the **only** manifest addition.
- Five **built-in descriptors** (Trigger / Tool / Rhai / Subflow / Sink) exposed in the same
  shape from the host, so the editor never distinguishes "ships-with-host" from "from-an-ext".
- **Named ports** carrying the chain **binding grammar verbatim** — whole-value `${…}`
  references or a literal, no templating mini-language.
- A read-only **`flows.nodes`** MCP verb returning the merged registry (built-ins ∪ every
  installed extension's `[[node]]` descriptors) the editor palette renders from.
- A settled **config-schema dialect** (JSON-Schema 2020-12) validated host-side *and* editor-side,
  plus a **`config_version`** discipline that survives schema evolution against version-pinned runs.

## Non-goals

- **Execution.** How a node *runs* (the `tool.call` dispatch, the `caller ∩ install-grant`
  callback, the deny matrix) lives in [`extension-nodes-scope.md`](./extension-nodes-scope.md)
  and [`flow-run-scope.md`](./flow-run-scope.md). This doc is the *contract*, not the runner.
- **The canvas.** Palette rendering, the form widget, draft-vs-pinned UX →
  [`flows-canvas-scope.md`](./flows-canvas-scope.md). This doc gives that canvas its data.
- **A new WIT world or SDK surface beyond `[[node]]`.** Node execution reuses the frozen
  `tool.call` / `host.call-tool` (see SDK/WIT below). No new persistence: the registry derives
  from existing `install` records.
- **A bespoke schema dialect or per-node UI grammar.** Decision 3 settled JSON-Schema; we render
  a form from it, not a custom layout language.

## Intent / approach

**The descriptor is the join.** A flow node needs four things to be both *editable* and
*runnable*: an identity (`type`), human labels (`title`, `category`), a structural contract
(`kind`, `inputs`, `outputs`), and a config contract (`config` schema + `config_version`). Bind
each node to **one** executing `[[tools]]` entry and the editor has everything to draw the
palette, draw the wires, and render the form — while the engine has everything to dispatch and
validate. One shape, two consumers (editor + engine), zero duplication.

The chosen approach is **make built-ins wear the same descriptor as extensions.** The host
synthesises a descriptor for each of its five kinds and returns them through `flows.nodes`
alongside extension descriptors. *Rejected:* a separate "built-in catalog" the editor merges
client-side — it forks the palette code into two render paths and lets the two shapes drift. One
registry, one shape, one renderer.

For config we adopt **JSON-Schema 2020-12 inline in the manifest** (Decision 3): the
dashboard's widget-builder *did not* settle a config-schema convention, so **flows sets it for
the platform** — same dialect, same validators (`jsonschema` host-side, `ajv` editor-side),
everywhere a record carries a user-authored config. *Rejected:* letting each surface pick its own
(guarantees divergence the first time a node config feeds a dashboard widget).

## The `[[node]]` manifest block

Additive alongside the existing `[extension] / [runtime] / [capabilities] / [[tools]] /
[visibility]` shape (`../extensions/extensions-scope.md`). An extension declares one `[[node]]`
per backend node type it contributes; each **binds to one `[[tools]]` entry** that executes it.

```toml
# extension.toml — additive [[node]] block (everything above is the existing shape)
[[tools]]
name        = "publish"
description = "Publish a payload to an MQTT topic."

[[node]]
type        = "mqtt_publish"        # unique within this ext; flow node type = "<ext_id>.<type>"
title       = "MQTT Publish"        # palette + node-header label
category    = "Messaging"           # palette group
kind        = "sink"                # "trigger" | "transform" | "sink" | "source"
tool        = "publish"             # binds to a [[tools]] name in THIS manifest — must exist
inputs      = ["payload"]           # named input ports (edges land here)
outputs     = ["ack"]               # named output ports (edges leave here)
config_version = 1                  # integer; bumped when the schema below changes shape

# Inline JSON-Schema 2020-12 (Decision 3). The editor renders the settings form from this;
# the host validates a node's saved config against it. No external schema file ref.
[node.config]
type        = "object"
required    = ["topic"]
additionalProperties = false
[node.config.properties.topic]
type        = "string"
title       = "Topic"
[node.config.properties.qos]
type        = "integer"
enum        = [0, 1, 2]
default     = 0
```

Field rules that bite:

- **Required vs optional.** Only `type`, `kind`, `tool`, and `[node.config]` are **required**;
  `title`, `category`, `inputs`, `outputs`, and `config_version` are **optional** with defaults
  (`title` ← `type`, `category` ← `"General"`, `inputs`/`outputs` ← `[]`, `config_version` ← `1`).
  So a minimal node is four lines plus its schema (see the `mqtt.in`/`mqtt.out` example in
  `extension-nodes-scope.md`); the full set above is for nodes that want palette polish and named
  ports. *Rejected:* making every field mandatory — it taxes the common case (a node with one
  input and one output) with ceremony for no contract benefit.
- **`type` is ext-unique; the global node type is `<ext_id>.<type>`.** This namespaces the
  palette the same way `mcp:<id>.*` namespaces caps — two extensions can both ship a `publish`.
- **`tool` must name a `[[tools]]` entry in the *same* manifest.** A `tool` that points at a
  non-existent tool name is a **load-time reject** (the manifest is incoherent — a node that
  can't execute). One node → one tool keeps "what runs this?" unambiguous; *rejected:* letting a
  node fan out to several tools (that's a sub-flow, not a node).
- **`kind` is the editor's coarse class** (palette grouping + wiring affordances): a `trigger`
  has no inputs, a `sink` no outputs, a `source` host-arms a series (Decision 2). It does **not**
  pick the runner — the bound `tool` does.
- **`config` is inline JSON-Schema 2020-12**, validated as a schema at load (a `config` that
  isn't valid JSON-Schema is a reject) and used to validate node config instances at save.

## The five built-in descriptors

These ship **with the host**, not via any manifest, but expose the **identical descriptor
shape** so the editor renders them through the same palette path. They map onto the spine's node
model (`flows-scope.md` "The node model"):

| Built-in `type` | `kind` | `tool` binding | ports | `config` schema (shape) |
|---|---|---|---|---|
| `trigger` | `trigger` | host (no MCP tool) | out: `fire` | `mode` ∈ `manual\|cron\|event\|inject\|boot`; `cron` spec / `series` per mode; an `inject` carries a sub-mode `fire\|retain` |
| `tool` | `transform` | the node's own `mcp_verb` field | in: `args`; out: `output` | `{ verb: string, args: object }` |
| `rhai` | `transform` | host `rules.eval` (the `lb-rules` cage) | in: `input`; out: `output`, `findings` | `{ source: string }` |
| `subflow` | `transform` | host `flows.run` (child, pinned) | in/out: **by the child's named ports** | `{ flow: "flow-id@version" }` (Decision 4) |
| `sink` | `sink` | host write (`inbox\|outbox\|channel\|series`) or `<ext-node>` | in: `value` | `{ target: ... }` |

Two notes. The built-in `tool` node is **"everything is a node"** for *actions*: it carries a
`verb` + `args` and dispatches any granted MCP verb, so the registry doesn't need a descriptor
per verb — one generic descriptor covers them all (the editor's verb-picker reads `mcp.resolve`).
The `trigger` node's `inject` sub-mode (Decision **9**, [`flows-scope.md`](./flows-scope.md))
splits intent: `fire` starts a one-shot run with the injected value, while `retain` updates the
node's RETAINED value in `flow_input:{ws}:{flow}:{node}` (read by future runs) and does **not**
start a run — this is what lets a dashboard slider/switch drive a control loop without a
long-lived run.
A `sink` whose `target` is `<ext-node>` defers to an extension `[[node]]` of `kind = "sink"`; the
built-in sink and the ext sink are the *same descriptor shape*, so the canvas treats them as one.
The `subflow` node's `flows.run` binding is **not a plain synchronous call**: the node's step
**parks on the pinned child run** (Decision **11**, [`flows-scope.md`](./flows-scope.md)),
suspending until the child `flow_run` reaches terminal, then maps the child's outputs → the
parent's named ports (Decision 4 grammar). The coordination pattern is detailed in
[`flow-run-scope.md`](./flow-run-scope.md).

## Ports and the binding grammar

Ports are **named** (`inputs` / `outputs` arrays of strings). An **edge** connects an upstream
output port to a downstream input port and carries a **whole-value binding in the chain grammar
verbatim** — exactly one of:

- `${steps.<id>.output}` — the upstream node's output value,
- `${steps.<id>.findings}` — the upstream node's findings (the rhai-cage convention),
- `${params.<name>}` — a flow/subflow parameter (Decision 4),
- or a **literal** (a TOML/JSON scalar or object).

**No templating mini-language** — a binding is exactly one reference or one literal, never a
partial-interpolation string (the `rule-chains` rule verbatim, `../rules/rule-chains-scope.md`).
*Rejected:* a `"prefix-${steps.x.output}-suffix"` interpolation — it invites a regex evaluator,
ambiguous typing, and an injection surface; resist until a real caller forces it (none has).
Because the grammar is identical to chains, the resolver, the validator, and the editor's wire
inspector all carry over unchanged.

## The merged registry — `flows.nodes`

One read-only MCP verb. `flows.nodes` returns the **node registry** =
`built-in descriptors ∪ every installed extension's [[node]] descriptors`, for the **calling
workspace**. It is **derived, not stored**: the host walks the workspace's `ext.list` install
records (each carries its parsed manifest, hence its `[[node]]` blocks) and unions them with the
five built-ins — holding **nothing new durable** (Decision: no new table; descriptors ride the
existing `install:{ext_id}` record). The editor palette renders entirely from this response.

- **API shape (§6.1):** read-only — **one `get-list` verb, no writes**. There is nothing to
  create/update/delete: a node type appears by *installing* an extension and disappears by
  *uninstalling* it (that's `ext.*`, not `flows.*`). No live feed is needed at descriptor
  altitude — the palette is fetched when the editor opens; install/uninstall is an explicit user
  action the editor re-fetches after. *Rejected:* a `flows.nodes.watch` stream (the registry
  changes only on an install the same client just triggered — polling on open suffices).
- **Capability:** `mcp:flows.nodes:call` — **read-only, admin-or-any-granted**. The descriptor
  **declares no capabilities itself**; reading the catalog reveals only *what could run*, and the
  executing **tool's** caps gate actual execution (`caller ∩ install-grant`,
  [`extension-nodes-scope.md`](./extension-nodes-scope.md)). So the palette is broadly readable;
  the deny lives at run time, not catalog time.

## Config schema: dialect, validation, and `config_version` evolution

**Dialect (Decision 3):** JSON-Schema **2020-12**, inline in `[[node]]` (no external file ref —
the manifest declares no paths). Validated **host-side** with the `jsonschema` crate and
**editor-side** with `ajv`, so a bad config is caught both before save and before run. A node
needing a huge schema is a smell — split the node.

**`config_version` + evolution** mirrors the job `schema_version` discipline:

1. A node descriptor carries `config_version` (an integer), bumped when its `config` schema
   changes shape. A persisted node config on a `flow` record records the `config_version` it was
   authored against.
2. A **run pins the flow version** (Decision 1), so an **in-flight run is never re-validated** —
   it executes the graph (and configs) it pinned, immune to a later schema bump.
3. On a **flow-version bump at save**, the host **re-validates every persisted node config**
   against the (possibly newer) descriptor schema. A config that no longer validates blocks the
   save with a precise error (which node, which schema rule) — the author fixes it in the new
   version; the old, pinned version keeps running. *Rejected:* silent best-effort migration of
   stored configs (a guess that diverges a config from its author's intent); we **fail the save**
   and make the human reconcile.

## How it fits the core

- **Tenancy / isolation:** the registry is **ws-scoped** — `flows.nodes` walks *this workspace's*
  `ext.list` records, so an extension installed in ws-A contributes nodes **only** in ws-A. ws-B's
  registry is the built-ins ∪ ws-B's own installs. Isolation is a mandatory test (below).
- **Capabilities:** one cap, `mcp:flows.nodes:call`, read-only. The descriptor declares **no**
  caps; execution caps belong to the bound tool, gated at run time elsewhere.
- **One datastore / no new persistence:** descriptors are **derived** from the existing
  `install:{ext_id}` record's parsed manifest. **No new table, no new record** — the registry is a
  read-time union, not stored state.
- **Symmetric nodes:** the merge logic is one code path; built-in vs extension is data in the
  union, never an `if cloud {…}` or an `if native {…}` branch.
- **MCP is the contract:** the registry *is* an MCP verb; the editor, AI agents, and other
  extensions all read the palette the same way.
- **SDK/WIT impact (flag loudly):** `[[node]]` is the **only** manifest addition — additive, and
  intended to be stable forever-ish, so it trips the README **§11.2 "stop and confirm"** gate for
  the manifest contract. **No new WIT world**: node execution reuses the frozen `tool.call` /
  `host.call-tool` (detail in [`extension-nodes-scope.md`](./extension-nodes-scope.md)).
- **One responsibility per file (FILE-LAYOUT):** implied modules each own one verb —
  `manifest/node_block.rs` (parse + validate `[[node]]`), `flows/descriptor.rs` (the shared
  shape), `flows/builtins.rs` (the five built-in descriptors), `flows/registry.rs` (the merge),
  `flows/nodes_verb.rs` (the `flows.nodes` handler), `config/schema_validate.rs` (the `jsonschema`
  gate). No `utils.rs`.

## Example flow

1. An admin installs the `mqtt` extension into **ws-A**; its `extension.toml` carries the
   `[[node]]` `mqtt_publish` (bound to `[[tools]].publish`) shown above. The host parses it,
   validates the `tool` binding (the `publish` tool exists ✓) and the `config` (valid
   JSON-Schema 2020-12 ✓), and stores it on `install:mqtt`.
2. A user in ws-A opens the flow editor; the canvas calls `flows.nodes`. The host returns the
   five built-ins ∪ ws-A's installs — including `mqtt.mqtt_publish`. The palette renders it under
   **Messaging**.
3. The user drops `mqtt.mqtt_publish`, wires a `rhai` node's `output` port into its `payload`
   input with the binding `${steps.shape.output}`, and fills the form: `topic = "sensors/temp"`,
   `qos = 1`. `ajv` validates the config against the inline schema in the browser; save
   re-validates host-side with `jsonschema`. The flow saves as **version N**.
4. The same user, in **ws-B**, opens a flow editor; `flows.nodes` returns built-ins ∪ ws-B's
   installs — **`mqtt.mqtt_publish` is absent** (the ext isn't installed in ws-B).
5. Later the ext author ships `mqtt` v2 with `mqtt_publish` `config_version = 2` (a renamed
   field). On the next save of the ws-A flow, the host re-validates the persisted config against
   the v2 schema; it no longer validates, so the save is blocked with "node `mqtt_publish`:
   missing required `topic`". The author fixes it in **version N+1**; any run still pinned to
   version N keeps executing unaffected (Decision 1).

## Testing plan

Per `scope/testing/testing-scope.md`, all against the **real** store (`mem://`) and a **real**
parsed manifest — no mocks, seed real `install` records. The mandatory categories this doc owns:

- **Parse / validate `[[node]]`:** a well-formed block parses into a descriptor with the right
  `type` / `kind` / ports / `config`.
- **Reject a bad `tool` binding:** a `[[node]]` whose `tool` names a non-existent `[[tools]]`
  entry fails to load with a precise error.
- **Reject a non-schema `config`:** a `config` table that isn't valid JSON-Schema 2020-12 fails
  to load (the `jsonschema` schema-compile gate).
- **Reject a bad config *instance*:** a node config that violates its descriptor's schema fails
  validation at save (`required` missing, wrong `enum`, `additionalProperties`).
- **`config_version` evolution:** a flow-version bump re-validates persisted configs against the
  new schema and blocks the save on a mismatch; a run pinned to the old version is unaffected.
- **Merged registry reflects install/uninstall:** `flows.nodes` includes an ext's nodes after
  install and drops them after uninstall (derived, not stale).
- **Workspace-isolation (mandatory):** an ext installed in ws-A contributes nodes to ws-A's
  `flows.nodes` and is **absent** from ws-B's registry; ws-B sees only built-ins ∪ ws-B installs.
- **Capability-deny:** `flows.nodes` without `mcp:flows.nodes:call` is refused.

## Risks & hard problems

- **`config_version` evolution is the real one.** The fail-the-save-on-drift rule is safe but can
  surprise an author who bumps an ext under live flows; the editor must surface *which* node and
  *which* rule (the precise-error requirement above) so reconciliation is mechanical, not a hunt.
- **Schema bloat:** a node with a sprawling inline schema signals a node doing too much — a
  review-time guardrail, not a parser rule. Split the node.
- **`[[node]]` is a forever-ish manifest field:** the §11.2 gate is a feature, not friction — it
  forces a deliberate decision before the contract widens again. Keep the block minimal.

## Related

- Spine + Decisions: [`flows-scope.md`](./flows-scope.md) (Decisions **1**, **3**, **4**).
- Siblings: [`extension-nodes-scope.md`](./extension-nodes-scope.md) (execution + caps),
  [`flow-run-scope.md`](./flow-run-scope.md) (the run, version-pinning),
  [`flows-canvas-scope.md`](./flows-canvas-scope.md) (palette + form renderer).
- Manifest contract: [`../extensions/extensions-scope.md`](../extensions/extensions-scope.md)
  (the existing `[[tools]]` shape this is additive to).
- Grammar source: [`../rules/rule-chains-scope.md`](../rules/rule-chains-scope.md) (the binding
  grammar, lifted verbatim).
- README §13 (manifest is the contract), §11.2 (the forever-boundary "stop and confirm" gate),
  §6.10 (jobs — the `schema_version` discipline this mirrors).
</content>
</invoke>
