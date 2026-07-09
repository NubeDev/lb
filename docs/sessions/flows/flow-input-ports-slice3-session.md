# Flows — port-labelled edges + per-input-port join policy, Slice 3 (the `link` pair) (session)

- Date: 2026-07-09
- Scope: ../../scope/flows/flow-input-ports-scope.md
- Stage: S8+ (flows shipped; this slice completes the `any`-funnel topology end to end)
- Status: done (Slice 3 of 4)

## Goal

Ship the **`link` built-in pair** (`link-out {target}` / `link-in {name}`) — Node-RED's wireless link
nodes, the canonical `any`-policy collector for "many sources → one handler, fire per message". The
pair is the load-bearing topology the scope's **propagate-one-hop-past-the-funnel** fail-before needs:
a non-sink `any` node (`link-in`) feeding a downstream transform `W` must settle **W once per link-in
firing**, each `W@<fctx>` reading its OWN firing's message — proving the firing context (`fctx`) from
Slice 2 propagates past the funnel (a naive `#{upstream}` depth-1 suffix settles `W` once). Plus the
per-firing capability-deny + the per-firing outbox-dedup + exactly-once-on-redelivery one hop past
the funnel — all riding the same topology.

## The load-bearing decision: run-load resolution, not save-time mutation

The scope's "Intent" step 5 worded the resolver as **save-time** ("a save-time resolver in
`save.rs`…rewrites into a `needs` edge"). On reading the code I rejected that and moved resolution to
**run-load**, with only **validation** at save. Recorded here because it's the deviation from the
scope's literal wording, and the user asked for the long-term-best call.

A save-time mutation of the persisted `flow` has two real bugs the scope wording didn't account for:

1. **Non-idempotent under re-save.** The editor loads a flow already carrying the resolver's added
   wires, then resolves again. Idempotent `needs` checks avoid duplicates, but…
2. **Stale wires on delete.** …the real bug: when an author deletes a `link-out`, the resolver-added
   wire onto `link-in` is now orphaned. The resolver can't tell an author's wire from one it added —
   so the stale wire persists, silently `link-in` firing on a phantom source. And the editor renders
   the resolver-added wires as physical wires, leaking the "wireless" promise into the canvas.

Run-load resolution sidesteps both cleanly: the persisted `flow` record is the **author's intent**
(link-out/link-in verbatim, no resolved wires), so the editor round-trips the wireless sugar and a
delete can never leave a stale wire. The engine runs on a **transient resolved copy** produced by
`Flow::resolve_links` at the top of `coordinator::start`/`drive` — deterministic, so the seed (start)
and the drive see the same graph. Save-time only `validate_links`s the topology (clear mistakes caught
before any run). The rejected alternative is recorded in `link.rs`'s module doc.

## What changed (Slice 3)

### The link built-ins (`lb-flows/src/builtins/link.rs`, new)
- **`link-out`** — a `sink`-kind naming node (one `payload` in, no out), config `{target}`. `sink`
  kind ⇒ its input defaults to `any` (so a multi-source link-out saves green), and **nothing may wire
  from it** (validate_links rejects a node that lists a link-out in `needs` — its only output is the
  wireless name, not a data port).
- **`link-in`** — a transform with one `any` primary input (declared via the `input_ports` table) +
  one `payload` output, config `{name}`. It fires **once per resolved upstream** (the Node-RED OR
  funnel); the `fctx` seam propagates the multiplicity downstream.
- Both ship under a new **`Links`** palette category. `builtins/mod.rs`'s `EXPECTED`/count bumped to
  35 (33 + the link pair); the `flows_nodes_test` `BUILTINS` const updated in lockstep.

### The resolver + validator (`lb-flows/src/link.rs`, new — pure graph math, no I/O)
- **`resolve_links(flow) -> Flow`** — produces a NEW flow where each `link-out {target:T}`'s `needs`
  are appended onto the matching `link-in {name:T}`'s `needs` (idempotent, de-duplicated; lands on
  link-in's primary port), and every `link-out` is **dropped** from `flow.nodes`. Its job is done:
  the link-in carries the wires, the `any`-funnel runtime + `fctx` propagate the multiplicity. A
  diamond (two link-outs sharing one upstream) de-dupes to one edge (no double-fire — the funnel
  settles per distinct upstream, not per link-out).
- **`validate_links(flow) -> Result<(), DagError>`** — catches: a `link-out` whose target names no
  `link-in` (`LinkOutMissingTarget`); a `link-out` with no `config.target` (`LinkOutNoTarget`); a
  node that wires from a `link-out` (`WiresFromLinkOut` — that wire vanishes when the sender is
  dropped); a `link-in` with no sources at all (`LinkInDead`); and two `link-in`s sharing one name
  (`LinkNameCollision` — the resolver would funnel both, silently duplicating). Five new `DagError`
  variants on the public enum.
- **Same-workspace wall (rule 6) is structural:** link names resolve only within one flow (one ws), so
  a ws-B `link-out` can never name a ws-A `link-in` — no special handling.

### Run-load wiring (`lb-host/src/flows/coordinator.rs`)
- `start` and `drive` each call `resolve_links(flow)` at the top and thread the resolved flow through
  (`create_run`, `policy_map`, `subgraph`, `execute_one`, `release_dependents`, `finalize_if_complete`).
  Deterministic ⇒ seed and drive agree. The pinned `flow.version` is untouched (the copy shares it);
  `flows_resume`'s drift check is unaffected. Every run path routes through start/drive, so the
  reactors (cron/interval/source) + `run_flow_to_completion` (subflow) all resolve identically.

### Save wiring (`lb-host/src/flows/save.rs`)
- `flows_save` calls `validate_links` right after `validate_flow` (before `validate_node_configs`) —
  a bad link topology is a clear save error, not a silent run-time no-op.

### The `link-in` dispatch + per-firing outbox dedup (`execute_node/mod.rs`, `execute_node/sink.rs`)
- `dispatch` gained a `"link-in"` leg: a pass-through (`NodeOutcome::ok({payload: inputs.payload})`).
  At run load the coordinator already rewire link-in's `any` port; the `fctx`-scoped auto-wire placed
  the triggering upstream's envelope in `inputs`, so link-in just emits it.
- **The outbox/inbox dedup tripwire the scope named** — "thread `fctx` through the outbox key wherever
  the node key is used today." `dispatch`'s `_fctx` became `fctx` and is passed to `dispatch_sink`;
  the sink's `effect_id` (outbox) and `id` (inbox) are now `format!("{run_id}:{node_id}{}", slot_suffix(fctx))`
  — `""` in the all-`all` case (byte-for-byte today's key), `@{fctx}` for a sink inside an `any`-funnel's
  reach. N firings ⇒ N distinct idempotent deliveries, not one swallowing the rest. (`series` is
  already per-firing via `seq = now`, unchanged.)

## Tests (real store/caps/gateway, rule 9) — all green

**`lb-flows` unit (`cargo test -p lb-flows --lib`): 108 passed** (+12 over Slice 2's 96): 3 link
descriptor tests (`link_out_is_a_naming_sink…`, `link_in_is_an_any_funnel_transform`,
`link_configs_compile`) + 9 link resolver/validator tests (resolve-rewrites-and-drops, resolve-
idempotent-on-physical-wires, resolve-dedupes-a-diamond, and the five validate rejections + the two
accepts).

**`lb-host` flows integration: 39 in `flows_run_test` (+7) + the full 13-binary suite green.** New
Slice-3 cases in `flows_run_test`:
- **`link_funnel_propagates_one_hop_past_the_funnel` — THE headline:** `link-in` (`any`, fed by 3
  `link-out`s each from a distinct source) → transform `W` (one wire) ⇒ **`W` settles THREE times**,
  each `W@li#a`/`@li#b`/`@li#c` reading its OWN firing's payload (1/2/3). The link-out senders are
  absent from the run snapshot (dropped at run load). A naive depth-1 `#{upstream}` scheme settles
  `W` ONCE — this proves the `fctx` propagates past the funnel.
- **`link_funnel_denies_per_firing_at_a_downstream_tool`** — an `any` funnel feeding a `tool` node
  whose verb the caller lacks ⇒ **2 err settles**, not one (per-firing deny; `FailurePolicy::Continue`).
- **`link_funnel_outbox_dedups_per_firing`** — an `any` funnel feeding a must-deliver sink ⇒ **2
  distinct outbox effects** (`lb_outbox::pending` read directly off the store), each effect id
  carrying the `@{fctx}` suffix.
- **`link_funnel_exactly_once_per_firing_on_redelivery`** — re-running the SAME `run_id` no-ops every
  `(node, fctx)` slot (CAS claim); still exactly two `W` settles.
- **`save_rejects_a_link_out_naming_a_missing_link_in`** + **`save_rejects_a_wire_from_a_link_out`** —
  the save-time link lints.
- **`workspace_isolation_link_funnel_step_keys`** — a ws-B caller cannot read a ws-A link-funnel run's
  per-firing `@{fctx}` slots (the mandatory isolation category for the new key shape).

`cargo build --workspace` + `cargo fmt --check` clean. `flows_nodes_test` `BUILTINS` const updated
for the new pair (5/5 green). The rest of the flows integration suite is unchanged and green
(flows_sink 4, flows_debug 7, flows_data_nodes 11, flows_data_engine 15, flows_multi_trigger 7,
flows_runtime_control 12, rules_workflow_convergence 14, …).

## Definition of done (Slice 3)

- [x] The `link` pair built end to end (builtins + run-load resolution + save validation + dispatch);
      the headline propagate-one-hop-past-the-funnel test ships and is green.
- [x] Per-firing cap-deny + per-firing outbox-dedup + exactly-once-on-redelivery one hop past the
      funnel — all on the link topology.
- [x] FILE-LAYOUT held (`builtins/link.rs` + `link.rs` are the named-concept files; host edits focused).
- [x] No `if cloud`; no core knowledge of any extension; no mock data / no fake backend.
- [x] The mandatory capability-deny (per firing) + workspace-isolation categories asserted for the
      link pair's new key shapes.
- [x] Session doc + STATUS + public doc + scope OQ all updated.

## Open questions resolved by this slice (recorded in the scope)

- **Should `link-in` allow an `all` policy?** — **Resolved: v1 `link-in` is `any`-only.** A `link-in`
  IS the canonical funnel; the descriptor declares `join: "any"` on `payload`. A join-over-links
  caller has not appeared; if one does, a deliberate `link-in`-with-`all` (or a second node kind) is
  the primitive, not a per-node toggle.
- **`flows.patch_run` and port policy** — **Resolved: out of `patch_run`'s scope.** A policy is
  descriptor-level (the `input_ports` table); `patch_run` is config-only (Decision 1/12). A policy
  change is structural ⇒ a new flow version, never a live-run patch. No validator needed — the shapes
  don't overlap.
- **Collect-join (`all` over an `any` funnel) — defined or forbidden?** — **Resolved by what the
  runtime actually does, which overturned the scope's "hard-error" proposal.** An `all` port whose
  ONLY upstream is a funnel **inherits the multiplicity** (fires once per funnel firing, each
  carrying one envelope) — coherent via the `fctx` propagation, and exactly the headline test's
  topology (`W` is an `all` transform downstream of `link-in`). Hard-erroring "an `all` port reaching
  through an `any` funnel" would **forbid the load-bearing propagate-past-the-funnel seam itself**.
  The genuine footgun is narrower: an `all` port **joining a funnel-carrying upstream with a
  different-fctx upstream** (the two settle under divergent `fctx`s ⇒ the barrier slot never
  completes). That needs a full `fctx`-lineage reachability analysis a save-time heuristic can't
  soundly approximate; it is left as a **named follow-up** (Slice 5: collect-join detection), not a
  silent gap. A future true "collect-into-array" join (barrier over a funnel's COMPLETE firing set)
  is a deliberate primitive if a caller appears.

## What this completes

With Slice 3, the `any`-funnel topology is expressible end to end: a flow author wires many sources
into a `link-in` (or directly into any `any` port), and the engine fires the downstream once per
upstream — multiplicity propagating past the funnel via `fctx`, exactly-once per firing, per-firing
deny + per-firing durable delivery all holding. Only the canvas paint (Slice 4) remains.
