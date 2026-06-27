# extensions scope — a guest drives a full inbox→approval→outbox round-trip through the host callback

Status: **in progress** (2026-06-27). Builds directly on the shipped host-callback ABI
([`scope/extensions/host-callback-scope.md`](host-callback-scope.md) /
[`sessions/extensions/host-callback-session.md`](../../sessions/extensions/host-callback-session.md)).
Session: [`sessions/extensions/proof-workflow-sim-session.md`](../../sessions/extensions/proof-workflow-sim-session.md).

## The gap this closes

The host-callback slice proved a guest can do real platform work — `proof.derive` reads a series and
writes a derived one through `host.call-tool`. But its **inbox/outbox** demonstration is unconvincing:
the proof-panel page only *reads* `outbox.status` and *lists/resolves* inbox items that something **else**
seeded (a test `/_seed` route). The **guest never PRODUCES** that workflow motion. A user clicking around
sees nothing happen and cannot tell the inbox/outbox actually works end to end.

Worse, the host callback today only exposes the **read/resolve** half of the workflow surface
(`outbox.status`, `inbox.list`, `inbox.resolve`). There is **no write verb** to CREATE an inbox item or
ENQUEUE an outbox effect over the bridge — so a guest *cannot* produce workflow motion even if it wanted
to. The chokepoint is one-directional.

## Goals

- Add the **two missing write verbs** to the `call_tool` chokepoint + `is_host_native`, each
  capability-gated exactly like the existing workflow verbs (workspace-first, then `mcp:<verb>:call`),
  reusing the **real** `lb_inbox::record` / `lb_outbox::enqueue` write paths — no new store access for the
  guest, host-mediated only:
  - `inbox.record` — create an inbox item `{channel, author, body, ts} -> {ok}`. Author is **host-forced**
    to the principal's `sub` (never caller-spoofable), like `inbox.resolve`'s actor.
  - `outbox.enqueue` — stage an effect `{id, target, action, payload, ts} -> {ok}`.
- Add a guest sim tool **`proof.simulate`** to the proof-panel wasm guest, built ONLY via
  `host.call-tool` (no store/bus handle). It runs a real workflow round-trip:
  1. `inbox.record` an item on a `proof-triage` channel,
  2. `inbox.list` it back to get its id,
  3. `inbox.resolve` that id Approved,
  4. `outbox.enqueue` an effect keyed off the approval,
  5. `outbox.status` to read pending/delivered counts,
  returning `{inbox_id, resolved, outbox_pending}` so the page can show each step landed.
- A **"Run workflow simulation"** card on the proof-panel page (one hook `useSimulate`, one
  `SimulateSection.tsx`, wired into the thin `Panel.tsx`) with honest loading/error/empty states. After a
  click, the existing `InboxSection` / `OutboxSection` **refresh** so the user SEES the item appear in the
  `proof-triage` inbox and the effect appear in the outbox counts — the "I can finally see it work" payoff.

## Non-goals

- **No new store/bus handle in the guest** (rules 4/5). The guest touches the platform ONLY through the
  mediated callback, exactly as `proof.derive` does.
- **No new MCP tool surface beyond the two write verbs.** `inbox.record` / `outbox.enqueue` are the raw
  durable-workflow verbs the host already owns (`lb_inbox::record`, `lb_outbox::enqueue`); this exposes
  them over the **existing** bridge chokepoint, gated identically. No CRUD/list/watch beyond what exists.
- **No relay/delivery.** `outbox.enqueue` stages a *pending* effect (the durable backstop); actually
  delivering it is the relay's job, unchanged. The sim asserts the effect is *pending*, not *delivered*.
- **No watch/motion from a guest** (still deferred from the host-callback scope) — request/response only.

## How it fits the core

- **Capabilities:** each new verb runs the full `authorize_tool` gate against `caller ∩ install-grant`.
  Deny is opaque. Mandatory deny-tests, **per direction**, for EACH new write verb: (i) the guest calls it
  but the **install grant omits** it → Denied even though the caller holds it; (ii) the guest calls it but
  the **caller lacks** it → Denied even though the install requested it.
- **Tenancy / isolation:** the `ws` is the host-set one from the caller's token, never guest-supplied.
  `proof.simulate` in ws-B records into ws-B only; a ws-A reader sees none of it (tested).
- **Data (SurrealDB):** none added. `inbox.record` writes through `lb_inbox::record`; `outbox.enqueue`
  through `lb_outbox::enqueue` (its transactional change+effect write — the change row is the
  approval-justification, so the effect is never orphaned). The one datastore, one mediated path.
- **Bus / Sync / Secrets:** unchanged. The guest never receives a token; the author/actor is host-forced.
- **MCP surface:** **two** new host-native verbs over the existing chokepoint, gated like the others.
  The guest sim tool `proof.simulate` is an `<ext>.<tool>` like `proof.derive` — requested in the manifest
  `[capabilities]` AND added to the `[ui]` scope (the `ui_decl::narrow` projection DROPS a UI verb unless
  `mcp:<verb>:call` is in the grant — the bug that bit `proof.derive`).
- **Placement / symmetric nodes:** either. No `if cloud`.

## Example flow

The page calls `proof-panel.proof.simulate {}` → host derives `caller ∩ install-grant`, sets the call
context, invokes the guest. Inside the guest's `tool.call("proof.simulate", …)`:

1. `host.call-tool("inbox.record", {channel:"proof-triage", body:…, ts:N})` → host gates
   `mcp:inbox.record:call`, forces author = principal sub, writes via `lb_inbox::record` → `{ok}`.
2. `host.call-tool("inbox.list", {channel:"proof-triage"})` → reads it back, the guest takes the newest id.
3. `host.call-tool("inbox.resolve", {item_id, decision:"approved", ts:N})` → records the approval.
4. `host.call-tool("outbox.enqueue", {id, target:"demo", action:"comment", payload:…, ts:N})` → enqueues
   a pending effect (change row = the approval justification).
5. `host.call-tool("outbox.status", {})` → reads the pending count.
6. Returns `{inbox_id, resolved:true, outbox_pending:N}`.

**Deny path:** install with a grant omitting `outbox.enqueue` → step 4 is Denied at the host even though
the caller holds it (the intersection narrowed it); the guest surfaces it as a failure.

## Testing plan

All through the **real** `lb-runtime` component + real store + real caps (no mocks, CLAUDE §9), extending
`crates/host/tests/proof_panel_test.rs`:

- **Capability deny — per direction, per NEW write verb** (`inbox.record`, `outbox.enqueue`): (i) grant
  omits it, caller holds it → Denied; (ii) caller lacks it, install requested it → Denied.
- **Workspace isolation:** `proof.simulate` in ws-B records into ws-B only; a ws-A reader (granted) sees
  none of it.
- **Happy round-trip:** `proof.simulate` creates an inbox item, resolves it Approved, enqueues an outbox
  effect; assert each via **separate** host reads (`inbox.list` / `outbox.status`), NOT the guest's return.
- **Frontend:** extend `ui/src/features/ext-host/ProofPanel.gateway.test.tsx` (REAL spawned gateway) — call
  `proof-panel.proof.simulate` over the page bridge, then assert `inbox.list` shows the item and
  `outbox.status` shows the effect. Plus proof-panel unit tests (the bridge double).
- **E2E:** extend `ui/e2e/proof-panel.spec.ts` — click "Run workflow simulation", assert the inbox item +
  outbox count render (or document the known nav-slot block from the host-callback session and rely on the
  real-gateway Vitest as the e2e-equivalent).

## Open questions

1. **Author/actor forcing on `inbox.record`** → forced to the principal's `sub` host-side (like
   `inbox.resolve`'s actor); the guest's requested author is advisory and ignored. **Resolved that way.**
2. **`outbox.enqueue` change row** → reuse `lb_outbox::enqueue`'s transactional change+effect: the change
   table/id is the approval justification (`proof_sim_approval` / the inbox item id), so the effect is
   never orphaned from the resolution it followed. **Resolved.**
3. **The sim channel** → a dedicated `proof-triage` channel (distinct from the existing `triage` the other
   section reads) so the simulation's items are self-contained and the existing InboxSection's `triage`
   view is unaffected unless re-pointed. **Resolved:** simulate uses `proof-triage`; the InboxSection is
   re-pointed to `proof-triage` so the user sees the produced item appear there.

## Related

- [`scope/extensions/host-callback-scope.md`](host-callback-scope.md) — the ABI this extends (the read
  half); its chokepoint `crates/host/src/tool_call.rs` is where the write verbs land.
- [`scope/inbox-outbox/…`] — the durable workflow primitives (`lb_inbox::record`, `lb_outbox::enqueue`).
- README `§6.10` (inbox/outbox), `§6.5` (MCP as the contract), `§3` rules 4/5/7.
