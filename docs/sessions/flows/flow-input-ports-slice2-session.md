# Flows — port-labelled edges + per-input-port join policy, Slice 2 (the `any` runtime + firing context) (session)

- Date: 2026-07-09
- Scope: ../../scope/flows/flow-input-ports-scope.md
- Stage: S8+ (flows shipped; this is the load-bearing runtime seam for the Node-RED multi-input model)
- Status: done (Slice 2 of 4)

## Goal

Ship the **`any`-funnel runtime** — Node-RED's "fire-per-message" OR — over the Slice-1 data model,
via the load-bearing **firing-context (`fctx`) propagation seam**. Three wires into one `debug` node
must print **three** times, in one durable run, exactly-once per firing; and a node one hop past a
funnel must settle once **per funnel firing**, each reading its own message. All-`all` flows must stay
**byte-for-byte today's** behaviour (the empty-`fctx` common case ⇒ today's claim key + resolution).

This is the slice the scope exists for: "the firing context is the actual load-bearing seam;
discovering it mid-implementation would force exactly the patch this doc exists to avoid."

## The load-bearing design (the firing context)

The naive fix — suffix an `any` node's step key by its immediate upstream (`{node}#{upstream}`) —
disambiguates **only at the funnel** and breaks one hop downstream (a node with one wire from the
funnel has one slot, settles once; `${steps.funnel.payload}` is ambiguous). The correct primitive is a
**per-message identity carried down the run** in an additive envelope field `fctx`:

- **Minted at each `any` slot**, deterministic per `(node, upstream, parent fctx)` (`rust/crates/flows/
  src/firing_context.rs::mint`). Nested funnels extend it: `link-in#mqtt-a` → `link-in#mqtt-a·funnel2#w`.
- **Every claim key + `${steps.*}` resolution is scoped by `(node, fctx)`** — `{run}:{node}` when
  `fctx` is empty (the all-`all` case ⇒ byte-for-byte today's key), `{run}:{node}@{fctx}` otherwise.
- **Still one run, no fan-out storm.** Multiplicity is statically bounded by the wire topology (path
  count), never by event volume (`split` stays array-carry, Decision 15). Run-terminal counts slots
  `(node, fctx)`, not nodes.

## What changed (Slice 2)

### The firing-context module (`lb-flows/src/firing_context.rs`, new)
- `mint(node, upstream, parent_fctx)`, `slot_suffix(fctx)` (`""` ⇒ no suffix), `triggering_upstream_of`,
  the `FCTX_FIELD = "fctx"` envelope constant, the `·`/`#` separators. Unit-tested: empty-fctx byte-
  identical, mint-extends-parent (nested funnels), deterministic-per-firing, slot-suffix round-trip.

### The record + claim-key seam (`lb-host/src/flows/record.rs`, `run_store.rs`)
- `FlowStepRecord` gains `fctx`, `triggered_by`, `parent_fctx` (all serde-defaulted). `step_record_id`
  takes `fctx` ⇒ `{run}:{node}` empty / `{run}:{node}@{fctx}` non-empty.
- `claim_step`/`record_outcome`/`park_step`/`read_step`/`write_step` all take `fctx`. `record_outcome`
  stamps `fctx` into the recorded envelope so a downstream binding resolves the matching settle.
- **Frontier-only seeding.** `create_run` seeds only indegree-0 nodes as `(node, "")` Enqueued.
  Non-frontier slots are minted dynamically by `release_dependents` as upstreams settle — for all-`all`
  this is byte-identical (every slot at `fctx=""`, key `{run}:{node}`).

### Per-policy release (`run_store.rs::release_dependents` + `touch_barrier_slot` + `mint_firing`)
- On a `(node, fctx)` settle, each in-subgraph dependent releases by its port's policy:
  - **`all`**: touch `(dep, fctx)` — create Pending with the in-subgraph barrier indegree if absent,
    decrement (this upstream settled under `fctx`), Enqueue at 0. Today's path.
  - **`any`**: mint `(dep, mint(fctx, dep, finished))` Enqueued, recording `triggered_by` +
    `parent_fctx`. Idempotent (deterministic `fctx` ⇒ redelivery re-mints the same slot, no-op).
- `ready_one_dependent`/`skip_gated`/`skip_subtree` take `fctx`. **`skip_gated` now CREATES a `Skipped`
  slot when the dependent was never seeded** — the bug that hung `switch_fires_only_the_matched_port`
  post-rewrite (a `switch`-gated dependent had no record, so finalize waited forever).

### Binding resolution (`run_store.rs::resolve_node_bindings`)
- **`any`-firing** (`triggered_by` set): auto-wire the single triggering upstream (settled under
  `parent_fctx`), carrying its non-`payload` fields forward (unambiguous — one message per firing).
- **barrier/frontier**: resolve `${steps.X}` against settles carrying the SAME `fctx` (empty in all-
  `all` ⇒ every settle, byte-for-byte today's resolution).

### Coordinator + execute (`coordinator.rs`, `execute_node/mod.rs`)
- `drive` builds the per-node **policy map** (node_id → descriptor, read once + pinned with the run's
  flow version) + the **subgraph** set, passes them through.
- `ready_slots` scans the run's step records for `Enqueued` `(node, fctx)` pairs (the frontier is now
  per-slot, not per-node).
- `execute_one(node, fctx, policies, subgraph)`: claims the slot, reads `triggered_by`/`parent_fctx`,
  resolves bindings (any-firing auto-wire vs barrier), dispatches, records outcome under `fctx`,
  releases dependents per-policy (or `switch::release_matched` under `fctx`).
- `patch_run` writes the patched config onto `(node, "")`; a non-empty-`fctx` firing falls back to it
  (a patch is config-level, not per-firing).

### Per-kind default + policy-aware lint (`descriptor.rs`, `save.rs`)
- `NodeDescriptor::join_of` applies the per-kind default: **`any` for `sink`-kind ports** (Node-RED's
  debug/funnel — three wires ⇒ three firings), `all` for everything else. An explicit `input_ports`
  entry always wins.
- The join lint moved off the pure model (`validate_flow`) into `save.rs` (registry-aware): an `all`
  port with ≥2 wires must bind `payload` (data-drop bug); an `any` port with N wires is **valid** (the
  funnel). `DagError::UnboundJoin` retained on the public enum (dead-coded, registry-aware lint replaces it).

### `runs.get` legibility (`runs.rs`)
- Each `(node, fctx)` slot surfaces as its own step; a non-empty `fctx` is labelled (the `fctx` id +
  `triggeredBy`) so the debug story stays readable past the funnel — "why did my sink write three
  times?" is answerable. Empty `fctx` ⇒ today's shape (no `fctx` field shown).

## Tests (real store/caps/gateway, rule 9) — all green

**`lb-flows` unit (`cargo test -p lb-flows --lib`): 96 passed** (+5 `firing_context`: empty-byte-
identical, mint-appends-segment, mint-extends-parent-for-nested, deterministic-per-firing,
slot-suffix-round-trips; +1 descriptor `join_defaults_all_for_transforms_any_for_sinks`).

**`lb-host` flows integration: 127 passed across 13 binaries** — the engine rewrite held the whole
existing surface (the all-`all` paths, switch edge-gating, multi-trigger, sinks, runtime-control,
data-nodes, data-engine, triggers, debug, flipflop, rhai-template, plc-reliability). New Slice-2
cases in `flows_run_test` (+3 → 32 total):
- `any_funnel_fires_once_per_upstream` — **THE headline**: 3 wires into `debug` ⇒ **3 settles**, each
  under a distinct `fctx` (`dbg#a`/`dbg#b`/`dbg#c`), each carrying its **own** upstream's payload
  (1/2/3). Fail-before: the old engine settled `debug` once.
- `all_join_barrier_settles_once_at_empty_fctx` — the byte-identical guard: a 2-upstream `all` join
  settles once at `fctx=""`, no `@fctx` suffix.
- `workspace_isolation_any_funnel_step_keys` — a ws-B caller cannot read a ws-A funnel's per-firing
  `@{fctx}` slots (the mandatory isolation category for the new key shape).

**`cargo fmt` clean; `cargo build --workspace` clean.**

## Debug entry

[`debugging/flows/multi-input-node-fires-once-not-per-message.md`](../../debugging/flows/multi-input-node-fires-once-not-per-message.md)
— the node-id-edge / single-claim root cause and the port-labelled-edge + per-port-`join` + propagated
`fctx` fix. README row added.

## Definition of done (Slice 2)

- [x] The `any` runtime + `fctx` seam built end to end; the headline Node-RED OR funnel ships.
- [x] All-`all` byte-for-byte unchanged (empty `fctx` ⇒ today's claim key + resolution), pinned by a
      dedicated test + the whole existing flows suite (127) staying green.
- [x] FILE-LAYOUT held (`firing_context.rs` is the named-concept file; `run_store` edits focused).
- [x] No `if cloud`; symmetric nodes unaffected (pure engine change).
- [x] No core knowledge of any extension; no mock data / no fake backend.
- [x] The mandatory workspace-isolation category re-asserted for the new `@{fctx}` key shape.
- [x] Session doc + debug entry + STATUS + public doc + scope OQ all updated.

## What's deferred to Slice 3 (named, not silent)

- **Propagate-one-hop-past-the-funnel end-to-end test** (the scope's THE-seam fail-before for a naive
  depth-1 suffix) — needs a non-sink `any` node so the funnel has a downstream. Slice 2's only `any`
  nodes are terminal sinks (`debug`), so the topology is not yet expressible; the `fctx` machinery
  that handles propagation (mint extending parent, same-`fctx` resolution) is built + unit-tested, and
  the test lands with `link-in` (Slice 3): `link-in` (any, 3 wires) → transform `W` must settle
  **three** times, each reading its own firing's message. Per-firing cap-deny + outbox-dedup-per-firing
  ride the same topology.
- **Slice 3 = the `link` built-in pair** (`link-out{target}` / `link-in{name}`) — virtual edges
  resolved at save into `any`-port wires.
- **Slice 4 = the canvas** — per-named-port handles, the `any`/`all` glyph, the wire inspector.

## Open questions carried (Slice 3 decision points)

- **`any`-port firing-order determinism** — Slice 2's funnel settles in upstream-completion order
  (arrival-ordered, like Node-RED). Confirmed acceptable; a node needing order uses `all` + sort.
- **Mixed-policy multi-port nodes** — the model is open (a `left` (`all`) + `control` (`any`) node is
  expressible) but untested; Slice 2 ships no mixed built-in. `link-in` (Slice 3) is `any`-only.
- **Collect-join (`all` over an `any` funnel)** — still v1-hard-errored by the per-port join lint in
  the common case; defining it ("barrier over a funnel's complete firing set, receive the array") is a
  deliberate tested primitive if a caller appears.
