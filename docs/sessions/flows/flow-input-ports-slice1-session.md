# Flows — port-labelled edges + per-input-port join policy, Slice 1 (the data-model foundation) (session)

- Date: 2026-07-09
- Scope: ../../scope/flows/flow-input-ports-scope.md
- Stage: S8+ (flows shipped; this is the structural foundation for the Node-RED multi-input model)
- Status: done (Slice 1 of 4)

## Goal

`flow-input-ports-scope.md` is a large, **structural** change to the flow edge model: an edge gains
a target input port (`to_port`), each input port declares a join policy (`all` join vs `any` funnel),
and the run engine honours `any` via a propagated **firing context (`fctx`)** — the load-bearing seam
that makes Node-RED's "fire-per-message" multiplicity survive one hop past the funnel. Plus a `link`
built-in pair and a canvas that paints per-port handles + the policy glyph.

That whole scope is genuinely multi-session. **This session lands Slice 1: the data-model
foundation** — port-labelled edges (`to_port`), the descriptor join-policy table, the per-port graph
math, and the registry-aware port lints — with **`all`-defaulted behaviour byte-identical** and **no
silent gaps** (no built-in declares `any` yet; the runtime still treats every multi-input node as a
barrier, so the existing join lint still holds). Slices 2–4 are named follow-ups, not silent gaps.

## The slicing decision (recorded)

The scope's 8 intent steps split into four coherent slices. The fault-line that forced the split:
**shipping the `any` policy declaration without the runtime that honours it would be a silent gap**
(the lint would let a 3-wire-into-`debug` flow save green, but the engine would still join-and-fire-
once). So the `any` default flip, the lint relaxation, and the runtime `fctx` seam must land
**together**. That made Slice 1 the pure data model (which is honest on its own: it carries the policy
as data the editor renders, with every port effectively `all`), and Slice 2 the runtime.

- **Slice 1 (this session):** edge `to_port` (additive `inputs` metadata) + descriptor `join` policy
  table (`InputPort{ name, join }`) + per-port graph helpers (`edges_into`,
  `indegrees_within_by_port`) + `[[node.input]]` manifest parse + the registry-aware save lints
  (undeclared-port error, no-input-port error) + the UI wire types + `flowGraph` round-trip. Every
  built-in stays `all`; the join lint is unchanged. Byte-identical run behaviour.
- **Slice 2 (next): the `any` runtime + the firing-context `fctx` seam.** The frontier releases an
  `any` port per settled upstream; the firing context propagates down every wire so multiplicity
  survives one hop past the funnel; claim/binding/job/outbox keys scope by `fctx` (empty in the
  all-`all` case ⇒ today's key byte-for-byte); run-terminal counts `(node, fctx)` slots; gated-skip
  interaction; flip `sink`-kind ports (incl. `debug`) to `any` + relax the lint; the mandatory
  cap-deny + ws-isolation tests against the new `@{fctx}` step-key shape.
- **Slice 3:** the `link` built-in pair (`link-out{target}` / `link-in{name}`) — virtual edges
  resolved at save into `any`-port wires.
- **Slice 4:** the canvas — per-named-port handles, the `any`/`all` glyph, the wire inspector showing
  `to_port`, the link-map follow-up.

## What changed (Slice 1)

### Edge model (`lb-flows/src/model.rs`)
- New `InputEdge { from, to_port: Option<String> }` — a wired edge's target input port metadata
  (Axis 1). `to_port = None` ⇒ the node's primary input port (the first declared input), so a
  pre-ports single-input linear flow is unchanged.
- `Node.inputs: Vec<InputEdge>` — additive, `#[serde(default, skip_serializing_if = "Vec::is_empty")]`,
  so a pre-ports flow deserialises with empty `inputs` ⇒ every edge is the primary port (the
  no-migration / back-compat property). `needs: Vec<String>` **stays** the DAG topology (scope intent
  #1: "needs stays the ordering/dependency edge; the port is additive metadata on it").
- `Node::to_port_from(upstream)` — resolves an edge's `to_port` (None ⇒ primary).
- New `DagError::PortForUnknownNeed` + a `validate_flow` check: every `InputEdge.from` must be in the
  node's `needs` (port metadata must agree with the topology — a label for a wire that isn't wired is
  a mistake, caught at save).
- New per-port graph math (pure — no registry needed): `Flow::edges_into(node)` → `Vec<(from,
  to_port)>`, and `Flow::indegrees_within_by_port(set)` → per-`(node, port)` barrier counts (the
  per-port grouping a join policy reads; the policy decision itself is host-side, where the registry
  lives). `None` groups under the primary-port sentinel.

### Descriptor (`lb-flows/src/descriptor.rs`)
- New `JoinPolicy { All, Any }` (Axis 2). `All` = barrier (today's behaviour, the default); `Any` =
  funnel (Node-RED fire-per-message). Slice 1 declares the type only.
- New `InputPort { name, join }` — the `[[node.input]]` table form.
- `NodeDescriptor.input_ports: Vec<InputPort>` — additive, skipped when empty (the clean all-`all`
  wire shape). `with_input_ports()` builder.
- `NodeDescriptor::primary_input()` — the first declared input (the edge default target; None for a
  trigger/source).
- `NodeDescriptor::join_of(port)` — the declared policy for a port, defaulting `All` (None ⇒
  primary). Slice 1: every port resolves to `All` (no built-in declares `Any`).

### Manifest (`lb-flows/src/node_block.rs`)
- `NodeBlock.input_ports: Vec<InputPort>` — the additive `[[node.input]]` block, serde-defaulted.
  `validate_node_block` lifts it into the descriptor. The string `inputs = [...]` shorthand keeps its
  meaning (empty table ⇒ `All`).

### Save lints (`lb-host/src/flows/save.rs`)
- `validate_node_configs` now also lints the port topology against the registry (Axis 1):
  - a wire's `to_port` that is not a declared input port on the node's descriptor ⇒ **error**
    (`undeclared input port`);
  - a node with ≥1 incoming wire but no declared input port (a misconfigured trigger/source) ⇒
    **error** (`declares no input port`).
- The existing join lint (≥2 upstreams without a `payload` binding) is **unchanged** in Slice 1 —
  `any` is declaration-only this slice, so a multi-input node is still a barrier and must still bind
  `payload`. Slice 2 relaxes this for `any` ports together with the runtime. (No silent gap.)

### UI
- `ui/src/lib/flows/flows.types.ts` — `JoinPolicy`, `InputPort`, `InputWire` types; `NodeDescriptor`
  gains optional `inputPorts`; `FlowNode` gains optional `inputs`.
- `ui/src/features/flows/flowGraph.ts` — `flowToEdges` now carries the wire's `to_port` on the React
  Flow edge's `targetHandle` (null ⇒ primary); `nodesToFlowNodes` round-trips it back, emitting a
  `to_port` entry ONLY for non-primary wires (a primary-only flow round-trips to the clean pre-ports
  shape — no `inputs` field).

## Tests (real store/caps/gateway, rule 9) — all green

**`lb-flows` unit (`cargo test -p lb-flows --lib`): 91 passed** (was 86; +5):
- model: `input_edge_round_trips_with_to_port`, `node_with_inputs_round_trips_and_a_pre_ports_node_loads_unchanged`,
  `edges_into_returns_per_port_wires`, `indegrees_within_by_port_groups_per_port`, `rejects_a_port_entry_for_a_wire_that_is_not_wired`.
- descriptor: `join_defaults_to_all_for_every_port_and_kind`, `input_ports_table_overrides_the_default`,
  `primary_input_is_the_first_declared_port`, `descriptor_with_input_ports_round_trips`.
- node_block: `input_ports_table_carries_the_join_policy`.

**`lb-host` flows integration: 127 passed across 13 binaries** — the test sweep (every `Node { … }`
literal gained `inputs: Vec::new()`; the `NodeBlock { … }` literal gained `input_ports: vec![]`).
New port-lint cases in `flows_run_test.rs` (+4 → 29 total):
- `save_rejects_a_wire_to_an_undeclared_input_port`,
- `save_accepts_a_wire_to_a_declared_input_port` (round-trips via `flows.get`),
- `save_rejects_a_wire_into_a_node_with_no_input_ports`,
- `save_still_requires_a_payload_binding_for_a_multi_input_join` (the no-silent-gap guard).

**UI (`pnpm exec vitest`):** `flowGraph.test.ts` **15 passed** (was 12; +3): `loads a stored toPort
onto the edge's targetHandle`, `round-trips a named toPort through load → export`, `a pre-ports flow
round-trips with primary handles and no inputs field`. `pnpm exec tsc --noEmit` adds **no new
errors** on flows/lib-flows files.

**`cargo fmt` clean.**

## Pre-existing reds fixed / surfaced (NOT this slice's behaviour change)

- `flows_nodes_test::registry_has_all_builtins_with_no_installs` (and its two siblings) was failing
  on clean master: the test's `BUILTINS` const never got `debug` appended when the debug node shipped
  (2026-07-08). Since this slice touches `flows_nodes_test.rs` (the `NodeBlock` literal), the stale
  expectation was corrected — `debug` appended → 5/5 green. Verified red on master first.
- `DebugValueView.test.tsx` (2 cases) fails identically on clean master — NOT this slice (no debug
  code touched).

## Definition of done (Slice 1)

- [x] Scope Slice 1 (edge `to_port` + descriptor `join` table + per-port graph math + manifest parse
      + save lints + UI types/round-trip) built end to end.
- [x] `all`-defaulted behaviour byte-identical; **no silent gap** (no `any` default flipped; the join
      lint unchanged; a multi-input node still settles as a barrier).
- [x] FILE-LAYOUT held (focused edits per file; no `utils`/`common` catch-all).
- [x] No `if cloud`; symmetric nodes unaffected (pure data-model change).
- [x] No core knowledge of any extension.
- [x] No mock data / no fake backend — real store/caps/gateway, seeded via the real write path.
- [x] Tests on both backend and frontend, green output pasted above.
- [x] Session doc filled.
- [x] Scope OQs refreshed (Slice-2 runtime OQs flagged as the next decision point).
- [x] STATUS.md moved.

Slice 1 is **honestly complete for its boundary**: it does NOT deliver the headline Node-RED OR
funnel — that is Slice 2's load-bearing `fctx` seam, named here, not silently deferred. The scope's
own debug entry (`debugging/flows/multi-input-node-fires-once-not-per-message.md`) belongs to Slice 2
(it documents the runtime root-cause + the firing-context fix); it is not logged in Slice 1 because
nothing in the runtime changed this session.

## Open questions carried to Slice 2

The scope's OQs are almost all runtime-`any` questions (firing-order determinism, `link-in` `all`
policy, mixed-policy nodes, the collect-join, `patch_run` policy rejection). They are unresolved by
design in Slice 1 (no runtime to ask them of) and become the Slice-2 decision points. One Slice-1
note: the additive `inputs` parallel to `needs` is the scope's literal wording ("needs stays … the
port is additive metadata on it") — Slice 2 will confirm it stays clean when the engine reads port
labels at release time; if the redundancy proves awkward, collapsing to a single edge list is the
documented escape hatch (the scope sanctions a clean cut).
