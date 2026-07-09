# Flows

The flows engine public docs. Filled from the `docs/scope/flows/` set + the session logs as slices
ship.

Shipped so far (promote from the session docs):

- N independent triggers per flow, each firing its own subgraph (`flow-multi-trigger-reactive`).
- The `{payload, topic}` message envelope + auto-wire on connect (`flow-message-envelope`).
- The debug node + debug panel (motion-only, fire-and-forget on the bus) (`debug-node`).
- **Port-labelled edges + per-input-port join policy — the full scope (Slices 1–4)** (`flow-input-ports`):
  the data model, the `any` runtime + firing context, the `link` pair, and the per-port canvas. See below.

## Port-labelled edges + per-input-port join policy

The Node-RED multi-input model, done right. An edge targets a **named input port** on the downstream
node (not just the node), and each input port declares a **join policy** — `all` (a barrier; today's
behaviour) or `any` (a funnel; Node-RED's fire-per-message OR). The author picks the policy by
picking the node (a transform port is `all`; a `debug`/`link-in`/funnel port is `any`), and can
override per port in the descriptor.

### The edge model

An edge is still encoded as `target.needs += [source]` (the DAG topology), with the **target input
port** as additive per-edge metadata:

```json
{
  "id": "avg",
  "type": "rhai",
  "needs": ["sensor-hi", "sensor-lo"],
  "inputs": [
    { "from": "sensor-hi", "toPort": "payload" },
    { "from": "sensor-lo", "toPort": "payload" }
  ]
}
```

`to_port` omitted (or no `inputs` entry for that edge) ⇒ the node's **primary** input port (its first
declared input), so a pre-ports single-input linear flow is unchanged. The two are kept in agreement
by a save-time lint: a `to_port` entry whose `from` is not in `needs` is rejected.

### The descriptor join-policy table

A node descriptor carries its input ports and (optionally) their join policy:

```json
{
  "type": "debug",
  "kind": "sink",
  "inputs": ["payload"],
  "inputPorts": [{ "name": "payload", "join": "any" }]
}
```

The string shorthand `inputs = ["payload"]` keeps its meaning: every port ⇒ `all` for a transform,
`any` for a `sink` (Node-RED's debug/funnel: three wires in ⇒ three firings). An extension opts a node
into OR-fan-in by declaring `join = "any"` on an input port in its `[[node.input]]` manifest block; one
that says nothing keeps `all`.

### The `any` runtime + the firing context (`fctx`)

An `any` port releases **once per settled upstream** — three wires into one `any` port ⇒ three firings
in one durable run, each carrying that one upstream's envelope. Multiplicity is statically bounded by
the wire topology (path count), never by event volume (`split` stays array-carry).

The multiplicity **propagates past the funnel** via a per-message identity carried in an additive
envelope field `fctx` (rides the wire like `topic`/`parts`): a node downstream of an `any` funnel
inherits its multiplicity — it settles once **per funnel firing**, each reading its own firing's
upstream envelope through same-`fctx` `${steps.*}` resolution. Empty `fctx` in the all-`all` case ⇒
today's claim key (`{run}:{node}`) byte-for-byte; a non-empty `fctx` ⇒ `{run}:{node}@{fctx}`. Exactly-
once, the per-node job key, and the outbox dedup key all scope by `(node, fctx)` — so N firings are N
idempotent deliveries, not one swallowing the rest.

### The `link` pair (wireless OR edges)

`link-out {target}` and `link-in {name}` — the canonical "many sources → one handler, fire per
message" collector that needs **no physical wire**. Every `link-out` naming `T` feeds the one
`link-in {name: "T"}`; the "wireless" promise is editor sugar — at run load each `link-out`'s
upstream(s) are rewritten onto the matching `link-in`'s `any` primary port (the `link-out` itself is
dropped from the run graph), so the engine sees ordinary wires and the `fctx` seam propagates the
multiplicity. The persisted flow keeps the author's `link-out`/`link-in` intact (the editor round-
trips the sugar; a deleted `link-out` can never leave a stale wire). Save-time rejects a `link-out`
targeting a missing `link-in`, a wire from a `link-out`, and a `link-in` with no sources at all.

### Save-time lints

- A wire to an **undeclared** input port (a misnamed handle, or a port the node type does not expose)
  ⇒ error.
- A node with an incoming wire but **no declared input port** (a misconfigured trigger/source) ⇒
  error.
- A multi-wire **`all`** port (a barrier) must bind `payload` explicitly — the engine cannot know
  which upstream's message to carry, and silently picking one would hide a join bug / drop data. An
  `any` port with N wires is **valid, silent** — the funnel fires once per upstream (the whole point).
- A `link-out`/`link-in` topology mistake (above) ⇒ error.

### The canvas

Each node renders one target handle per declared input port, each wearing an **`any` (funnel) vs
`all` (join) glyph** so the author reads the convergence at a glance (`debug`/`link-in` = funnel,
`rhai`/`avg` = join). A single-port node keeps one anonymous handle; a multi-port node stacks named
handles (an edge's `targetHandle` matches the port name). A wire to a named (non-primary) port shows
its `to_port` as a midpoint label. The palette shows each node's input ports + their policy marks, so
an author picks a node knowing whether it funnels or joins before dragging it on.
