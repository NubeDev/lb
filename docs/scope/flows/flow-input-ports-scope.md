# Flows scope — port-labelled edges + a per-input-port join policy (the Node-RED multi-input model, done right)

Status: **shipped** (2026-07-09, Slices 1–4). Promoted to
[`public/flows/flows.md`](../../../doc-site/content/public/flows/flows.md). Read the spine
[`flows-scope.md`](flows-scope.md) (canonical **Decisions 1–16**),
[`flow-message-envelope-scope.md`](flow-message-envelope-scope.md) (the `{payload, topic}` envelope +
auto-wire), [`node-descriptor-scope.md`](node-descriptor-scope.md) (ports), and
[`flow-multi-trigger-reactive-scope.md`](flow-multi-trigger-reactive-scope.md) (N independent triggers,
each firing its own subgraph) first.

> **Slicing (recorded).** The scope landed in four slices, each honestly complete on its own
> boundary: **Slice 1** (data model — `to_port`, the `join` policy table, per-port graph math, save
> lints, UI wire types), **Slice 2** (the `any` runtime + the propagated firing-context `fctx` seam),
> **Slice 3** (the `link` built-in pair + per-firing cap-deny/outbox-dedup + the propagate-past-the-
> funnel headline), **Slice 4** (the per-port canvas). Sessions: `flow-input-ports-slice{1,2,3,4}`
> under `sessions/flows/`.

> **No backwards compatibility.** Flows is in development. This is a **structural** change to the edge
> model — an edge stops being a bare node-id `needs` and gains a **target input port** — plus a new
> per-port **join policy** in the descriptor. Re-save dev flows. There is no migration burden and no
> alias; we cut cleanly (the envelope scope set the precedent).

> **Build slicing (in flight).** This scope is large and is being built in four slices. **Slice 1
> shipped 2026-07-09: the data-model foundation** — port-labelled edges (`to_port`), the descriptor
> `join` policy table (`InputPort{ name, join }`), per-port graph math, `[[node.input]]` manifest
> parse, and the registry-aware save lints. **Slice 2 shipped 2026-07-09: the `any` runtime + the
> firing-context (`fctx`) seam** — the load-bearing piece. An `any` port releases once per settled
> upstream, each firing scoped by a per-message identity (`fctx`) carried in the envelope, so
> multiplicity survives past the funnel; every claim key + `${steps.*}` resolution is scoped by
> `(node, fctx)` (empty in the all-`all` case ⇒ today's key byte-for-byte). `sink`-kind ports (incl.
> `debug`) default to `any`; the join lint is per-port policy-aware. **Slice 3 (next): the `link`
> built-in pair** (virtual OR edges — also unlocks the propagate-past-the-funnel + per-firing
> cap-deny/outbox-dedup tests, which need a non-sink `any` node). **Slice 4:** the per-port canvas.
> See [`sessions/flows/flow-input-ports-slice1-session.md`](../../sessions/flows/flow-input-ports-slice1-session.md)
> + [`flow-input-ports-slice2-session.md`](../../sessions/flows/flow-input-ports-slice2-session.md).

## The ask (from the user, against the live design)

> "I want a node to be able to have multiple inputs and work the same way as Node-RED. For multi-inputs
> do whatever is best long term — no hacks, best practice."

Node-RED's convergence has **two** distinct meanings, and today's engine collapses them into one
emergent behaviour. When two wires land on a node, Node-RED fires the node **once per arriving
message** (an **OR funnel** — a debug node wired from three sources prints three times). Our engine
instead treats ≥2 upstreams as an **AND barrier** — the node waits for *all* of them and fires once,
and a save-time lint forces an explicit `payload` binding ([envelope D3](flow-message-envelope-scope.md)).
Both semantics are legitimate; the bug is that **which one you get is not a decision the author makes** —
it falls out of `indegree` counting and a lint, and the OR case (fire-per-message) is simply
**unreachable within a single run**. [Spine Decision 14](flows-scope.md) already named the root cause and
deferred it: *"this DAG's edges are node-id `needs` with no port label … a port-labelled edge model is a
bigger change than this pack owns."* This scope **owns that change**, because the user asked for the
long-term-correct answer, not a lint.

## Root cause (read end-to-end in the code)

1. **Edges are node-id `needs`, not port-targeted wires.** The descriptor *speaks* of ports
   ([node-descriptor "An edge connects an upstream output port to a downstream input port"](node-descriptor-scope.md))
   but the runtime edge is a bare `needs: [node_id]` with **no target-port label** (Decision 14, verbatim).
   So the engine structurally **cannot tell** "two wires into node Z's one input port" (Node-RED OR) from
   "two different inputs Z must join" (AND). It guesses: ≥2 upstreams ⇒ AND barrier.

2. **A node fires exactly once per run.** The frontier claims each node `Enqueued→Running` under a
   `flow_step_output:{ws}:{run}:{node}` key (Decision 8) — one settle per node per run. There is no seam
   for "Z fired once for A's message, then again for B's message." OR-per-message is not expressible.

The result: the *safe durable* behaviour (join-and-correlate) is the **only** behaviour, and the *familiar
Node-RED* behaviour (funnel-and-repeat) is missing. The user wants both — and wants the author to choose,
explicitly, per port.

## The model decision (the best-practice long-term answer)

Make the input the **typed, first-class unit** it should have been, on two axes:

- **Axis 1 — port-labelled edges.** An edge targets a **named input port** on the downstream node
  (`{from: <upstream>, from_port?, to: <node>, to_port}`), not just the node. This is exactly the
  Node-RED wire model (a wire lands on a specific port) and it is the load-bearing structural fix — every
  other decision here needs it. When `to_port` is omitted it defaults to the node's **primary** input port
  (its first declared input), so today's single-input linear flows are unchanged.

- **Axis 2 — a declared per-input-port *join policy*.** Each input port declares how it combines the
  wires that land on it:
  - **`all` (join — the default for transforms):** a **barrier**. The node fires **once** when every wired
    upstream on that port (within the fired trigger's induced subgraph) has settled; the port's value is
    the set of those upstream envelopes, combined by an explicit binding. This is today's AND behaviour —
    now **named and deliberate**, not inferred from a lint.
  - **`any` (funnel — the default for `sink`/observability nodes):** the node is released **once per
    settled upstream** on that port — Node-RED's fire-per-message OR. Three wires into an `any` port ⇒
    three firings in the run, each carrying that one upstream's envelope. Multiplicity is **statically
    bounded by the wire topology (the path count into the node), never by event volume** (a `split` still
    array-carries — [Decision 15](flows-scope.md) — so `any` reintroduces no fan-out storm).

The author picks the policy by picking the node (a transform port is `all`; a `debug`/`link`/funnel port is
`any`), and can override per port in the descriptor. **Safe default, faithful escape hatch, both
declared** — no emergent guessing, no lint standing in for a decision.

Rejected: **keep node-id edges and keep inferring** (the status quo) — it is precisely the hack the user
ruled out: the semantics are a side effect of `indegree` + a lint, OR is unreachable, and a forgotten join
binding "looks like it works" while dropping data (the envelope scope's own stated risk). Also rejected:
**a global engine mode** (all-OR *or* all-AND) — convergence semantics are a **per-port** property of the
node's job, not a flow-wide switch; a `join` node and a `debug` node in the same flow want opposite
policies.

## How `any` fires more than once — the firing context (the load-bearing seam)

This is the one genuinely new engine seam, and getting it right is the reason this scope exists rather
than a lint. The naive design — suffix an `any` node's step-output key with its immediate upstream
(`flow_step_output:{run}:{node}#{upstream}`) — **only disambiguates at the funnel itself and breaks one hop
downstream.** Take `link-in` (an `any` node, 3 wires) feeding a transform `W`. `W` has exactly **one** wire
(from `link-in`), so under a per-upstream scheme its key is the singular `…:{run}:W` — there is **no slot
under which `W` can settle three times**, and `${steps.link-in.payload}` is **ambiguous** (three settles of
`link-in`, and `W`'s firing must read *its* one). Multiplicity must **propagate**, and a depth-1 suffix
cannot express that. The correct primitive is a per-message identity carried down the run.

- **A firing context is minted at each `any` slot and carried forward like `topic`/`parts`.** When an
  `any` port releases for a settled upstream, the engine mints a **firing id** for that slot (the slot key
  `{node}#{upstream}` is a fine id) and stamps it into an **additive envelope field `fctx`** — exactly the
  [Decision 15](flows-scope.md) precedent (`parts` rides the wire; `fctx` does too). The context propagates
  down every wire the firing traverses, so a downstream node knows *which* firing it is executing.
- **Every claim key and `steps.*` resolution is scoped by the current firing context.** A node's
  step-output key becomes `flow_step_output:{ws}:{run}:{node}[@{fctx}]` and a binding
  `${steps.<id>.<path>}` resolves against the upstream settle **carrying the same `fctx`**. In the common
  case `fctx` is the **empty string**, so an all-`all` flow is **byte-for-byte unchanged** (`…:{run}:{node}`,
  today's key and today's resolution). `W` above now settles once per `link-in` firing —
  `…:{run}:W@link-in#mqtt-a`, `@link-in#mqtt-b`, `@link-in#cron-c` — and each reads the matching
  `link-in` settle. No new `Outcome` variant, no wire sentinel ([Decision 14](flows-scope.md)'s discipline)
  — the multiplicity lives in a **carried identity**, not a magic payload.
- **Nested / diamond funnels compose by extending the context.** A firing that passes through a second
  `any` slot extends its id (`link-in#mqtt-a` → `link-in#mqtt-a·funnel2#x`), so multiplicity multiplies
  correctly along **path count**, deterministically keyed, still one run. `all` joins **within** a firing
  context (same `fctx`) barrier normally — the context makes "join the two branches *of this message*"
  well-defined.
- **Exactly-once holds per (node, firing).** [Decision 8](flows-scope.md)'s CAS claim
  (`Enqueued→Running`) now keys on `{node}@{fctx}`: a redelivered message re-mints the **same** `fctx`
  (the slot id is deterministic per `(node, upstream[, parent fctx])`), re-claims the same slot, and
  no-ops — exactly as today's per-node claim does, one hop or ten hops past the funnel.
- **Decision 8's per-node job and the outbox dedup key both take the firing context too.** "One `lb-jobs`
  job per node" becomes one job **per (node, firing)** for a node inside a funnel's reach; a must-deliver
  sink's outbox idempotency key includes `fctx` so N firings are N idempotent deliveries, not one
  swallowing the rest. Thread `fctx` through the job key and the outbox key wherever the node key is used
  today — this is the implementer's tripwire.
- **Still one run, still runs to terminal** ([Decision 9](flows-scope.md)). An `any` node firing N times
  is N settles in **one** `flow_run`; nothing parks, nothing fans a run out. **Run-terminal now counts
  slots, not nodes** — the frontier is done when every reachable `(node, fctx)` slot has settled (including
  gated-empty slots, below).
- **This also makes carry-forward mechanically true downstream.** The Goals' "each `any` firing forwards
  its own `topic`" rule is not a funnel-only special case — because `fctx` scopes resolution, a node three
  hops past the funnel reads *its* firing's upstream envelope, so *its* `topic` is the right one, for free.

The honest v1 trim, if a future team wants to descope: make `any` legal **only on nodes with no outputs**
(sinks — `debug` still gets its three prints, the headline UX still ships) and defer/`sink`-restrict
`link-in`. That avoids propagation entirely (a sink has no downstream). We **do not** take that trim here —
the ask was "long term, no hacks," and the firing context is the actual load-bearing seam; discovering it
mid-implementation would force exactly the patch this doc exists to avoid. It is scoped now.

## Goals

- **Port-labelled edges (`to_port`).** The edge model carries a target input port; omitted ⇒ primary
  input. `Flow::indegrees_within` and `reachable_from` ([multi-trigger scope](flow-multi-trigger-reactive-scope.md))
  are computed **per (node, port)**, not per node.
- **Per-input-port `join` policy in the descriptor** (`all` | `any`), defaulting `all` for transforms and
  `any` for `sink`-kind nodes, overridable per port. The merged `flows.nodes` registry carries it; the
  editor renders it.
- **The run engine honours the policy** — `all` = barrier over that port's in-subgraph wired upstreams;
  `any` = release-and-settle once per settled upstream, each firing scoped by a propagated **firing context
  (`fctx`)** so multiplicity survives downstream (the seam above).
- **The firing context is an additive envelope field.** `fctx` rides the wire like `topic`/`parts`
  ([Decision 15](flows-scope.md) precedent), empty in the all-`all` common case; every step-output claim
  key, `${steps.*}` resolution, per-node `lb-jobs` job key, and outbox dedup key is scoped by it.
- **A `link` built-in pair** (`link-out` / `link-in`) — the canonical `any`-policy collector for the
  Node-RED "many sources → one handler, fire per message" pattern, and the escape hatch for OR-fan-in that
  does **not** want a physical wire (wireless virtual edges by name). `debug` ([debug-node scope](debug-node-scope.md))
  gets an `any` primary input for free (a debug node wired from three places prints three times).
- **Carry-forward is defined for both policies.** An `all` join emits only its `emitted` envelope
  ([envelope D4](flow-message-envelope-scope.md) — no ambiguous merge across upstreams). An `any` funnel
  **carries the arriving message's** metadata forward (each firing forwards *that* upstream's `topic` &
  friends) — the Node-RED "metadata survives a join" behaviour, now unambiguous because each `any` firing
  has exactly one incoming message.
- **A save-time lint that flags real mistakes, not policy.** With ports + declared policy, the lint stops
  standing in for a decision: it **errors** on an `all` port with a forgotten binding (data-drop bug), on a
  wire to an undeclared port, and on an `all` port reading an `any` funnel **across firing contexts** (a
  "collect-join" whose semantics are undefined in v1 — hard-error, not warn; see Open Questions), and it is
  **silent** on two wires into an `any` port (the whole point) — not "you have 2 inputs."
- **The canvas paints the port + policy.** Handles are per named input port; an `any` port renders
  distinctly (a funnel glyph) from an `all` port (a join glyph); the wire inspector shows `to_port`.

## Non-goals

- **A new execution runtime.** `any` reuses the CAS-claim + frontier verbatim; the addition is the
  **firing-context (`fctx`) scoping** of the existing claim/binding/job/outbox keys (empty ⇒ today's key).
  No parked runs, no per-event fan-out ([Decisions 9, 15](flows-scope.md) hold).
- **Per-message fan-out of a `split`.** `split`/`join` stay **array-carry** ([Decision 15](flows-scope.md)).
  `any` multiplicity is bounded by the **static wire topology (path count)**, not runtime array length — the
  two are orthogonal and neither reintroduces the fan-out storm.
- **Cross-flow / cross-tab links.** Node-RED's link nodes' headline use is crossing tabs; the v1 `link`
  pair is **intra-flow** (same run, resolved to physical port-targeted edges at save). Cross-flow links
  (one run signalling another) are a separate scope (they need the trigger/event bridge, not this seam) —
  stated so nobody assumes parity.
- **Output-port fan-out policy.** This scope types **input** convergence. An output port already fans to
  all wired dependents; nothing changes there.
- **Cross-run correlation** (an `all` join that waits for trigger A's run *and* trigger B's run) — each
  firing is its own run/message ([multi-trigger non-goal](flow-multi-trigger-reactive-scope.md)); an `all`
  barrier is **within one run** only.
- **A visual "link map" overview** (Node-RED's link-node highlight-all-partners UX) — the `link` pair
  ships functional in v1; the canvas affordance for jumping between partners is a `flows-canvas` follow-up.

## Intent / approach

1. **Edge model (`lb-flows/src/model.rs`).** An edge gains `to_port: Option<String>` (None ⇒ primary
   input) and keeps its `from`/`from_port`/`with` binding. `needs` stays the ordering/dependency edge; the
   port is additive metadata on it. Serialise on the `flow` record; export/import round-trips `to_port`.
2. **Descriptor (`node-descriptor`).** `inputs` widens from `["payload"]` to accept a **table form**
   declaring `join`: `[[node.input]] name = "payload"; join = "any"`. The string shorthand `inputs =
   ["payload"]` stays valid (⇒ `join = "all"`, or `"any"` when the node `kind = "sink"`). The registry
   merge carries the policy; `ajv`/`jsonschema` validate it. Built-in descriptors set: transforms `all`,
   `sink`/`debug`/`link-in` `any`.
3. **Per-port graph helpers (`lb-flows`).** `Flow::port_upstreams(node, port, &subgraph) ->
   Vec<edge>` and `indegrees_within` recomputed per (node, port): an `all` port's indegree is its wired
   in-subgraph upstream count; an `any` port contributes **one releasable unit per settled upstream**, not
   a barrier.
4. **Run engine (`flow-run` / `run_store`) — the firing context.** The frontier release rule becomes
   port-aware: a node with only `all` inputs releases when every port's barrier is met (today's path); a
   node with an `any` input is released **per settled upstream**, minting a firing id and claiming
   `flow_step_output:{run}:{node}@{fctx}`. Thread `fctx` (empty in the all-`all` case ⇒ today's key
   verbatim) through: the CAS claim key, `resolve_node_bindings`' `${steps.*}` lookup (match the upstream
   settle carrying the same `fctx`), the per-node `lb-jobs` job key ([Decision 8](flows-scope.md)), and the
   **outbox dedup key**. **Run-terminal counts slots** (`(node, fctx)`), not nodes. Resume rebuilds the
   partial slot set from the run-store records (the executed-node-lock invariant holds **per slot**).
   **Gated-skip interaction ([Decision 14](flows-scope.md)):** when a `switch` upstream of an `any` port
   gates its wire, that slot **settles `Skipped` (empty, no firing)** rather than leaving the barrier
   unmet — so a 3-wire `any` port with one gated upstream fires **twice** and the run still reaches
   terminal (terminal-detection must settle gated-empty slots, not hang on them).
5. **`link` built-in pair (`flows/builtins/link.rs` + `flows/link.rs`).** `link-out {target}` and
   `link-in {name}` — a `link-in` has an `any` primary input and receives every `link-out` naming
   it, as a **virtual edge resolved at run load into normal port-targeted `needs`** (so the engine
   sees ordinary wires; the "wireless" part is editor sugar). **As built (Slice 3):** resolution is
   run-load (`coordinator::start`/`drive` call `Flow::resolve_links` on a transient copy — the
   persisted flow keeps the author's link nodes intact so the editor round-trips and a deleted
   `link-out` can never leave a stale wire), NOT a save-time mutation; save-time only
   `validate_links`s the topology (a `link-out` targeting a missing `link-in`, a wire from a
   `link-out`, a dead `link-in`). Same ws wall, same run, no new cap. (The scope's original
   save-time wording was rejected — see the Slice 3 session for the bugs that motivated the move.)
6. **Carry-forward (`resolve_node_bindings`).** `all` port ⇒ [envelope D4](flow-message-envelope-scope.md)
   (emit only `emitted`). `any` port ⇒ carry the **single arriving** upstream's non-`payload` fields
   forward (unambiguous — one message per firing).
7. **Save-time lint (`flows.save`).** Replace "node has N inputs — bind `payload`" with: `all` port with a
   wired upstream and no binding ⇒ **error** (data-drop); wire to an undeclared input port ⇒ **error**;
   `all` port whose upstream reaches through an `any` funnel (a cross-firing-context "collect-join",
   undefined in v1) ⇒ **error** (not a warn — a warn implies it does something, and whatever the engine
   happens to do becomes de-facto behaviour). Two wires into an `any` port ⇒ **valid, silent** (the whole
   point).
8. **Canvas (`flowGraph.ts` / `FlowNodeView`).** Per-named-port handles; `any` vs `all` glyph; wire
   inspector shows `to_port`; the palette shows a node's input ports + policies from `flows.nodes`.
   **As built (Slice 4):** `joinOf`/`effectiveInputPorts` mirror the host `join_of`; a single-port
   node keeps one anonymous handle (back-compat), a multi-port node stacks named handles (primary
   anonymous, non-primary `id = portName`); each handle wears a funnel (`any`) / merge (`all`) glyph;
   `flowToEdges` labels a named-port wire with its `toPort` (the wire-inspector surface); the palette
   shows each port + policy mark; `link-out`/`link-in` render as their built-in descriptors.

## How it fits the core

- **Tenancy / isolation (rule 6):** unchanged surface — ports + policy are fields on the existing
  ws-scoped `flow` / `flow_step_output` records; the `@{fctx}` suffix stays inside `{ws}:{run}`. **A
  two-session isolation test is mandatory** (a ws-A `any` funnel's per-firing step outputs never reachable
  from ws-B) because the step-output key shape changed.
- **Capabilities (rule 5/7):** **no new verb, no new cap.** Convergence is internal to a run already gated
  by `caller ∩ grant`; a `link` pair runs under `flows.run` like any node. The deny tests carry over — an
  `any` funnel calling an ungranted tool per firing is denied at **each** firing (assert the per-firing
  `Err` under `FailurePolicy`).
- **Symmetric nodes (rule 1):** pure engine/data change; no role branch, no `if cloud`.
- **One datastore / state vs motion (rule 3):** ports + policy are **state** on the `flow` record; the
  per-firing step outputs are **state** in the run-store; the firings are **motion** on the run. No new
  table, no new persistence layer — the `@{fctx}` key is an additive suffix on an existing record.
- **MCP surface (§6.1):** **no new verbs.** `flows.save`/`flows.get` round-trip `to_port` + input `join`;
  `flows.nodes` returns the per-port policy in the descriptor; `flows.runs.get` shows the per-firing
  settles of an `any` node and its downstream (each `(node, fctx)` slot as its own step outcome, the `fctx`
  labelling which firing — so the debug story stays legible one hop past the funnel, not just at it). No
  CRUD/list/batch addition — this is the data model under the existing surface. Live-feed: the existing
  `flows.watch`/`flows.debug.watch` streams each firing as its own settle (no new route).
- **Durability:** unchanged — an `any` funnel feeding a must-deliver `sink` still routes each firing's
  effect through the **outbox**; the **`fctx`-scoped** outbox dedup key makes each of the N firings its own
  idempotent delivery (not one delivery swallowing the rest).
- **One responsibility per file (rule 8):** the change lands as focused edits — `model.rs` (edge +
  `to_port`), a `descriptor/input.rs` (the input-port table + policy), `flows/graph_ports.rs` (the per-port
  helpers), a `flows/firing_context.rs` (mint/extend/carry `fctx`), the `run_store` claim-key seam,
  `flows/builtins/link.rs` (the pair), and the `flowGraph` port render — no `utils`/`common` catch-all.
- **SDK/WIT impact:** **flag.** The `[[node]]` manifest gains an optional `[[node.input]]` table (join
  policy per port) — **additive** to the frozen descriptor block, no new WIT world (node execution still
  rides `tool.call`/`host.call-tool`). An extension node that wants OR-fan-in declares `join = "any"` on an
  input port; one that says nothing keeps `all`. Document the convention in the node-descriptor public doc;
  it is manifest data, not a WIT change.
- **No mocks / no fake backend (§9):** every test drives a **real** run over the real store/bus/jobs/outbox
  and asserts real per-firing (`@{fctx}`) step-output records + real per-firing settles; no `*.fake.ts`.

## Example flow — three sources funnel into one debug (Node-RED OR), plus a real join (AND)

Flow `funnel-demo` (ws `kfc`): `mqtt-a → debug`, `mqtt-b → debug`, `cron-c → debug`; and separately
`sensor-hi + sensor-lo → avg(rhai)`.

1. **`debug` has one input port, `join = "any"`** (sink default). Three wires land on it, all
   `to_port = "payload"`.
2. **`mqtt-b` settles first.** The frontier releases `debug`, mints firing `debug#mqtt-b`, claims
   `flow_step_output:{kfc}:{run}:debug@debug#mqtt-b`, and `debug` fires **once** with `mqtt-b`'s envelope
   (its `topic` carried forward, `fctx = debug#mqtt-b`). The panel prints it.
3. **`mqtt-a` then `cron-c` settle.** `debug` fires **again** for each, under `@debug#mqtt-a` and
   `@debug#cron-c`. Three messages in, three prints out — Node-RED behaviour, in one durable run,
   exactly-once per firing on redelivery. Had `debug` fed a downstream `W`, `W` would settle three times too
   (`W@debug#mqtt-a`, …), each reading its own firing's message — the multiplicity **propagates** via `fctx`.
4. **`avg` has one input port `join = "all"`** with wired upstreams `sensor-hi`, `sensor-lo`. It is a
   **barrier**: it fires **once**, when both have settled, reading
   `with: {hi: "${steps.sensor-hi.payload}", lo: "${steps.sensor-lo.payload}"}`. A forgotten binding here
   is a **lint error** (data-drop), not a silent one-of-two pick.
5. **Author never wrote a policy.** `debug` funnels because it is a sink; `avg` joins because it is a
   transform. The safe default is the default; OR is a node choice, not a lint accident.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`) — real store (`mem://`), real caps/jobs/outbox, no
fakes:

- **Capability-deny** — an `any` funnel whose downstream `tool` node lacks a grant is denied at **each**
  firing (assert N `Err` settles under `FailurePolicy`, not one); the no-widening run gate still bites.
- **Workspace-isolation** — a ws-A `any` node's per-firing step outputs (`…:debug@debug#mqtt-a`) are
  unreachable from ws-B across store + `flows.runs.get`.

Plus this slice (`lb-flows` + `lb-host` + UI):

- **Port-labelled edges round-trip (unit + save):** an edge with `to_port` saves, loads, and
  export/import round-trips it; omitted `to_port` resolves to the primary input.
- **`any` fires once per upstream (run, THE headline):** three wires into one `any` port ⇒ **three**
  settles in one run, each carrying its own upstream envelope + carried `topic`; run stays one `flow_run`,
  reaches terminal, no park. Fail-before: today's engine settles the node **once**.
- **Multiplicity propagates one hop (run, THE seam):** `link-in` (`any`, 3 wires) → transform `W` (one
  wire) ⇒ **`W` settles three times**, each `W@<fctx>` reading its own firing's `link-in` message; a naive
  depth-1 `#{upstream}` scheme settles `W` **once** — this is the fail-before that proves the firing context.
- **Diamond/nested funnels (run):** two `any` hops compose ⇒ path-count firings, each with a distinct
  extended `fctx`; deterministic, one run.
- **`any` exactly-once per firing (run):** redeliver one upstream's message ⇒ the `@{fctx}` claim
  no-ops (no double-print), one hop past the funnel too; a *different* upstream still fires.
- **Gated-skip × `any` (run, Decision 14 interaction):** 3 wires into an `any` port, one upstream gated by
  a `switch` ⇒ that slot settles `Skipped` (empty), the node fires **twice**, and the run **reaches
  terminal** (no hang on the gated slot).
- **`all` barrier unchanged (run):** a 2-upstream `all` join fires exactly once when both settle;
  induced-fan-in over a subgraph ([multi-trigger scope](flow-multi-trigger-reactive-scope.md)) still counts
  in-subset upstreams **per port**; the all-`all` claim key is `…:{run}:{node}` (empty `fctx`) byte-for-byte.
- **Policy is the default, not inferred (save):** a `sink` node with 2 wires **saves green** (no lint); a
  `transform` `all` port with a wired upstream and no binding is a **lint error**; a wire to an undeclared
  port is a **lint error**; an `all` port reaching through an `any` funnel (collect-join) is a **hard error**.
- **`link` pair (run):** three `link-out{target:"t"}` and one `link-in{name:"t"}` ⇒ the `link-in` fires
  three times (virtual edges resolve to `any`-port wires); ws-walled (a ws-B `link-out` never reaches a
  ws-A `link-in`).
- **Outbox dedup per firing (run):** an `any` funnel feeding a must-deliver sink ⇒ **N** idempotent outbox
  deliveries (one per `fctx`), not one; redelivery of a firing no-ops its own delivery.
- **Carry-forward per policy (run):** an `any` firing forwards **its** arriving `topic` — verified **one
  hop past** the funnel (not just at it); an `all` join emits only `emitted` (no cross-upstream metadata
  merge) — [envelope D4](flow-message-envelope-scope.md) preserved.
- **`flows.runs.get` legibility:** an `any` node **and its downstream** show each `(node, fctx)` slot as its
  own step outcome, labelled by firing (the debug story stays readable past the funnel).
- **Frontend (Vitest, real spawned gateway):** the canvas paints per-named-input-port handles with the
  `any` vs `all` glyph; the wire inspector shows `to_port`; a 3-into-1 `any` flow saves and the debug panel
  ([debug-node scope](debug-node-scope.md)) tails three messages.

**Test sweep (structural change):** every test that builds an edge as a bare node-id `needs` gains the
implicit primary `to_port` (unchanged behaviour); the join/fan-in tests split into an `all` set (barrier)
and a new `any` set (funnel). Touch list: `flows_run_test`, `flows_nodes_test`,
`flows_multi_trigger_test`, `flows_sink_test`, `flows_runtime_control_test`, the `binding.rs` tests, and
`flowGraph.test.ts`. Green means the sweep is complete.

## Risks & hard problems

- **The firing context (`fctx`) is *the* load-bearing seam — a depth-1 suffix is a trap.** Keying an
  `any` node's step output by immediate upstream disambiguates **only at the funnel** and breaks one hop
  downstream (a node with a single wire from the funnel has one slot and can settle only once, and
  `${steps.funnel.payload}` is ambiguous across its firings). The fix is a **propagated** firing id carried
  in the envelope (`fctx`) that scopes **every** downstream claim key, `${steps.*}` resolution, per-node
  job key, and outbox dedup key — empty in the all-`all` case so today's paths are byte-identical. This is
  the piece most likely to be under-scoped mid-build; it is designed in now. Test the **propagate-one-hop**
  and **redelivery-per-firing-downstream** cases explicitly — they are the ones a naive suffix passes at
  the funnel and fails past it. `fctx`/policy are read once at run start and **pinned with the version**
  (Decision 1) so a descriptor edit mid-run cannot flip an in-flight node's key shape.
- **Funnel multiplicity propagates (correctly, and is statically bounded).** A node downstream of an `any`
  funnel inherits its multiplicity (fires per funnel firing), composing **multiplicatively along path
  count** through diamonds/nested funnels. It is **statically bounded by the wire topology, never by event
  volume** — but it is *not* ≤ indegree, so the doc says "path count," not "wire count." Surface each firing
  in `flows.runs.get` (labelled by `fctx`) so "why did my sink write three times?" is answerable, and
  **hard-error** (not warn) the `all`-join-over-`any`-funnel collect-join whose semantics are undefined in
  v1.
- **Resume across per-firing slots.** Resume must rebuild an `any` node's (and its downstream's) partial
  slot set — some firings settled pre-suspend, some after — from the run-store, keyed by `(node, fctx)`. The
  executed-node-lock invariant now holds **per firing slot**, not per node. Reuse the CAS-claim idempotency;
  test a suspend between two `any` firings **and** between a funnel firing and its downstream.
- **Primary-input default must be unambiguous.** "Omitted `to_port` ⇒ primary input" requires every node
  to have a well-defined **first** input port. A node with **zero** inputs (`trigger`, `flipflop`) rejects
  any inbound wire at save (a lint), so the default is only consulted where a port exists.
- **Descriptor-form migration.** The string `inputs = ["payload"]` shorthand must keep meaning the same
  policy it does today (`all` for transforms, `any` for sinks) so unchanged descriptors behave unchanged;
  only an explicit `[[node.input]] join = …` overrides. Grep every built-in `inputs = […]` and confirm the
  default lands right (a `sink` silently becoming `all` would break the funnel intent).

## Open questions

- **`any`-port firing order determinism.** Firings settle in upstream-completion order (non-deterministic
  under concurrency). **Resolved (Slice 2):** accept arrival order (matches Node-RED); document it; a
  node needing order uses an `all` join + explicit sort. No built-in relies on order.
- **Should `link-in` allow an `all` policy?** A `link-in` is conceptually a funnel (`any`). **Resolved
  (Slice 3):** v1 `link-in` is `any`-only (the descriptor declares `join = "any"` on `payload`); a
  join-over-links caller has not appeared. If one does, a deliberate second node kind is the primitive.
- **Mixed-policy multi-port nodes.** A future node with a `left` (`all`) and a `control` (`any`) port is
  expressible in this model. **Resolved (Slices 2–3):** the model is open (it costs nothing), but v1
  ships no mixed-policy built-in — every built-in is single-port. The canvas already paints it
  per-port (Slice 4) for an extension that declares one.
- **Collect-join (`all` over an `any` funnel) — defined or forbidden?** **Resolved by what the runtime
  does (Slice 3), which overturned the original "v1 hard-error" proposal.** An `all` port whose ONLY
  upstream is a funnel **inherits the multiplicity** (fires once per funnel firing, each carrying one
  envelope) — coherent via the `fctx` propagation, and exactly the propagate-past-the-funnel headline
  topology (`W` is an `all` transform downstream of `link-in`). Hard-erroring "an `all` port reaching
  through an `any` funnel" would forbid that load-bearing seam itself. The genuine footgun is
  narrower — an `all` port **joining a funnel-carrying upstream with a different-`fctx` upstream**
  (the barrier slot never completes). That needs a full `fctx`-lineage reachability analysis a
  save-time heuristic can't soundly approximate; **left as a named follow-up** (collect-join
  detection), not a silent gap. A true "collect-into-array" join (barrier over a funnel's COMPLETE
  firing set) is a deliberate primitive if a caller appears.
- **`flows.patch_run` and port policy.** **Resolved (Slice 3):** out of `patch_run`'s scope. A policy
  is descriptor-level (`input_ports`); `patch_run` is config-only (Decision 1/12). A policy change is
  structural ⇒ a new flow version, never a live-run patch — the shapes don't overlap, so no validator
  is needed.

## Skill doc

**N/A for a *new* skill.** This scope adds **no new agent-/API-drivable surface** — no new MCP verb, no new
gateway route, no new automatable task. It changes the data model + run semantics under the existing
`flows.*` surface. The implementing session **must update** the existing flows skill/how-to (the `flows.save`
edge shape now carries `to_port`; `flows.nodes` descriptors carry input `join`) if one exists — a stale
example of the old node-id-only edge is a finding — but it authors no new `skills/<name>/SKILL.md`.

## Debugging entries to log (this session)

- `debugging/flows/multi-input-node-fires-once-not-per-message.md` — the node-id-edge / single-claim root
  cause and the port-labelled-edge + per-port-`join` + propagated-firing-context (`fctx`) fix.

## Related

- [`flows-scope.md`](flows-scope.md) — the spine; **Decision 8** (the CAS-claim exactly-once this
  sequence-keys), **9** (one-shot runs, no park — `any` stays inside it), **14** (edge-gating `switch`,
  which **deferred** port-labelled edges — this scope executes that deferral), **15** (array-carry, the
  orthogonal fan-out axis).
- [`flow-message-envelope-scope.md`](flow-message-envelope-scope.md) — **D3** (the auto-wire + ≥2-input
  lint this replaces with a real policy), **D4** (carry-forward, now defined per `all`/`any`).
- [`flow-multi-trigger-reactive-scope.md`](flow-multi-trigger-reactive-scope.md) — per-trigger induced
  subgraph + `indegrees_within`, now recomputed **per (node, port)**.
- [`node-descriptor-scope.md`](node-descriptor-scope.md) — the `inputs[]` ports that gain the
  `[[node.input]]` table + `join` policy; the extension-node convention (declare `join` to opt into OR).
- [`debug-node-scope.md`](debug-node-scope.md) — the `debug` sink that gets an `any` primary input (wired
  from three places, prints three times).
- [`flows-canvas-scope.md`](flows-canvas-scope.md) — the per-port handles + `any`/`all` glyph + link-map
  follow-up.
- README **§3** (rules 1–8), **§6.1** (API shape), **§6.10** (jobs — the per-(node,`fctx`) job key),
  **§13** (manifest is the contract — the additive `[[node.input]]` block).
- Promotes to `doc-site/content/public/flows/flows.md`.
</content>
</invoke>
