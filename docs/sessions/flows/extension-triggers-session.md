# Flows — extension nodes + triggers/lifecycle (slices 3 + 4)

- Area: flows
- Status: shipped (green) — backend spine (Waves 1–2) complete.
- Scope: [`scope/flows/extension-nodes-scope.md`](../../scope/flows/extension-nodes-scope.md) (slice 3),
  [`scope/flows/triggers-lifecycle-scope.md`](../../scope/flows/triggers-lifecycle-scope.md) (slice 4).
- Spine Decisions: 2, 9, 10, 13 (slice 3/4).
- Session: this file. Prev: [slice 1](node-descriptor-session.md) · [slice 2](flow-run-session.md).

## Slice 3 — extensions contribute backend nodes

The user's key ask: an extension contributes a backend node type, **identically for WASM and native**
(the descriptor is byte-for-byte tier-agnostic; only the execution transport differs — the symmetric-
nodes rule applied to node execution). The run engine dispatches an extension node's bound
`<ext>.<tool>` through the **one** `call_tool` chokepoint, where the shipped `build_call_context`
derives `effective = caller ∩ install.grant` — **two-direction deny, no widening** (extension-nodes-
scope). No new WIT world; arm/disarm are ordinary declared `[[tools]]`.

### What shipped
- `execute_node.rs` — descriptor-aware ext-node dispatch: resolves the exact `<ext>.<tool>` binding
  from the merged registry (slice 1), then `call_tool` under the caller's principal. The
  `caller ∩ install-grant` narrowing is enforced by the shipped `build_call_context` (the same seam
  the page bridge + guest-callback use) — an ext node cannot widen beyond either the caller or the
  install grant.
- `source.rs` — the **source** shape (Decision 2): `source_series` (host-owned `flow:{ws}:{flow}:{node}`
  — ws-scoping + uniqueness host-owned, never extension-chosen), `arm_source` (allocate the series +
  call the ext's `arm` tool), `disarm_source` (call `disarm` — no leaked live socket on disable).
  Stateless flow; the socket is motion owned by the supervised extension (rule 4).
- `extensions/mqtt/extension.toml` — the worked reference extension manifest: `mqtt.in` (source) +
  `mqtt.out` (sink) `[[node]]` blocks, the bound `[[tools]]` (subscribe/publish/arm/disarm), the
  `net:*` + `lb-secrets` request caps. The proof of the `[[node]]` contract.
- `Install.nodes` propagated at install (wasm + native paths) — slice 1; exercised here.

### Mandatory categories claimed
- **Workspace-isolation** — the source series + the registry are ws-scoped; ws-B's registry excludes
  ws-A's mqtt nodes; ws-B's `source_series` is a distinct (absent) name.
- **Both-direction install-grant narrowing** — rides the shipped `build_call_context` (`effective =
  caller ∩ install.grant`), already proven by the ext-lifecycle / hot-reload suites (the chokepoint
  every ext call goes through). The mqtt native **sidecar binary** itself is a deferred mechanical
  piece (the manifest + node contract + host arm/disarm ship here); building the OS process is the
  follow-up, named honestly rather than faked.

`host flows_ext_test` **5**: mqtt manifest `[[node]]` parse+validate · registry surfaces mqtt nodes
after a real install · **source series ws-scoped** · arm allocates the series + records the armed
marker / disarm clears it (Decision 13 no-leak) · **workspace isolation** (mqtt nodes absent in ws-B).

## Slice 4 — triggers & lifecycle

What starts a flow, and where. The five trigger kinds (`manual|cron|event|inject|boot`), the
enable/disable + start-on-boot switch, the **two lifecycle passes in two files** (a `react_to_flows_cron`
clock-scan + a `reconcile_flows` state-convergence loop), and the **placement** (eligible set matched
as data against role — never an `if cloud` branch, rule 1).

### What shipped (all one-verb-per-file, FILE-LAYOUT)
- `Flow` lifecycle fields (additive serde defaults): `enabled` / `start_on_boot` / `placement`
  (`Either|CloudOnly|LocalOnly`) / `cron: Option<String>` / `next_attempt_ts`. A flow written before
  these deserialises as enabled / not-start-on-boot / either / no cron.
- `triggers.rs` — `flows.enable {id, enabled, start_on_boot}` (durable intent) + `flows.inject {id,
  node, value}` (Decision 9: upsert `flow_input`; fire a run ONLY for a `fire`-mode inject-trigger —
  `retain` updates state and starts nothing; the control-loop pattern).
- `react_cron.rs` — `react_to_flows_cron` (modelled on `react_to_reminders`): deterministic run id
  `(flow_id, scheduled_ts)` → idempotent (a re-scan no-ops); fire-once-then-skip-to-next-future-slot
  (no backfill storm); `next_after` on the injected clock (never wall-clock). The `manual` kind IS
  `flows.run`; `event`/`boot` are the reconciler's.
- `reconcile.rs` — `reconcile_flows`: re-reads the flow dir, elects a single owner (Decision 10 —
  `placement_matches(placement, role)` as data), arms enabled+placement-matching flows' sources, fires
  `boot` once for `start_on_boot` flows, **disarms** the rest (Decision 13 — no leaked socket).
- Wired into `mod.rs` dispatch (`flows.enable`, `flows.inject`) + `lib.rs` re-exports.

### Mandatory categories claimed
- **Capability-deny** — `flows.enable` refused without `mcp:flows.enable:call`.
- **Workspace-isolation** — a ws-B `react_to_flows_cron` pass never fires a ws-A flow (the directory
  is ws-scoped).
- **Offline/sync** — the cron reactor is at-least-once; the deterministic `(flow_id, scheduled_ts)`
  run id + the CAS claim make a reconnect re-scan exactly-once (no double-fire).
- **Placement as data** — `placement_matches` matched against `Role` (CloudOnly→Hub/Solo,
  LocalOnly→Edge/Solo, Either→any), no role branch in core.

`host flows_triggers_test` **8**: enable flips durable flags · inject-**retain** (state set, no run) ·
inject-**fire** (one run) · cron **fire-once-then-skip** · cron **idempotent re-scan** · cron
**workspace-isolation** · **capability-deny** enable · placement-matched-as-data (no `if cloud`) ·
reconciler **disarms a disabled flow** (no leaked socket).

## Honest deferrals (with the decision each traces to)
- **The mqtt native sidecar binary** (the OS process that owns the broker socket) is deferred — the
  manifest `[[node]]` contract + the host arm/disarm + series bridge ship now; the binary itself is
  the shipped native-tier pattern (`echo-sidecar`) generalised, a mechanical follow-up. The
  descriptor/transport parity claim holds (the host picks transport from the install record, no
  engine branch). Traces to extension-nodes-scope "Tier parity".
- **Cross-node failover** (re-electing an owner when the home node dies) is a `node-roles` deferral
  (triggers-lifecycle-scope explicit non-goal). v1 elects the local node for placement-matching flows.
  Traces to Decision 10.
- **`flows.watch` SSE** (the canvas's preferred live source) is a named follow-up; the canvas falls
  back to a bounded `flows.runs.get` poll. Traces to flow-run-scope "Live feed".
- **Subflow "park"** is realised as an inline child drive (slice 2 note) — a reactor-driven park is a
  follow-up. Traces to Decision 11.

```
cargo test -p lb-flows                                  → 26 green
cargo test -p lb-ext-loader                             → 16 green
cargo test -p lb-host --test flows_nodes_test           → 5
cargo test -p lb-host --test flows_run_test             → 12
cargo test -p lb-host --test flows_ext_test             → 5
cargo test -p lb-host --test flows_triggers_test        → 8
cargo build --workspace / cargo fmt --check             → green / clean
```
