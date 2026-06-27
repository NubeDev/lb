# Session — a guest drives a full inbox→approval→outbox round-trip through the host callback

- **Scope:** [`scope/extensions/proof-workflow-sim-scope.md`](../../scope/extensions/proof-workflow-sim-scope.md)
- **Builds on:** [`scope/extensions/host-callback-scope.md`](../../scope/extensions/host-callback-scope.md) /
  [`sessions/extensions/host-callback-session.md`](host-callback-session.md) (the read half of the bridge)
- **Public:** [`public/extensions/extensions.md`](../../public/extensions/extensions.md) ("The host-callback ABI")
- **Stage:** S10 (extensions). **Status:** shipped — backend + frontend + live curl green.
- **Date:** 2026-06-27.

## The ask, restated

The host-callback ABI let a guest READ/RESOLVE workflow state (`outbox.status`, `inbox.list`,
`inbox.resolve`), but the inbox/outbox demo was unconvincing: the page only read items something ELSE
seeded. The guest never PRODUCED workflow motion — because the `call_tool` chokepoint had **no write verb**
to create an inbox item or enqueue an outbox effect. A user clicking around saw nothing happen.

This slice closes the gap: it adds the two missing write verbs (`inbox.record`, `outbox.enqueue`) to the
chokepoint, gives the guest a `proof.simulate` tool that DRIVES a full inbox→approval→outbox round-trip
through them, and a "Run workflow simulation" card that makes the produced item + effect VISIBLE on the
page. The "I can finally see it work" payoff.

## What shipped (end to end)

**1. Two write verbs at the chokepoint** (`crates/host/src/tool_call.rs`). `call_workflow_tool` gained
`inbox.record` and `outbox.enqueue` arms; `is_host_native` already matched the `inbox.`/`outbox.`
prefixes. Each runs the same MCP gate (workspace-first, then `mcp:<verb>:call`) as the existing verbs;
deny is opaque. No new store access for the guest — they reuse the REAL write paths:
- `inbox.record {channel,id,body,ts}` → `lb_host::record_inbox` → `lb_inbox::record`. The **author is
  host-forced** to the effective principal's `sub` (= `ext:proof-panel`, the delegated identity) — never
  caller-supplied, exactly like `inbox.resolve`'s actor (`crates/host/src/inbox/record.rs`).
- `outbox.enqueue {id,target,action,payload,ts}` → `lb_host::enqueue_outbox` → `lb_outbox::enqueue`'s
  transactional change+effect write (a minimal justification change row keyed by the effect id, so the
  effect is never orphaned). Staged **Pending** — delivery stays the relay's job
  (`crates/host/src/outbox/enqueue.rs`).

**2. The guest sim tool `proof.simulate`** (`extensions/proof-panel/src/lib.rs`). Built ONLY via
`host.call-tool` (no store/bus handle). It runs a real round-trip: `inbox.record` an item on
`proof-triage` → `inbox.list` it back (proving the host committed it) → `inbox.resolve` Approved →
`outbox.enqueue` an effect → `outbox.status` for the pending count → returns
`{inbox_id, resolved, outbox_pending}`. Each callback authorizes host-side against
`caller ∩ install-grant`; a denied step surfaces as a `Failed`, never a silent skip.

**3. Manifest + dev claims.** `proof.simulate` added to `[[tools]]`, to `[capabilities] request` (so the
`ui_decl::narrow` projection keeps it — the bug that bit `proof.derive`), and to `[ui] scope`; the
inbox/outbox write verbs added to `[capabilities] request` (so the intersected grant permits them) and
`[ui] scope`. Dev claims (`role/gateway/src/session/credentials.rs`) gained `mcp:inbox.record:call`,
`mcp:outbox.enqueue:call` (member-level) and `mcp:proof-panel.proof.simulate:call`.

**4. UI** (`extensions/proof-panel/ui/`). One hook `useSimulate` (calls `proof-panel.proof.simulate`),
one `SimulateSection.tsx`, wired into the thin `Panel.tsx`. On a successful simulate the page bumps a
`workflowKey` counter; `InboxSection` (re-pointed to the `proof-triage` channel) and `OutboxSection` take
a `refreshKey` prop and re-read on the bump — so the user SEES the produced item appear in the inbox and
the effect in the outbox counts. Frozen `app/contract.ts` untouched; FILE-LAYOUT respected (one
hook/verb, ≤400 lines, no utils).

## Open questions — resolved

1. **Author/actor forcing on `inbox.record`** → forced host-side to the principal's `sub`. With the guest
   callback the effective principal's sub is `ext:proof-panel` (the ext acting on the caller's behalf), so
   the recorded author is `ext:proof-panel` — proven by a test assertion (initially I expected
   `user:test` and the test caught the real, more-correct behaviour).
2. **`outbox.enqueue` change row** → `lb_outbox::enqueue`'s transactional change+effect, change keyed by
   the effect id, so the effect is never orphaned. Resolved.
3. **The sim channel** → a dedicated `proof-triage` channel; the InboxSection re-pointed to it so the
   produced item is visible. Resolved.

## Tests (real infra, seeded via the real write path — no mocks, CLAUDE §9)

### Backend — `cargo test -p lb-host --test proof_panel_test` (REAL wasm + store + caps)

```
running 23 tests
test proof_simulate_drives_the_full_workflow_round_trip ... ok
test simulate_inbox_record_denied_when_grant_omits_it ... ok
test simulate_inbox_record_denied_when_caller_lacks_it ... ok
test simulate_outbox_enqueue_denied_when_grant_omits_it ... ok
test simulate_outbox_enqueue_denied_when_caller_lacks_it ... ok
test simulate_is_workspace_isolated ... ok
test inbox_record_and_outbox_enqueue_are_gated_over_the_bridge ... ok
test proof_derive_reads_and_writes_through_the_host_callback ... ok
test callback_denied_when_install_grant_omits_the_verb ... ok
test callback_denied_when_caller_lacks_the_verb ... ok
test callback_is_workspace_isolated ... ok
test re_entrancy_is_bounded_never_hangs ... ok
test identity_does_not_leak_between_calls_on_the_node_global_instance ... ok
test hello_v0_1_guest_still_loads_alongside_a_v0_2_callback_guest ... ok
... (+ the 9 pre-existing proof-panel/host-callback tests)
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Mapped to the mandatory categories (the +7 new tests):
- **capability deny — per direction, per NEW write verb (all four mandatory):**
  `simulate_inbox_record_denied_when_grant_omits_it` (grant omits, caller holds → denied; nothing
  recorded) + `simulate_inbox_record_denied_when_caller_lacks_it` (caller lacks, install requested →
  denied) + the same pair for `outbox.enqueue` (`simulate_outbox_enqueue_denied_when_grant_omits_it`
  asserts nothing staged; `..._when_caller_lacks_it`).
- **workspace isolation:** `simulate_is_workspace_isolated` (ws-B's guest records/enqueues into ws-B
  ONLY; a ws-A reader granted in ws-A sees NONE of it — both inbox and outbox).
- **happy round-trip, asserted via SEPARATE host reads:** `proof_simulate_drives_the_full_workflow_round_trip`
  (the guest's `proof.simulate` runs; the item is read back via a separate `inbox.list` — author
  `ext:proof-panel`, host-forced — and the effect via a separate `outbox.status`, NOT the guest's return).
- **defense-in-depth direct gate:** `inbox_record_and_outbox_enqueue_are_gated_over_the_bridge`
  (each verb deny-without-grant then allow-with-grant, confirmed via a separate read).

**Whole workspace:** `cargo test --workspace` → **398 passed, 0 failed**; `cargo fmt --check` clean.
(One run hit the known `cross_node_routing_test` Zenoh-timing flake — `Elapsed`; passes in isolation
[`cargo test -p lb-host --test cross_node_routing_test` → 3/3] and the clean re-run was 398/0.)

### Frontend — proof-panel unit (`vitest run`, the bridge-interface double, testing §0)

```
 Test Files  2 passed (2)
      Tests  12 passed (12)
```
(+2 new: simulation produces item+effect the refreshed sections SHOW; simulate denied → honest error.)

### Frontend — real spawned gateway (`vitest.gateway.config.ts`, REAL node)

```
 ✓ src/features/ext-host/ProofPanel.gateway.test.tsx (13 tests)
   ✓ workflow-sim: proof.simulate PRODUCES an inbox item + outbox effect the page then reads, live
   ✓ workflow-sim: proof.simulate is denied for an out-of-scope page (deny per verb)
 Test Files  1 passed (1)
      Tests  13 passed (13)
```
Whole shell `pnpm test:gateway` → **85 passed**. The page → guest → host → store → page loop runs over a
real socket: `proof.simulate` produces the motion, then `inbox.list` + `outbox.status` read it back.

### Live node (the "I want to see it" proof)

Fresh in-memory node on :8087, `make publish-ext EXT=proof-panel`, then over the real `POST /mcp/call`:

```
$ proof-panel.proof.simulate {}
  {"inbox_id":"proof-sim-item","resolved":true,"outbox_pending":1}
$ inbox.list {"channel":"proof-triage"}
  {"items":[{"id":"proof-sim-item","channel":"proof-triage","author":"ext:proof-panel",
             "body":"proof.simulate: please approve this simulated request","ts":1}]}
$ outbox.status {}
  {"pending":[{"id":"proof-sim-effect","target":"demo","action":"comment","status":"pending"}]}
```

The wasm guest PRODUCED the inbox item (author host-forced to `ext:proof-panel`) and the pending outbox
effect, both confirmed via SEPARATE host reads — live, over a real socket.

### E2E (Playwright) — extended

`ui/e2e/proof-panel.spec.ts` gained a "Run workflow simulation" step (click → `simulate-result` shows
`proof-sim-item` → `inbox-list` shows the produced item → `outbox-pending` rises → no console/hook
errors, fresh screenshot).

```
✓ 1 [chromium] › proof-panel.spec.ts:32 › proof-panel federated page mounts in the built shell … (1.8s)
  1 passed
```

The full e2e PASSES — unlike the host-callback session (whose e2e was blocked by an in-flight shell
nav-slot refactor), the shell now renders the per-extension "Proof Panel" nav slot, so the built-shell
browser path is proven end to end: a real Chromium logs into a real node, opens the page, clicks "Run
workflow simulation", and sees the produced inbox item + the risen outbox pending count — no console/hook
errors. Screenshot: `ui/e2e/__screenshots__/proof-panel-mounted.png` (the card shows "✓ Inbox item
created: proof-sim-item · ✓ Resolved: Approved · ✓ Outbox effect enqueued · pending now 1").

## Files

- Host: `crates/host/src/inbox/record.rs` (new, `record_inbox`), `crates/host/src/outbox/enqueue.rs`
  (new, `enqueue_outbox`), `crates/host/src/{inbox/mod.rs, outbox/mod.rs, lib.rs}` (exports),
  `crates/host/src/tool_call.rs` (the two dispatch arms).
- Gateway: `role/gateway/src/session/credentials.rs` (+3 caps).
- Ext: `extensions/proof-panel/{src/lib.rs (proof.simulate), extension.toml}`,
  `ui/src/data/useSimulate.ts` (new), `ui/src/pages/SimulateSection.tsx` (new),
  `ui/src/pages/{Panel.tsx, InboxSection.tsx, OutboxSection.tsx}`.
- Tests: `crates/host/tests/proof_panel_test.rs` (+7), `extensions/proof-panel/ui/src/pages/Panel.test.tsx`
  (+2), `ui/src/features/ext-host/ProofPanel.gateway.test.tsx` (+2), `ui/e2e/proof-panel.spec.ts` (+step).
- Docs: this session, `scope/extensions/proof-workflow-sim-scope.md`, `public/extensions/extensions.md`,
  `STATUS.md`.
