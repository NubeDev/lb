# Flows scope — extensions contribute backend node types (WASM + native parity)

Status: scope (the ask). Promotes to `public/flows/` once shipped. **Read the spine first** —
[`flows-scope.md`](./flows-scope.md) owns the canonical **Decisions (v1) 1–7**; this doc references them
by number and does not re-decide them.

This doc owns the user's key ask: how an **extension contributes a backend node type** to a flow, working
**identically for WASM (Tier-1) and native (Tier-2)**, and how those nodes execute under the capability
gates. The headline is a one-liner: **the node descriptor is identical across tiers; only the execution
transport differs.** That is the same symmetric-nodes principle the runtime already uses (one binary,
config/role not a code branch — CLAUDE rule 1) applied to node execution. An `mqtt` extension drops an
"MQTT publish" node onto a canvas; the flow does real work when it runs; nothing in the descriptor, the
palette, the editor form, or the gate path knows or cares whether the backend is a wasm guest or a
supervised process.

## Goals

- An extension declares a flow node with the additive `[[node]]` block — the **contract lives in**
  [`node-descriptor-scope.md`](./node-descriptor-scope.md) (ports + inline JSON-Schema config + the bound
  `tool`). This doc consumes that contract; it does not redefine it.
- **Tier parity:** the *same* descriptor binds the *same* tool whether the backend is WASM or native. The
  host picks the transport from the install record, not from anything in the node.
- The **three interaction shapes** — transform, sink, source — each map onto an **already-shipped
  primitive**, with no new engine and no new WIT world.
- Node execution runs under the **three capability gates** (`host-callback-scope.md`): `effective_principal
  = caller ∩ install-grant`, host-set `ws`, two-directional deny — **no widening, ever**.
- A node needing a **secret** (an MQTT broker password) gets it mediated by `lb-secrets` under the
  *extension's* grant; the flow never sees it. A native node owns its external socket via `net:*`.

## Non-goals

- **No new execution runtime, WIT world, or manifest surface** beyond the additive `[[node]]` block
  (spine Non-goals). Node execution reuses the frozen `tool.call` / `host.call-tool` boundary shipped
  2026-06-27 (`host-callback-scope.md`).
- **No durable state in a node instance** (CLAUDE rule 4). A source's socket lives in the (supervised,
  native) extension; run state lives in the `lb-jobs` job; the flow instance is stateless.
- **No second data path.** A node reaches the platform only through host-mediated MCP tools — never a raw
  `Store`/bus handle (rule 5). A source bridges through `ingest.write`, not a private subject.
- Re-deciding source bridging, config schema, or sharing — those are spine Decisions 2, 3, 7.

## Intent / approach — the parity story (the headline)

An extension declares a node in `extension.toml`. The descriptor names ports, an inline config
JSON-Schema, and the **bound MCP tool** the engine invokes for that node (`node-descriptor-scope.md`). The
descriptor is **byte-for-byte tier-agnostic.** When the engine runs the node it calls the bound tool
`<ext_id>.<tool>(input) → output` through the **one** host chokepoint, `lb_host::call_tool(node,
&effective_principal, ws, tool, input)` — the same authorize-then-dispatch the page bridge and the
guest-callback already use (rule 7). **Only the last hop differs**, and the host selects it from the
install record:

- **WASM (Tier-1, in-process wasmtime Component Model).** The host invokes the bound tool via the WIT
  `tool.call(name, input)` — JSON in, JSON out. A guest→host call (e.g. a source's `ingest.write`) uses the
  **frozen `host.call-tool` import** (shipped 2026-06-27).
- **Native (Tier-2, supervised OS process via `lb-supervisor`).** The host sends a framed JSON-RPC
  `{method, params}` over the child's stdio (`Content-Length` framing). The child receives identity by env
  — `LB_EXT_WS` / `LB_EXT_ID` / `LB_EXT_TOKEN` — and its child→host callbacks are framed `call-tool`
  requests dispatched through `lb_host::call_tool`. The sidecar handle is keyed `(ws, ext_id)` in the
  supervisor's `SidecarMap`.

This is **exactly** the symmetric-transport split the runtime already lives by: edge vs cloud is config and
role, never a code branch; here wasm vs native is install record and transport, never a branch in the flow
engine. The engine sees one thing — "call this tool, get JSON" — and the dispatcher routes it. *Rejected:*
a per-tier node variant in the descriptor (forks the palette and the editor, and an AI author couldn't tell
which to write) and a flow-engine `if native { … }` branch (the precise rule-1 violation we refuse).

## The three interaction shapes (each maps to a shipped primitive)

- **Transform / Sink (request→response).** The engine calls `<ext_id>.<tool>(input) → output`
  **synchronously inside the run step** — the ordinary `tool.call`/framed-JSON-RPC round trip above. A
  transform returns a derived value onto its output port; a pure sink returns an ack. **A must-deliver
  sink** (a publish that *must* reach a broker/another node) does **not** go through raw pub/sub — it
  stages an **outbox** effect (must-deliver, idempotent, retried;
  [`../inbox-outbox/outbox-scope.md`](../inbox-outbox/outbox-scope.md)). The run step records the staged
  effect; delivery is the outbox's durable job. Fire-and-forget sinks may publish directly.
- **Source (long-lived / external, e.g. MQTT subscribe).** **Spine Decision 2.** A source can't be a
  request/response call — it's an external feed. The source tool bridges incoming external events through
  `ingest.write` onto a **host-allocated series** `flow:{ws}:{flow}:{node}` — the **exact shipped
  MQTT-bridge pattern** (inbound MQTT → sidecar `ingest.write` → series → SSE;
  [`../extensions/reference-extensions-scope.md`](../extensions/reference-extensions-scope.md)). The flow's
  **event-trigger** node watches that series and fires a run per sample. The host **arms** the source when
  the flow **enables** (start: passes the host-allocated series id + the validated config) and **disarms**
  it when the flow **disables** (stop) — so a disabled flow never leaks a live socket. `arm`/`disarm` are
  just **two ordinary declared `[[tools]]`** the host invokes on enable/disable — **no new WIT import,
  no new world.** The extension owns the socket; the flow instance stays **stateless** (rule 4). *Rejected
  (per Decision 2):* a per-node raw Zenoh subject and letting the extension choose the series name.

## How it fits the core

- **Tenancy / isolation.** Every leg is workspace-scoped: the bridged **series** `flow:{ws}:{…}`, the
  **sidecar** keyed `(ws, ext_id)` in the `SidecarMap`, the staged **outbox effects**, and the run job all
  live in the `{ws}` namespace. The callback `ws` is **host-set from the caller's token, un-spoofable** —
  a node can never name another workspace. Workspace-B physically cannot see Workspace-A's sidecar, its
  series, or its run.
- **Capabilities — the three gates (`host-callback-scope.md`).** When a flow runs a node of an extension
  type, the host derives `effective_principal = caller ∩ install-grant` and authorizes the bound tool
  call; `ws` is host-set from the token. **Two-directional deny, no widening:** (a) if the **install grant
  omits** the tool → **DENIED** even when the caller holds it (delegation narrowing); (b) if the **caller
  lacks** the tool's cap → **DENIED** even when the install requested it (intersection both ways). Running
  the flow additionally needs `mcp:flows.run:call` — composition, never widening (spine "How it fits the
  core").
- **Placement.** Either, symmetric. A node binding a tool on another node routes through the existing
  registry/queryable path; **a `local-only` node only runs where it's installed** (an MQTT socket node runs
  on the appliance that has the broker). No `if cloud`.
- **One datastore / state vs motion.** SurrealDB holds the bridged series + the run-store + the outbox
  queue; series/Zenoh carry the live ticks. An external store an extension owns (Timescale, a broker) is
  the **extension's**, behind its MCP tools — rule 2 untouched (the sanctioned-escape-hatch doctrine).
- **Stateless extensions / hot-reload.** A node instance holds no durable state. Swapping a node-providing
  extension `mqtt@0.1.0 → mqtt@0.1.1` mid-flow is safe: graph is in the store, **run state is in the job**,
  the live value is on the series; the next node call resolves a fresh instance/route.
- **Secrets.** A node needing a secret (broker password) is an extension node; the secret is mediated by
  `lb-secrets` under the **extension's** grant (`secret:<ext>/<key>:get`) — **the flow never sees it**.
  Native extensions get `net:*` caps (`net:tcp:<host>:<port>`, `net:tls:…`, `net:zenoh:<scope>`) to own
  external sockets: the **sanctioned escape hatch** (`reference-extensions-scope.md`) — a native Tier-2 ext
  may own external resources without breaking rule 2.
- **MCP surface / API shape (§6.1).** **Consumes** the existing tool surface through `call_tool`; **exposes
  no new flow-engine verb** here (the `flows.*` family is the spine's). A node's `[[tools]]`
  (subscribe/publish/arm/disarm) are ordinary MCP tools, each gated `mcp:<ext>.<tool>:call`. No new
  CRUD/`watch`/batch on the engine side — live source data is the existing **series SSE**.
- **SDK/WIT impact.** **None.** The `[[node]]` block is additive manifest (the spine's only addition);
  arm/disarm are ordinary tools; execution reuses the frozen `tool.call` / `host.call-tool`. Flagged so the
  reviewer can confirm no boundary moved.

## Which tier for which node (guidance)

A node that **owns an external socket or DB** — MQTT, Modbus, a SQL connector, a Zenoh mesh — is **native
(Tier-2)**: it holds a long-lived connection a wasm sandbox cannot. A node that is a **pure transform**
(reshape JSON, derive a value, normalize a payload) is **WASM (Tier-1)**: sandboxed, hot-reloadable, no
external resource. When in doubt: external resource → native; pure compute → wasm. The descriptor is the
same either way; only the install record (and thus the transport) differs.

## Example flow — an MQTT extension's `mqtt.in` / `mqtt.out` nodes

A worked `mqtt/extension.toml` (the `[[node]]` contract is `node-descriptor-scope.md`; shown here to
ground the narrative):

```toml
[extension]
id = "mqtt"
tier = "native"            # owns a long-lived broker socket → Tier-2 (guidance above)

# --- node types this extension contributes to the flow palette ---
[[node]]
type = "mqtt.in"
kind = "source"            # long-lived; host arms/disarms on enable/disable (Decision 2)
tool = "subscribe"         # the bound tool the host arms with the allocated series id
[node.config]              # inline JSON-Schema (2020-12), Decision 3 — editor renders the form
type = "object"
required = ["broker", "topic"]
properties.broker = { type = "string" }
properties.topic  = { type = "string" }
properties.qos    = { type = "integer", enum = [0, 1, 2], default = 0 }

[[node]]
type = "mqtt.out"
kind = "sink"              # must-deliver → staged as an outbox effect, not raw pub/sub
tool = "publish"
[node.config]
type = "object"
required = ["broker", "topic"]
properties.broker = { type = "string" }
properties.topic  = { type = "string" }
properties.qos    = { type = "integer", enum = [0, 1, 2], default = 0 }

# --- the tools the nodes bind (one responsibility per file in the crate) ---
[[tools]]
name = "subscribe"         # arm target: opens the socket, bridges topic → series
[[tools]]
name = "publish"           # sink target: publish a message (must-deliver via outbox)
[[tools]]
name = "arm"               # host calls on flow ENABLE (start) — ordinary tool, no new WIT
[[tools]]
name = "disarm"            # host calls on flow DISABLE (stop) — releases the socket

[capabilities]
request = [
  "net:tcp:broker.local:1883",       # own the broker socket (native escape hatch)
  "secret:mqtt/password:get",        # broker password, mediated by lb-secrets
]
```

Narrative:

1. **Install.** Admin approves the grant; the host stores `granted = requested ∩ approved`. The palette now
   shows `mqtt.in` and `mqtt.out` as draggable nodes (descriptor → editor form from the inline schema).
2. **Wire.** The user wires `mqtt.in → Rhai(value > 5) → Tool(inbox.raise) → mqtt.out` on the canvas and
   saves — a new flow version (Decision 1).
3. **Enable → arm.** On enable, the host **arms** `mqtt.in`: it allocates series `flow:{ws}:{flow}:{mqtt-in}`,
   validates the node config against the inline schema, and calls the extension's `arm` tool with the series
   id + config. The native sidecar (keyed `(ws, mqtt)`) pulls the password via `secret:mqtt/password:get`,
   checks `net:tcp:broker.local:1883` is in its grant, and opens the socket.
4. **Broker message → run.** A broker message arrives; the sidecar emits a framed
   `call-tool("ingest.write", {series:"flow:{ws}:{flow}:{mqtt-in}", payload, …})` → host authorizes
   `mcp:ingest.write:call` against `caller ∩ grant` in `{ws}` → series tick. The flow's **event-trigger**
   (watching that series) fires a **run** (an `lb-jobs` `flow-run` job).
5. **Run the graph.** `Rhai(value > 5)` filters in the `lb-rules` cage; on pass, the `Tool(inbox.raise)`
   node calls `inbox.raise` — **gates re-checked** (effective principal ∩ grant, host-set ws); then
   `mqtt.out` stages a **must-deliver outbox effect** (`publish`), delivered idempotently by the outbox.
6. **Disable → disarm.** On disable the host calls `disarm`; the sidecar closes the socket. **No leaked
   socket** — a disabled flow holds nothing live.

## Testing plan

Mandatory categories from [`../testing/testing-scope.md`](../testing/testing-scope.md), all against the
**real** store (`mem://`) / bus / jobs / outbox / gateway and the **real** supervisor + wasm runtime — no
mocks (CLAUDE §9). The **only** permitted fake is the external MQTT broker, behind **one clearly-named
extension trait in one file** (testing-scope §0).

- **Capability deny — both directions of install-grant narrowing (MANDATORY).** (a) A node bound to a tool
  the **install grant omits** → `Denied` even though the caller holds it; (b) a node bound to a tool the
  **caller lacks** → `Denied` even though the install requested it. Run on **both** transports (wasm
  `tool.call` and native framed callback) — same deny, two front doors.
- **Workspace isolation (MANDATORY).** Across **store AND mcp**: ws-B's **sidecar** (`SidecarMap` key
  `(ws, ext_id)`), its bridged **series** `flow:{ws}:…`, and its **run** are invisible to ws-A; the
  callback `ws` is un-spoofable (a node in ws-B reaches none of ws-A's data).
- **Hot-reload (MANDATORY).** Swap a node-providing extension mid-flow (`mqtt@0.1.0 → 0.1.1`): an in-flight
  `flow-run` keeps its run state (job-held), the next node call resolves the new instance, **no dropped run
  state**, no identity bleed across calls.
- **Transform round-trip.** A wasm transform node round-trips JSON: seed input on the port, assert the
  derived value on the output port, end to end through `call_tool`.
- **Source lifecycle.** A source **arms** (host allocates the series + calls `arm`), **bridges** a real
  broker message via `ingest.write` onto `flow:{ws}:{flow}:{node}`, the **event-trigger fires** a run, and
  on disable the host **disarms** it — assert **no leaked socket** (the fake broker reports the subscription
  closed).
- **Must-deliver sink idempotency.** `mqtt.out` stages an **outbox** effect; assert it lands in the outbox
  and re-driving the run (resume/retry) does **not** double-publish (idempotent on the outbox key).

## Risks & hard problems

Structural questions are settled in the spine's **Decisions (v1)**. The genuine build-time risks here:

- **Transport parity is a real seam, not a wish.** The dispatcher must route wasm vs native off the install
  record with **zero** flow-engine branch; the deny tests must pass identically on both transports or the
  parity claim is theater.
- **Source fan-out.** A chatty broker → one run per sample → job storm. The event-trigger debounce posture
  lives in [`flow-run-scope.md`](./flow-run-scope.md); the source must respect it, not flood.
- **arm/disarm must be exactly-once at the edges.** A double-enable must not open two sockets; a
  disable-mid-arm must still release. The host owns the lifecycle (it calls the two tools) — make it
  idempotent against the extension's connection state.
- **Secret never escapes the extension.** The broker password reaches the sidecar via `lb-secrets`,
  never the flow, never a log, never the bridged series payload — assert it in the secret test.

## Related

- Siblings: [`flows-scope.md`](./flows-scope.md) (the spine + Decisions 1–7),
  [`node-descriptor-scope.md`](./node-descriptor-scope.md) (the `[[node]]` contract),
  [`flow-run-scope.md`](./flow-run-scope.md) (the durable run + debounce),
  [`triggers-lifecycle-scope.md`](./triggers-lifecycle-scope.md) (enable/disable arm/disarm hooks).
- Extensions: [`../extensions/host-callback-scope.md`](../extensions/host-callback-scope.md) (the three
  gates + the frozen WASM callback), [`../extensions/reference-extensions-scope.md`](../extensions/reference-extensions-scope.md)
  (the MQTT-bridge pattern + the `net:*` escape-hatch doctrine),
  [`../extensions/native-tier-scope.md`](../extensions/native-tier-scope.md) (the Tier-2 supervisor +
  `SidecarMap` + framed JSON-RPC).
- Durability + secrets: [`../inbox-outbox/outbox-scope.md`](../inbox-outbox/outbox-scope.md) (must-deliver
  sinks), [`../secrets/`](../secrets/) (`lb-secrets` mediation).
- README §3 (rules 1/2/4/5/7), §6.5 (host dispatch), §6.13 (the three gates), §13 (manifest is the contract).
