# Flows scope — the Node-RED-style message envelope (`payload`/`topic`, auto-wire on connect)

Status: scope (the ask). Promotes to [`public/flows/flows.md`](../../public/flows/flows.md) once shipped.
Read the spine [`flows-scope.md`](flows-scope.md) (canonical **Decisions 1–13**) and
[`node-descriptor-scope.md`](node-descriptor-scope.md) (ports) first.

> **No backwards compatibility.** Flows is in development; there is no migration burden. This is a
> **clean breaking change** to the inter-node message model and the binding grammar. Old `${steps.x.output}`
> bindings and bare-value node outputs are **removed**, not deprecated. Re-save any dev flows.

We want flows to feel like **Node-RED**: a message is an object with a `payload` (the value) and optional
`topic` (routing/name), it **flows down a wire automatically** when you connect two nodes, and metadata
like `topic` **carries through** a chain. Today a connection (`needs`) only sets *ordering* — data does
**not** flow until you also hand-type a binding (`with: {items: "${steps.a.output}"}`); in the real
`chain4`, node `b` only received `a`'s value because someone typed that string. That is the friction. We
adopt Node-RED's **vocabulary and ergonomics** (familiar names + drag-a-wire-and-it-flows) **without**
adopting its mutate-one-shared-`msg` runtime — each node still records its **own** immutable output
envelope, which is what powers resume, the persistent runtime view (`flow_node_state`), and typed ports.

## Goals

- **One message shape — the envelope.** Every value on a wire is a JSON **object** with a conventional
  primary slot `payload` and optional well-known `topic`; nodes may carry extra fields. This replaces
  today's bare per-node outputs (`{count: 4}`, a raw scalar, …).
- **Auto-wire on connect.** Connecting A→B makes B **receive A's full output envelope** as its input
  message with **no binding typed** — the Node-RED "just drag a wire" feel. Explicit bindings stay for
  joins/multi-input/power use.
- **Metadata carries through.** `topic` (and any non-`payload` field) propagates down a linear chain
  unless a node overwrites it — like `msg.topic` in Node-RED.
- **Familiar names everywhere.** The default in/out port is `payload`; `topic` is the routing slot. The
  canvas shows `payload`; the dashboard picker offers `payload`/`topic`.
- **Kill the implicit-throughput trap while we're here.** The `counter` node's "increment by input size
  when an input is wired" auto-detection becomes an **explicit `mode`** (now that auto-wire means a
  payload is almost always present) — no more surprise +N.

## Non-goals

- **No Node-RED runtime/import compatibility.** We do **not** import `.json` flows or run their node
  implementations. We borrow the *message convention*, not the engine (we already chose our durable,
  capability-gated engine over edgelinkd/reflow — [`flows-scope.md`](flows-scope.md)).
- **No mutate-and-pass-the-same-object.** The wire carries a **copy-forward** of the upstream envelope;
  each node emits a **new** recorded envelope. No shared mutable `msg` (it would break per-node durable
  snapshots / resume).
- **No templating mini-language.** A binding is still exactly one whole-value `${…}` reference or a
  literal — we only widen *what* a reference can address (a field path), not introduce interpolation.
- **No new transport, table, or capability.** Same store records, same verbs, same gates. `flow_node_state`
  and `flow_input` now hold envelopes/payloads; nothing new is persisted.

## Decisions (resolved — there are no open questions)

**D1 — The envelope.** A message is a JSON object. Reserved fields: **`payload`** (the primary value,
always present on a node's output) and **`topic`** (optional string — routing/destination name). Any
other field is free metadata (e.g. a rules node's `findings`). A node that needs a non-object value puts
it in `payload`.

**D2 — `inputs` *is* the incoming message.** `execute_one` builds an `inputs` map that **is** the node's
incoming `msg`. Every builtin reads `inputs["payload"]` (and `inputs["topic"]` where relevant) instead of
ad-hoc keys (`items`, `value`).

**D3 — Auto-wire (single upstream).** In `resolve_node_bindings`: if a node has **exactly one** `needs`
upstream **and** its `with` does not bind `payload`, then `inputs` = that upstream's **full recorded
envelope** (copy). With an explicit `with`, build `inputs` from the bindings only (no auto). With **≥2**
upstreams and no `with`, `inputs` is empty and a **save-time lint** flags "node has N inputs — bind
`payload`" (a join must be explicit). Auto-wire never overrides an explicit binding.

**D4 — Metadata carry-forward.** A node's recorded output envelope = `{ ...carry, ...emitted }` where
`carry` = the incoming `inputs` **minus `payload`** (so `topic` and friends propagate) and `emitted` =
what the node produced (always a fresh `payload`, plus any field it sets — e.g. it may set `topic`).
Carry-forward applies only when `inputs` came from a single upstream (D3) or a single explicit `payload`
binding; a multi-input join emits just its `emitted` envelope (no ambiguous merge).

**D5 — Binding grammar (breaking).** `lb-flows/src/binding.rs` resolves:
- `${steps.<id>}` → the upstream's **whole envelope**;
- `${steps.<id>.<dot.path>}` → a field path **into** that envelope (`payload`, `topic`, `findings`,
  `payload.items`, …) via a JSON-pointer-style walk (missing → `null`);
- `${params.<name>}` → a flow/subflow param (unchanged);
- a literal otherwise. **Remove** the special-cased `.output`/`.findings` forms — they become ordinary
  field paths (`.payload`, `.findings`). Keep "whole-reference only, no interpolation".

**D6 — Per-builtin envelopes** (the implementer's contract):

| Node | Reads | Emits (`emitted`) | Notes |
|---|---|---|---|
| `trigger` | run params under node id, else `with` | `{ payload: <firing value>, topic: <config.topic?> }` | cron firing value is the cron ts; inject firing value is the injected payload |
| `count` | `payload` | `{ payload: <size> }` | array len / object keys / scalar→1 / null→0 |
| `counter` | `payload` (throughput mode only) | `{ payload: <running total> }` | **D7** |
| `rhai` | whole `msg` as the script scope | the script's return as `payload`; if it returns an object containing `payload`, that object **is** the emitted envelope (function-node `return msg`); rules `findings` → `findings` field | |
| `tool` | `config.args` merged with `payload` when `payload` is an object | `{ payload: <verb result> }` | |
| `sink` | `payload`; destination = `msg.topic ?? config.name` | `{ payload }` (pass-through) | series sample `payload` = `msg.payload`; inbox `body` = stringify(`payload`); outbox value = stringify(`payload`) |
| `subflow` | `payload` in | child's `payload` out | |

**D7 — `counter` mode is explicit (trap removed).** Config gains `mode: "tick" | "throughput"` (default
**`tick`**) alongside `step` (default 1) and `reset`. `tick` → increment by `step` every firing,
regardless of input. `throughput` → increment by the **size** of `payload` (array len / object keys / 1).
This removes the old "an input is wired ⇒ throughput" surprise (now that auto-wire means a payload is
almost always present).

**D8 — `flows.inject` / retained inputs are `payload`.** `flows.inject {id, node, value}` sets the node's
retained **`payload`** in `flow_input:{flow}:{node}`. When a run reads a retained input node, `inputs =
{ payload: <retained value> }`. (The port-aware variant is owned by
[`flow-dashboard-binding-ux-scope.md`](flow-dashboard-binding-ux-scope.md); this doc keeps inject =
node-level payload.)

**D9 — `flow_node_state` stores the envelope.** A node's persistent last-value record holds the whole
emitted envelope; `flows.node_state` returns it unchanged. The canvas + dashboard read `payload` from it
for display (D10/D11).

**D10 — Canvas displays `payload`.** `flowGraph.ts` (`snapshotValues`/`nodeStateValues`) and
`FlowNodeView` show the node's **`payload`** as the value badge (fall back to the whole envelope only when
there is no `payload` key). The node-inspector/wire view may show the full envelope.

**D11 — Dashboard reads default to `payload`.** Read views/widgets bound to a flow node default to the
`payload` field; a JSON/object view may show the whole envelope (see the binding-UX scope).

## How it fits the core

- **Tenancy / isolation (rule 6):** unchanged — envelopes live in the same ws-scoped `flow_node_state`/
  `flow_input` records and ride the same ws-walled verbs. **Two-session isolation test still required**
  (a ws-A flow's envelopes never reachable from ws-B).
- **Capabilities (rule 5/7):** unchanged — no new verb or cap; the message shape is internal to a run
  that already runs under `caller ∩ grant`. The deny tests (a tool node calling an ungranted verb) carry
  over verbatim — only the assertion shapes change (`payload`).
- **Placement (rule 1):** no role branch; pure engine change.
- **MCP surface (§6.1):** **no new verbs.** `flows.inject` keeps its signature (its `value` is now framed
  as the retained `payload`). `flows.node_state`/`flows.runs.get` return envelopes. Get/list/live/batch:
  N/A — this is the data model under the existing surface.
- **One datastore / state vs motion (rule 3):** envelopes are **state** in `flow_node_state`; the run is
  the motion. No new persistence; `flow_input` holds a `payload`, `flow_node_state` holds an envelope.
- **Durability:** unchanged — must-deliver sink effects still go through the outbox inside the run.
- **No mocks / no fake backend (CLAUDE §9):** every test drives a **real** run over the real store and
  asserts real recorded envelopes; no `*.fake.ts`.
- **Stateless / SDK-WIT:** **flag** — the descriptor `inputs[]`/`outputs[]` port names change to
  `payload`/`topic` for built-ins. Extension `[[node]]` descriptors declare their own ports and are
  unaffected, **but** an extension node now receives the envelope as its input `msg` and should emit a
  `payload` to participate in carry-forward. Document this in the node-descriptor public doc; it is a
  convention, not a WIT change (the callback still passes JSON).

## Example flow — a linear chain that "just works"

Flow `temps` (ws `kfc`): `trigger(cron */1) → scale(rhai) → store(sink series)`, **no `with` typed on any
node** (all auto-wired).

1. **trigger fires.** Emits `{ payload: 1782823440, topic: null }` (cron ts as payload).
2. **scale auto-wires.** It has one upstream, no binding → `inputs` = trigger's envelope. The rhai script
   `msg.payload = msg.payload * 0.1; msg.topic = "kfc.temp"; return msg` emits
   `{ payload: 178282344.0, topic: "kfc.temp" }`.
3. **store auto-wires.** One upstream, no binding → `inputs` = scale's envelope. Destination =
   `msg.topic` (`"kfc.temp"`) since `config.name` is empty; it writes a series sample with
   `payload = msg.payload`. `topic` carried through the whole chain with zero wiring.
4. **A join needs one binding.** Add `avg(count)` that needs **both** `scale` and another source → the
   save-time lint says "bind `payload`"; the author sets `with: {payload: "${steps.scale.payload}"}`.
   Explicit, because a join is genuinely ambiguous.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`) — real store (`mem://`), real caps, no fakes:

- **Capability-deny** — a `tool` node calling an ungranted verb is still denied at that node (assert the
  `Err` step + `FailurePolicy`); a no-widening run gate still bites. (Assertion shapes move to `payload`.)
- **Workspace-isolation** — a ws-A flow's envelopes (`flow_node_state`/`flow_input`) are unreachable from
  ws-B across store + MCP.

Plus this slice (all in `lb-flows` + `lb-host`):

- **Binding grammar (`binding.rs` unit):** `${steps.x}` → whole envelope; `${steps.x.payload}` /
  `${steps.x.topic}` / `${steps.x.findings}` / `${steps.x.payload.items}` → field paths; missing path →
  `null`; `${params.y}`; literal pass-through; partial-interpolation stays literal; old `.output` no
  longer special (it is just a (likely-`null`) field path).
- **Auto-wire (run):** a 3-node linear chain with **no `with`** flows the envelope end to end; a node
  with 2 upstreams and no `with` is flagged by the save-time lint (assert the 400/lint), and works once
  `payload` is bound.
- **Carry-forward (run):** `topic` set upstream survives to the sink unless a middle node overwrites it.
- **Per-builtin envelopes (run):** `trigger`/`count`/`counter`/`rhai`/`tool`/`sink`/`subflow` each emit
  the D6 shape; `count` payload = size; `rhai` `return msg` round-trips; `tool` result becomes `payload`.
- **`counter` mode (run, regression for the trap):** default `tick` → +`step` every firing **regardless
  of payload** (an auto-wired counter no longer jumps by input size); `throughput` → +size. Fail-before
  the `mode` field would have incremented by payload size.
- **Sink targets (extend `flows_sink_test.rs`):** series/inbox/outbox read `payload`; destination uses
  `msg.topic ?? config.name`.
- **Frontend (vitest):** `flowGraph` maps an envelope to the `payload` badge (and falls back when no
  `payload`); existing flows UI tests updated to envelope outputs.

**Test sweep (breaking change):** every existing flows test that asserts a bare output (`output.count`,
a scalar `output`, `${steps.x.output}` bindings) must move to the envelope (`payload`,
`${steps.x.payload}`). Touch list: `flows_run_test`, `flows_nodes_test`, `flows_multi_trigger_test`,
`flows_sink_test`, `flows_runtime_control_test`, `binding.rs` tests, and the UI `flowGraph.test.ts`.
This is expected and part of the slice — the build is not green until the sweep is done.

## Risks & hard problems

- **The test sweep is the bulk of the work, not the engine change.** Every flows test encodes the old
  output shape. Budget for it; "green" means the sweep is complete, not just the new paths.
- **Auto-wire vs joins.** The single-upstream auto-wire must **not** silently pick one of several
  upstreams — that would hide a join bug. The ≥2-upstream lint is load-bearing; without it a join with a
  forgotten binding looks like it works but drops data.
- **Carry-forward scope.** Merging *all* non-`payload` fields forward is powerful but can leak stale
  metadata down a long chain. Keep `topic` the only blessed carried field in v1 *by convention* (the
  merge is general, but built-ins only ever set `payload`/`topic`), and document that a node clears a
  field by emitting it as `null`.
- **`rhai` `return msg` ergonomics.** Deciding "did the script return a value or a full envelope" must be
  unambiguous: **if the return is a JSON object with a `payload` key, it IS the envelope; otherwise it is
  the new `payload`.** Implement exactly that rule; test both.
- **Descriptor port rename ripples.** Renaming built-in ports to `payload`/`topic` changes the palette,
  the canvas handles, and the dashboard picker labels. Grep every `"items"`/`"value"`/`"output"` port
  literal; a missed one is a silently-unwired port.

## Related

- [`flows-scope.md`](flows-scope.md) — the spine; **Decision 4** (the binding grammar this redefines),
  **5** (`flow_node_state`), **9** (inject/retained input = `payload`).
- [`flow-run-scope.md`](flow-run-scope.md) — the run engine + `resolve_node_bindings` that gains auto-wire
  + carry-forward; the executed-node-lock/resume that the immutable-per-node envelope preserves.
- [`node-descriptor-scope.md`](node-descriptor-scope.md) — the `inputs[]`/`outputs[]` ports renamed to
  `payload`/`topic`; the extension-node convention (receive `msg`, emit `payload`).
- [`flow-dashboard-binding-ux-scope.md`](flow-dashboard-binding-ux-scope.md) — consumes this: the picker
  offers `payload`/`topic` ports; controls write `payload`; read views default to `payload`.
- README **§3** (rules 1–4), **§6.1** (API shape).
```
