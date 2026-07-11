# Flows scope — plain wiring: remove the `link` pair, every port fires per message (the actual Node-RED model)

Status: **shipped 2026-07-12** (branch `flow-plain-wiring`; session
[`flow-plain-wiring-session.md`](../../sessions/flows/flow-plain-wiring-session.md)). Peer-reviewed
2026-07-12 (findings folded in — the switch matched-release fix, the binding-lineage decision, the
run-load guard, the dead-code sweep). Promoted to
[`public/flows/flows.md`](../../../doc-site/content/public/flows/flows.md).

The `flow-input-ports` build ([flow-input-ports-scope.md](flow-input-ports-scope.md), shipped
2026-07-09) delivered the machinery the user asked for — many wires into one input port — but
also shipped **two palette nodes the user never asked for**: the `link-out`/`link-in`
"wireless" pair. Worse, the machinery's *defaults* still push authors toward those nodes:
a non-sink node's input port defaults to the **`all` barrier** (fire once when every upstream
settles, payload binding required by lint), so wiring three sources straight into an ordinary
node does **not** behave like Node-RED — only `sink`-kind nodes funnel. This scope removes the
two nodes and flips the default so that **plain wiring is the whole story**: a node has an
input port, any number of wires can land on it, and the node fires once per arriving message.
Same on the way out: one output port, any number of wires, every downstream fires. No special
nodes, no policy to think about. Exactly Node-RED, exactly the original ask.

## Goals

- **Delete `link-out` and `link-in`** — descriptors, the virtual-edge resolver/validator, the
  five link `DagError` variants, their run-load/save call sites, their tests, and every doc/UI
  reference. The palette shows no `Links` category.
- **Every input port fires per message by default.** The default join policy becomes **`any`**
  for *every* node kind (today: `any` for sinks only, `all` otherwise). Three wires into a
  `rhai` node ⇒ three firings, each carrying the arriving message's envelope — identical to
  wiring three sources into a Node-RED function node.
- **The `switch` matched-release path honours the policy** (peer-review blocker). Today a
  `switch` match releases its dependent through the **barrier** path unconditionally
  (`switch.rs:59-61` → `run_store.rs touch_barrier_slot`), ignoring the port's join policy.
  After the flip that hangs the mainline topology (switch + two plain wires into one node: the
  match seeds a `Pending` barrier slot with indegree 3 that the two `any` firings never touch —
  the run never reaches terminal). The matched release must mint an `any` firing
  (`triggered_by = switch`) when the dependent port is `any`. This bug is latent *today* for a
  multi-wire `any` sink downstream of a `switch`; the flip just promotes it to mainline.
- **`${steps.X}` bindings resolve along the firing's lineage** (peer-review finding 5). Today
  a non-arriving upstream silently binds `null` (`binding.rs` — there is no resolution error),
  and an `any` firing's auto-wire map records **only the triggering upstream**
  (`run_store.rs:683-684`), so with `any` universal even a linear chain's grandparent binding
  (`b` reading `${steps.trigger.x}` through `a`) would go null. Decision: **resolution walks
  the firing-context lineage** — `${steps.X}` matches X's settle whose `fctx` is an ancestor
  of (a prefix of, falling back through parent contexts to `""`) the current firing's — so
  linear/tree flows keep full binding expressivity, and only a genuine **cross-branch**
  reference (a sibling wire of a multi-wire port, never in the lineage) resolves null — and
  *that* gets a **save lint** (a binding referencing a node that can never be in the firing's
  lineage is a data-drop mistake, flagged, not silent). Rejected: accept silent null (a
  data-drop the lint rules exist to catch); rejected: forbid all non-arriving references
  (breaks linear-flow grandparent bindings — far more restrictive than Node-RED, where the
  data would simply ride `msg`).
- **Output fan-out stays as-is and gets stated.** One output port wired to N downstreams fires
  all N — already true (`release_dependents` iterates every wired dependent); this scope pins
  it with a test. Node-RED clones `msg` per wire; lb records one **immutable** envelope per
  settle and each dependent firing reads its own copy — no shared mutation is possible, so the
  semantics are equivalent to cloning.
- **The author never sees a join policy.** With `any` universal, the funnel/merge port glyphs
  and the "bind payload on a multi-input join" lint disappear from the default authoring
  experience. The canvas paints a port as just a port.
- **Dead code goes with it** (peer-review finding 13). The by-port indegree helpers
  (`indegrees_within_by_port`, `edges_into` — zero callers outside `model.rs`; the real engine
  is `to_port_from` + `join_of` + `barrier_indegree`), the legacy `ready_frontier` helper
  (`coordinator.rs:144-157`), and the `#[allow(dead_code)] DagError::UnboundJoin` variant all
  join the deletion.

## Non-goals

- **Ripping out the `to_port`/`fctx` machinery.** Port-labelled edges (`to_port`, which feeds
  `PortForUnknownNeed` validation and extension multi-port nodes) and the propagated firing
  context (`fctx`) are **kept — they are the engine of per-message firing**, not the mistake.
  The mistake was the two nodes and the `all` default sitting on top of them.
- **Removing `JoinPolicy::All` from the model.** The enum stays as a descriptor-level opt-in
  (an extension node may still declare `join = "all"` on a port) — the barrier machinery is
  needed anyway (the `switch` fix reuses it). No built-in declares it after this scope, so it
  vanishes from the default experience. Rejected alternative — delete the variant entirely: it
  costs nothing to keep, the `[[node.input]]` manifest convention is already published
  (SDK/WIT additive block), and deleting it would be a breaking manifest change for zero
  simplification of the user-facing surface.
- **A barrier/join replacement node.** Node-RED joins with an explicit `join` node; lb already
  has `join`/`batch` in the sequence pack for array-shaped joins. If a true "wait for both
  branches then combine" primitive is wanted later, it is a deliberate new node in its own
  scope — not a default, not a lint.
- **Cross-flow signalling.** The `link` pair's Node-RED headline (crossing tabs) was already a
  non-goal of the input-ports scope; removing the pair removes nothing anyone has.
- **Migration/heal of saved flows containing link nodes.** Flows is in development (the
  input-ports scope set the no-back-compat precedent). A saved flow containing a `link-out`/
  `link-in` fails the next save with a clear "unknown node kind" error. **And because the
  cron/source reactors execute persisted flows without a re-save** (`react_cron.rs:47`,
  `react_source.rs:46` — save-time validation never runs for an already-enabled flow), the
  removal adds a cheap **run-load unknown-kind guard**: `coordinator::start`/`drive` validate
  node kinds against the merged registry and fail the run with the same clear error, instead
  of letting removed kinds fall into the extension-dispatch leg
  (`execute_node/mod.rs:236-245`) and settle as confusing unknown-tool denials.
- **Named Node-RED divergences we accept (state them, don't fix them):**
  - **No feedback wires.** lb hard-rejects cycles (`DagError::Cycle`); Node-RED allows them.
    Pre-existing engine posture, orthogonal to this scope.
  - **Duplicate wires collapse.** Two wires from the same output to the same input are one
    firing in lb (the firing id is deterministic per `(node, upstream, parent_fctx)` —
    `run_store.rs:393-397`); Node-RED delivers two messages. Arguably saner; named.
  - **Retained inputs override the wire.** A `flow_input` overlay wins over the arriving
    message (`run_store.rs:689,759-798`, Decision 9) — under per-message firing a retained
    payload means N firings with N identical payloads. Pre-existing and deliberate.
  - **Arrival-order firing** (non-deterministic under concurrency) — matches Node-RED, already
    documented in the input-ports scope.

## Intent / approach

1. **Remove the pair (`lb-flows`).** Delete `builtins/link.rs` and `link.rs`
   (`resolve_links`/`validate_links`); drop the module + `extend` + `EXPECTED` entries in
   `builtins/mod.rs` and fix the built-in count assertion (35 → 33; the `12+20+1+2` comment at
   `mod.rs:104` loses its `+2`); drop the re-exports in `lib.rs:44`; drop the five link
   `DagError` variants in `model.rs:416-436` (`LinkOutMissingTarget`, `LinkOutNoTarget`,
   `WiresFromLinkOut`, `LinkInDead`, `LinkNameCollision`). Same sweep takes the dead code:
   `indegrees_within_by_port` + `edges_into` (`model.rs:335-380`), `ready_frontier`
   (`coordinator.rs:144-157`), `DagError::UnboundJoin` (`model.rs:405-410`).
2. **Remove the call sites + add the run-load guard (`lb-host`).** `coordinator.rs:37,85`
   (`start`/`drive`) stop calling `resolve_links` (the transient-copy resolution step goes
   away entirely — take care not to disturb the version-pinning order around it: policies are
   pinned at run start with the version); `save.rs:35` stops calling `validate_links`;
   `execute_node/mod.rs:285` loses the `link-in` dispatch leg. In the freed `start`/`drive`
   slot, validate node kinds against the merged registry (the run-load unknown-kind guard).
3. **Flip the default — all four sites (`lb-flows` + `lb-host` + UI).** `join_of`
   (`descriptor.rs:205-208`) returns `Any` for every port unless the descriptor explicitly
   declares `all` — the `NodeKind::Sink` branch is **deleted, not inverted** (no dead
   kind-branch left to re-grow policy-by-kind). The string shorthand `inputs = ["payload"]`
   now means `any` everywhere. The other three defaults flip with it: the missing-descriptor
   fallback `.unwrap_or(JoinPolicy::All)` at `run_store.rs:307`, the UI mirror
   `flowGraph.ts:39 joinOf`, and the prose mirrors (`descriptor.rs:73-77`,
   `flows.types.ts:45`, the `coordinator.rs:44` doc claim). Audit every built-in descriptor:
   none declares `all` after this change.
4. **Fix the `switch` matched release (`run_store.rs` + `switch.rs`) — the blocker.**
   `release_matched`/`touch_barrier_slot` (`run_store.rs:422-432`) becomes policy-aware: when
   the matched dependent's port is `any`, mint a normal `any` firing slot
   (`triggered_by = switch`, same deterministic id scheme) instead of seeding/decrementing a
   barrier. The barrier path remains for an explicit-`all` port. The gated (skip) side already
   settles `Skipped` per slot and stays as-is.
5. **Retire the barrier-only save lint; add the lineage lint (`save.rs`).** The "≥2 wires into
   an `all` port without a `payload` binding" error (`save.rs:145-156`) now only fires for a
   port that *explicitly* declares `all`; with no built-in declaring it, it is unreachable in
   stock flows. The undeclared-port and wire-into-no-input-port lints stay. **New lint:** a
   `${steps.X}` binding where X can never be in the bound node's firing lineage (X is neither
   an ancestor along the wires into this node nor the node itself — e.g. a sibling wire of a
   multi-wire port, or an unrelated branch) ⇒ **error** (it would silently bind null).
6. **Bindings resolve along the lineage (`resolve_node_bindings` + `run_store.rs`).**
   `${steps.X.*}` matches X's settle whose `fctx` is an **ancestor** of the current firing's
   (equal, or a prefix through the `·`-separated lineage, falling back to `""`) — the
   input-ports scope's "same `fctx`" rule widened to "same lineage." This restores grandparent
   bindings for linear/tree flows under universal `any` (today's `recorded`-map short-circuit
   at `run_store.rs:683-692` covers only the arriving upstream). An `any` firing's auto-wire
   and carry-forward are unchanged (envelope D2/D4: `inputs` is the arriving message; its
   metadata carries forward).
7. **UI (`flowGraph.ts`, `Palette.tsx`, `FlowNodeView.tsx`).** Mirror the default flip in
   `joinOf` (`flowGraph.ts:39`); remove the `Links` palette expectations; the funnel/merge
   `PolicyMark` glyph renders **only** on an explicit-`all` port (i.e. never for built-ins) —
   a port with the default policy gets no glyph. The wire inspector keeps showing `toPort`
   (inert metadata, free). `FlowCanvas` needs no change: `onConnect` already allows many wires
   per handle.
8. **Docs.** Scrub `link-in`/`link-out`/`Links`/resolver references from **`docs/STATUS.md`**
   (:201-253 — it is the "where are we" source of truth) and from
   `public/flows/flows.md:77-95` on promote; sweep the "all-`all` common case" doc-comment
   stragglers (`record.rs`, `runs.rs`, `flows.types.ts:45`). `flow-input-ports-scope.md`
   already carries the supersede note. Append a pointer note to
   `debugging/flows/multi-input-node-fires-once-not-per-message.md` (append-only history —
   don't rewrite it).

Rejected alternative — **remove only the two nodes, keep the `all` default for transforms**:
that leaves the user's actual ask unmet. Three wires into a transform would still barrier and
still demand a binding, and the next session would reinvent a funnel node to escape it. The
default *is* the feature; the nodes were just its symptom.

## How it fits the core

- **Tenancy / isolation (rule 6):** unchanged surface — this deletes nodes and flips a
  descriptor default; every key stays ws-scoped. The mandatory two-session isolation test
  re-runs against a direct (link-free) multi-wire flow.
- **Capabilities (rule 5/7):** no new verb, no new cap, two fewer built-ins under the same
  `flows.run` gate. Per-firing cap-deny (each `any` firing denied independently) carries over
  and must stay green with direct wiring.
- **Symmetric nodes (rule 1):** pure engine/descriptor/UI change; no role branch.
- **One datastore / state vs motion (rule 3):** no table, key, or subject changes. The
  `@{fctx}` step-output key shape is untouched; lineage resolution is a read-side widening.
- **Stateless extensions (rule 4):** N/A — built-ins only.
- **MCP surface (§6.1):** no new verbs. `flows.nodes` returns 33 built-ins (no `Links`
  category); `flows.save` rejects `link-out`/`link-in` as unknown kinds and the run loader
  does the same for armed flows — that *is* the removal contract. No CRUD/list/feed/batch
  addition.
- **Durability:** unchanged — per-`fctx` outbox dedup shipped in input-ports Slice 3 and is
  untouched.
- **No mocks (rule 9):** all tests drive real runs over the real store/bus (`mem://`), real
  saves through the real gateway.
- **One responsibility per file (rule 8):** a deletion plus small focused edits
  (`descriptor.rs` default, `save.rs` lints, the `run_store.rs` matched-release + lineage
  seams, `flowGraph.ts` mirror); no new files expected beyond tests.
- **SDK/WIT impact:** **none breaking.** The `[[node.input]]` manifest block is unchanged and
  still additive; only the *default* when a port declares nothing flips from `all` to `any`.
  Flag it in the node-descriptor public doc: an extension node that silently relied on the
  implicit barrier must now declare `join = "all"`. No published extension does (the block
  shipped three days ago); state it and move on. **Named follow-up (not this scope):** an
  extension multi-port node mixing an explicit-`all` port with other wires hits the
  port-blind `barrier_indegree` + primary-port-only lint (`save.rs:145-148`) — wrong counts,
  leaky lint. Only reachable via the opt-in after this scope; log it as a known limit in the
  node-descriptor doc.

## Example flow — three sources into one function node, no special nodes

Flow `plain-demo` (ws `kfc`): `mqtt-a → rhai`, `mqtt-b → rhai`, `cron-c → rhai`, and
`rhai → debug` plus `rhai → tool`.

1. The author drags three wires onto `rhai`'s input port. **Save is green** — no lint, no
   binding demand, no policy question, no link nodes.
2. `mqtt-b` settles first. `rhai` fires with `mqtt-b`'s envelope (`fctx = rhai#mqtt-b`),
   exactly as Node-RED runs a function node per message.
3. `mqtt-a` and `cron-c` settle; `rhai` fires twice more. Three messages in, three firings,
   exactly-once per firing on redelivery. **Run-count depends on posture:** in a manual/
   whole-graph run (`entry = None`) that is three firings in **one** durable run; in reactive
   posture each source event starts its **own** run scoped to `reachable_from(entry)` —
   three runs of one firing each. Same per-message behaviour either way; tests must assert
   against the right mode.
4. Each `rhai` firing fans out its output to **both** `debug` and `tool` — output-side
   multi-wire, also plain, each reading its own immutable copy of the envelope.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`), real store/bus, no fakes:

- **Capability-deny** — direct 3-wire flow whose downstream `tool` lacks a grant: three
  independent `Err` settles (per-firing deny survives the link removal).
- **Workspace-isolation** — ws-A's per-firing step outputs (`…@rhai#mqtt-a`) unreachable from
  ws-B via store + `flows.runs.get` (re-pin over a link-free topology).
- **Offline/sync:** N/A — no sync-surface change (no new table/subject; single-node engine
  semantics only).
- **Hot-reload:** N/A for extensions (built-ins only) — but the durable-suspend analogue is
  mandatory: **suspend/resume between two `any` firings** over direct wiring (and between a
  funnel firing and its downstream) rebuilds the partial `(node, fctx)` slot set — the
  input-ports resume risk, re-run link-free.

Removal:

- **Registry:** `flows.nodes` returns 33 built-ins; `link-out`/`link-in` absent; no `Links`
  category. Update `flows_nodes_test.rs:107-108`, `EXPECTED`,
  `FlowsCanvas.gateway.test.ts:85-90`, `flowGraph.test.ts:287-288`, and the
  `flows_run_test.rs` link suite (:72, :917-1330).
- **Save + run-load reject the kinds:** saving a flow containing a `link-in` fails with
  unknown-kind; an already-enabled persisted link flow fails at **run load** with the same
  clear error (the reactor path never re-saves) — both asserted.
- **Dead-code sweep:** `resolve_links`/`validate_links`/link `DagError` variants/
  `indegrees_within_by_port`/`edges_into`/`ready_frontier`/`UnboundJoin` gone;
  `cargo build --workspace` clean (no unused warnings papered over).

Default flip (the headline):

- **Transform funnels by default (run):** 3 wires into a `rhai`/`change` node ⇒ **three**
  settles, each with its own upstream envelope + carried `topic` (assert in whole-graph mode:
  one run; add a reactive-mode assertion: three runs of one firing). Fail-before: today this
  barriers into one firing behind a lint.
- **Save is silent on multi-wire (save):** 3 wires into a transform saves green — no
  bind-payload lint. Fail-before: today it errors.
- **Matched `switch` into a multi-wire port reaches terminal (run, THE blocker):**
  `switch` + two plain wires into one node; the switch matches ⇒ the node fires three times
  (one per upstream incl. the switch) and the run reaches terminal. Fail-before: the matched
  barrier slot sits `Pending` forever and the run hangs.
- **Lineage bindings (run):** linear `trigger → a → b` where `b` binds
  `${steps.trigger.x}` ⇒ resolves (ancestor lineage), under universal `any`. Fail-before:
  binds null via the recorded-map short-circuit.
- **Cross-branch lint (save):** a binding referencing a sibling wire of a multi-wire port ⇒
  **lint error**. Fail-before: saves green, silently binds null per-firing.
- **Multiplicity still propagates (run):** funnel node → downstream transform ⇒ downstream
  settles once per firing (the `fctx` regression pin — proves the removal didn't take the
  seam with it).
- **Explicit `all` still works (run + save):** a test-fixture descriptor declaring
  `join = "all"` barriers once, still demands the binding, and a matched `switch` into it
  still takes the barrier path — the opt-in survives; built-ins audit shows none declares it.
- **Output fan-out (run):** one node wired to 3 downstreams ⇒ all three fire per firing, each
  from its own immutable envelope copy.
- **Duplicate wire (run):** two wires same-output→same-input ⇒ one firing (the named
  divergence, pinned so it's deliberate).
- **Exactly-once + gated-skip (run):** redelivery no-ops per `(node, fctx)`; a `switch`-gated
  wire into a multi-wire port settles `Skipped` and the run reaches terminal — both re-run
  over direct wiring.
- **Frontend (Vitest, real spawned gateway):** palette has no `Links` category; no policy
  glyph on default ports; a 3-into-1 direct flow saves and round-trips its `toPort`s;
  `flowGraph.test.ts` `joinOf` cases updated to `any`-default.

**Test sweep:** every existing test that placed a `link` pair rewires directly; every test
asserting the `all`-barrier *default* (not explicit `all`) flips to per-message expectations.
Known barrier-dependent tests (peer-review verified):
`diamond_frontier_runs_in_dependency_order` (`flows_run_test.rs:206` — the diamond join now
fires twice; rewrite its two-step binding per the lineage rule),
`save_rejects_a_join_with_no_payload_binding` (:534),
`save_still_requires_a_payload_binding_for_a_multi_input_join` (:888),
`all_join_barrier_settles_once_at_empty_fctx` (:979 — needs an explicit-`all` fixture to
survive), the `descriptor.rs:226` unit test, and `flowGraph.test.ts:257-279`. Full touch
list: `flows_run_test`, `flows_nodes_test`, `flows_multi_trigger_test`, `flows_sink_test`,
`binding.rs` tests, `flowGraph.test.ts`, `FlowsCanvas.gateway.test.ts`. (Devkit
`scaffold_test.rs` has **no** link references — earlier inventory error; not in the list.)
No seeded demo/template flow ships a link pair or a multi-wire non-sink node
(peer-review-verified empty set); `docs/skills/` has zero link references.

## Risks & hard problems

- **The `switch` matched-release fix is load-bearing, not optional.** Without it the flip
  ships a run-hang in the headline topology. It reuses the existing `any`-slot mint (same
  deterministic id), but it is a change to the one CAS-claim seam — test matched, gated, and
  explicit-`all` sides together.
- **Lineage resolution is the second load-bearing seam.** The recorded-map short-circuit
  (`run_store.rs:683-692`) is correct for auto-wire but must not be the only lookup path for
  `${steps.*}` — grandparent bindings go through the lineage walk. Prefix-match on the
  `·`-separated `fctx` segments; the all-default-`any` linear chain (every hop extends? no —
  a single-wire port **propagates** the incoming `fctx` without extending; only a ≥2-wire
  release mints) is the case to pin first. If the implementation finds that single-wire `any`
  ports currently mint (extend) per hop, that is the first thing to correct — unbounded
  lineage growth on linear chains is a smell and breaks the byte-identical claim for them.
- **The flip must land at all four default sites together** — `join_of`
  (`descriptor.rs:205`), the `.unwrap_or(All)` fallback (`run_store.rs:307`), the save lint's
  policy read, and the UI mirror (`flowGraph.ts:39`) — or save/run/canvas disagree.
  Mirror-drift is the classic lb trap (cf. `dashboard_save`); `flowGraph.test.ts` pins parity.
- **Barrier-shaped existing tests change behaviour deliberately.** The peer-reviewed list
  above is the sweep; anything else that goes red on the flip gets read, not patched blind.
- **Deleting the resolver cleanly.** `resolve_links` runs at the top of `coordinator::start`
  *and* `drive` on a transient copy; removing it must not disturb the version-pinning order
  (policies pinned at run start with the version — keep that), and the run-load guard slots
  in where it was.

## Open questions

- **None blocking.** Peer review resolved the previously open ones: no seeded demo/template
  flow ships a link pair or an implicit multi-wire barrier (empty set); the wire inspector
  keeps `toPort` (inert, free); the debugging-history entry gets an append-only pointer note.
- **Resolved during implementation (2026-07-12):** single-wire `any` ports **minted** per hop
  (`release_dependents` called `mint` unconditionally on the `Any` leg) — exactly the unbounded-
  lineage smell the Risks section flagged. Fixed first, as directed: `release_one_dependent` counts
  the port's in-subgraph wires (effective-port comparison, primary-resolved) and **propagates** the
  incoming `fctx` on a single-wire port, minting only for ≥2 wires. A linear chain's claim keys are
  byte-identical to the pre-ports engine (pinned by
  `funnel_multiplicity_propagates_one_hop_downstream`).
- **Recorded interpretation (cross-branch lint):** the lint is the scope's literal graph rule —
  `${steps.X}` errors iff X is neither the bound node itself nor a transitive upstream via `needs`.
  A binding to ONE sibling wire of the node's own multi-wire port (X *is* an upstream) is allowed:
  X can be in that firing's lineage (the X-triggered firing), and in a whole-graph run the fctx
  ancestor walk resolves it from the shared root. Node-RED-closest reading; noted in the session.

## Skill doc

**N/A for a new skill** — no new verb, route, or drivable task; two kinds *leave* the
registry. `docs/skills/` has zero link references (verified); `public/flows/flows.md:77-95`
and `docs/STATUS.md:201-253` do reference the pair and must be scrubbed (intent step 8).

## Related

- [`flow-input-ports-scope.md`](flow-input-ports-scope.md) — shipped the machinery this scope
  keeps (`to_port`, `fctx`) and the two nodes + `all` default this scope removes. Its
  link-pair goal is superseded here.
- [`flow-message-envelope-scope.md`](flow-message-envelope-scope.md) — D2 (`inputs` is the
  arriving message) and D4 carry-forward, which make per-message firing well-defined.
- [`flows-scope.md`](flows-scope.md) — Decisions 8 (CAS claim, per `(node, fctx)`),
  9 (one run, no park; retained-input precedence), 14 (edge-gating `switch` — whose matched
  path this scope makes policy-aware), 15 (array-carry — the orthogonal fan-out axis,
  unchanged).
- [`node-descriptor-scope.md`](node-descriptor-scope.md) — the `[[node.input]]` block whose
  *default* flips; the extension convention to declare `join = "all"` explicitly; the named
  mixed-port barrier-counting follow-up.
- [`debug-node-scope.md`](debug-node-scope.md) — `debug` already funnels; unchanged.
- README **§3** (rules 1–9), **§6.1** (API shape — no new verbs).
- Promotes to `doc-site/content/public/flows/flows.md`.
