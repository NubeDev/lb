# Flows

The flows engine public docs. Filled from the `docs/scope/flows/` set + the session logs as slices
ship.

Shipped so far (promote from the session docs):

- N independent triggers per flow, each firing its own subgraph (`flow-multi-trigger-reactive`).
- The `{payload, topic}` message envelope + auto-wire on connect (`flow-message-envelope`).
- The debug node + debug panel (motion-only, fire-and-forget on the bus) (`debug-node`).
- Port-labelled edges + the firing-context (`fctx`) runtime (`flow-input-ports`) — the structural
  seams under plain wiring. The `link` pair + `all`-by-default that shipped with them were removed
  by `flow-plain-wiring` (below).
- **Plain wiring — every port fires per message** (`flow-plain-wiring`). See below.

## Plain wiring — every port fires per message

Exactly the Node-RED model: a node has an input port, **any number of wires can land on it, and the
node fires once per arriving message** — for every node kind, with nothing to configure. Same on the
way out: one output port, any number of wires, every downstream fires (each firing reads its own
immutable copy of the envelope, so the semantics match Node-RED's per-wire `msg` clone). No special
nodes, no policy to think about — a port is just a port.

There is no `link-out`/`link-in` pair and no `Links` palette category: plain wires already fire per
message, so the wireless collector had nothing left to collect. A saved flow that still contains a
link node fails its next save — and an already-armed one fails at **run load** — with a clear
"unknown node kind" error (flows is in development; no migration).

An edge still targets a **named input port** on the downstream node (`toPort`, omitted ⇒ the primary
input), which is what multi-port extension nodes wire by.

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

**Every port defaults to `join = "any"`** — plain per-message wiring, for every kind; the string
shorthand `inputs = ["payload"]` means exactly that. `all` (a barrier: fire once when every wired
upstream on the port has settled, with an explicit `payload` binding) survives only as a
**descriptor-level opt-in**: an extension declares `join = "all"` on an input port in its
`[[node.input]]` manifest block. **No built-in declares it.** An extension node that silently relied
on the old implicit barrier must now declare `join = "all"` explicitly. (Known limit: a multi-port
extension node mixing an explicit-`all` port with other wired ports hits port-blind barrier counting
and a primary-port-only lint — the opt-in is single-port-safe today.)

### The per-message runtime + the firing context (`fctx`)

A port releases **once per settled upstream** — three wires into a `rhai` node ⇒ three firings in
one durable run (whole-graph posture; in reactive posture each source event starts its own run of
one firing — same per-message behaviour, different run bookkeeping), each carrying that one
upstream's envelope with its `topic` carried forward. Multiplicity is statically bounded by the wire
topology (path count), never by event volume (`split` stays array-carry).

Multiplicity **propagates downstream** via a per-message identity carried in an additive envelope
field `fctx`: a **multi-wire** port mints a firing id per arriving message (`{node}#{upstream}`,
extended segment-by-segment through nested fan-ins); a **single-wire** port propagates the incoming
`fctx` unchanged — a linear chain never grows its lineage, and its claim key stays the plain
`{run}:{node}` byte-for-byte. Exactly-once, the per-node job key, and the outbox dedup key all scope
by `(node, fctx)` — so N firings are N idempotent deliveries, not one swallowing the rest. A matched
`switch` releases an `any` dependent as a normal per-message firing too (`triggered_by` = the
switch); only an explicit-`all` port takes the barrier path.

### Bindings resolve along the firing lineage

`${steps.X}` resolves against X's settle whose `fctx` is an **ancestor** of the current firing's
(equal, a whole-segment prefix, or the `""` root — nearest ancestor wins). A linear chain keeps its
full binding expressivity under per-message firing (a grandparent binding resolves); a genuine
cross-branch settle never matches — and is caught at save (below), not silently bound null.

### Save-time lints

- A wire to an **undeclared** input port (a misnamed handle, or a port the node type does not expose)
  ⇒ error.
- A node with an incoming wire but **no declared input port** (a misconfigured trigger/source) ⇒
  error.
- A `${steps.X}` binding where X is neither the node itself nor a transitive upstream (a sibling
  branch, an unrelated branch, a typo) ⇒ error — it can never be in the firing's lineage and would
  silently bind null per firing.
- A multi-wire **explicit-`all`** port (the extension opt-in) must bind `payload` — the engine cannot
  know which upstream's message to carry. Multiple wires into an ordinary (default) port are
  **valid, silent** — that IS plain wiring.
- An unknown node kind (e.g. the removed link pair) ⇒ error at save AND at run load.

### Named divergences from Node-RED (deliberate)

- **No feedback wires** — cycles are rejected at save (pre-existing engine posture).
- **Duplicate wires collapse** — two wires from the same output to the same input are one firing
  (deterministic firing id), not two messages.
- **Retained inputs override the wire** — a `flow_input` overlay wins over the arriving message.
- **Arrival-order firing** — non-deterministic under concurrency, exactly as Node-RED.

### The canvas

A port is just a port: the default per-message port renders **no policy glyph** anywhere. Only a
port that explicitly opts into `all` (an extension descriptor) wears a small merge glyph flagging
the barrier exception, on the canvas handle and in the palette. A single-port node keeps one
anonymous handle; a multi-port node stacks named handles (an edge's `targetHandle` matches the port
name), and a wire to a named (non-primary) port shows its `toPort` as a midpoint label.
