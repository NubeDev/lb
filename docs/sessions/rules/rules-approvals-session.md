# Rules — the approval loop (`inbox.request_approval` + the release reactor) — session

- Date: 2026-07-04
- Scope: ../../scope/rules/rules-approvals-scope.md
- Stage: S8 — data plane (rules on the real workspace). See STATUS.md.
- Status: DONE — the rule verb, the held-effect outbox status, the generic release reactor, all
  mandatory test categories, and the Playground gateway loop are green.

## Goal
Let a rule body **request an approval explicitly** — raise a `needs:approval` item that stages a
**gated effect** — and have a generic reactor **fire that effect only when the item is `Approved`**
(discard on reject, leave held on defer). "A rule proposes, a human disposes," reusing the exact
`Item` + `Resolution` + outbox trio the coding workflow already ships — no new approval primitive.

## The shape built (load-bearing choice)
The gated effect is a normal outbox effect staged in a new **`held`** status (state vs motion, §3):
the relay skips `held`, so an un-approved effect is never delivered; approval flips it `held → pending`
(the existing relay then delivers it), rejection flips it `held → discarded`. Staging held-and-visible
means the reviewer sees exactly what approving will do. The reactor is a **domain-free sibling** of the
coding-workflow's `resolve_approval` (scope Open-questions option b) — it keys only on
`(resolution, held-effect-id)`, deriving the effect id from the item id, so it treats the effect as
opaque data (rule 10) and leaves the coding path untouched.

## What changed

### Outbox (`crates/outbox`)
- `src/model.rs` — added `EffectStatus::Held` and `EffectStatus::Discarded` (kebab-case → `"held"` /
  `"discarded"`) + an `Effect::held()` builder that stages a fresh effect held instead of pending.
- `src/pending.rs` — the relay's schedulable scan (`pending`/`due`) is `["pending","failed"]` only, so
  `held`/`discarded` are skipped **for free** (a held effect is never delivered). Added a `held()` scan
  for the read-only/reactor view; updated the module doc to name the two new non-schedulable statuses.
- `src/release.rs` (NEW) — `release` (`held → pending`) and `discard` (`held → discarded`), both
  **guarded on the current status being `Held`** (a replay/late-reject is a no-op → released exactly
  once). Returns whether it transitioned. Absent effect → `false` (not an error).
- `src/lib.rs` — export `held`, `release`, `discard`.

### Inbox (`crates/inbox`)
- `src/rejected.rs` (NEW) + export — the `Rejected`-resolution scan, sibling of the existing
  `approved()`, that the reactor reads to discard a gated effect.

### Host (`crates/host`)
- `src/outbox/enqueue_held.rs` (NEW) + exports — `enqueue_held_outbox`: identical authority +
  transactional path to `enqueue_outbox` (gated by `mcp:outbox.enqueue:call`), but stages the effect
  `Held`. No new cap: staging is not the gated step; the *release* is.
- `src/outbox/status.rs` — `OutboxStatus` gains a `held: Vec<Effect>` bucket (`#[serde(default)]` —
  additive, older clients still decode).
- `src/approval_reactor/` (NEW module — `id.rs` / `react.rs` / `spawn.rs`): `held_effect_id(item_id)`
  = `held:{item_id}` (the one derivation the rule verb and reactor agree on);
  `react_to_approval_releases(node, ws)` = the pass (release approved, discard rejected, tally);
  `spawn_approval_reactors(node, workspaces, period)` = the detached boot tick (twin of the flow/agent
  reactors). Runs under a host/system authority; the release is gated by the resolution existing, NOT a
  user cap.
- `src/tool_call.rs` — new MCP dispatch arm `outbox.enqueue_held` (takes the `item_id`, derives the
  effect id host-side); aliased in the outer gate to check `mcp:outbox.enqueue:call` (no new cap).
- `src/lib.rs` — module decl + re-exports (`held_effect_id`, `react_to_approval_releases`,
  `spawn_approval_reactors`, `ApprovalReleasePass`, `enqueue_held_outbox`).

### Rules engine (`crates/rules`)
- `src/verbs/inbox.rs` — the **`inbox.request_approval(#{ id, channel, body, route, on_approve })`**
  rhai verb. Two writes in the partial-failure-safe order: stage the held effect FIRST
  (`outbox.enqueue_held`), then record the `needs:approval route:… body` item — so an item never
  dangles without its effect. Both charge the shared write meter; both route through the seam under
  `caller ∩ grant`. Returns the item id (for a follow-on `inbox.resolve`).

### Node / test gateway
- `node/src/main.rs` — spawns `spawn_approval_reactors` beside the flow/agent reactors (2s tick).
- `role/gateway/src/bin/test_gateway.rs` — spawns the reactor for the fixed `rules-approvals` ws (1s
  tick) so the UI gateway test can drive the *full* approve→released loop against the real reactor.

### UI
- `ui/src/lib/outbox/outbox.types.ts` — `Effect.status` gains `"held" | "discarded"`; `OutboxStatus`
  gains an optional `held?: Effect[]`.
- `ui/src/features/rules/examples/examples.ts` — a `propose-and-approve` worked example.

## Tests (real infra, no mocks — rule 9)
- `crates/outbox/tests/outbox_test.rs` (+3, **9/9**): a held effect is never schedulable until released;
  release/discard are guarded-on-held + idempotent (replay + reject-after-approve are no-ops); releasing
  an absent effect is a harmless no-op.
- `crates/host/tests/approval_release_test.rs` (NEW, **8/8**) — the full loop through real
  store/host/reactor/relay: gated release delivers **exactly once**; reject → discarded, never
  delivered; defer → still held; idempotent replay; cap-deny (`outbox.enqueue_held` refused without the
  outbox cap, no write); no user verb can force a release; **ws-isolation** (a ws-B approval never
  releases a ws-A effect); durable re-scan after "restart".
- `crates/rules/tests/approvals_test.rs` (NEW, **4/4**) — the verb stages effect-first then records the
  tagged item; charges TWO writes (a cap of 1 trips on the item); a deny on the effect stage is opaque
  with **no partial write** (no dangling item); returns the item id.
- `ui/src/features/rules/RulesApprovals.gateway.test.tsx` (NEW, **4/4**, real spawned gateway) —
  request_approval raises the item + stages the effect HELD; approving releases it to `pending` **via
  the real reactor tick**; caller-gated deny (no outbox cap → no write); the catalog example runs green.
- Regression: `crates/host` workflow suite (reactor/workflow/isolation) green; `RulesMessaging.gateway`
  green; UI typecheck clean on the changed files.

## Decisions confirmed (from the scope's Open questions)
- **Body-tag convention kept for v1** (`needs:approval route:… body`) — no `Item` schema change, reuses
  the exact parse the coding path uses. Typed facet noted as the clean follow-up.
- **A sibling `approval_release` reactor** (option b), not a generalization of `resolve_approval` — keeps
  the coding path untouched and the new path domain-free.
- **Compound-write order: effect (held) first, then item** — a mid-verb fault leaves at most a harmless
  held effect; the deny test asserts no partial write.
- **`defer` is inert in v1** (left held; re-resolve later). **`route` is advisory** (the cap is the
  gate; `route` is a UI/reactor routing hint).

## Follow-ups (out of scope, noted)
- Typed `ItemFacet`/`meta` if a second consumer of the tag appears.
- Enforced reviewer routing (a policy scope) + multi-reviewer quorums / escalation timers.
- A GC pass for a dangling held effect (item write failed mid-verb) — today visible via `outbox.status`.
