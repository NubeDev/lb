# Agent scope — close out the in-house agent (agent-close-out)

Status: scope (the ask). Promotes to `public/agent/agent.md` (new sections) once shipped.

The in-house agent is working — real loop, real `openai_compat` provider, one tool wall
(`call_tool` under `agent ∩ caller`), catalog + active pick, per-workspace model resolution, the
dock riding it. What remains is the honest "Not yet" list at the bottom of
`public/agent/agent.md`. This scope turns that list into a finish line: **four slices** that make
the agent's *accounting honest*, its *cost governable per workspace*, its *progress visible as
motion across nodes*, and its *routed invocation trustworthy* — after which the in-house agent is
**done** (everything else on the old list is deliberately deferred to its owning topic, see
Non-goals).

## Goals

- **A. Real token accounting (`Turn.usage`).** Surface the provider's actual token counts
  (`prompt/completion/total`) on every `Turn` out of the gateway, threaded from
  `openai_compat`'s `response.usage`. Rules' budget meter and any future spend cap consume real
  numbers, not the current content-length estimate. `MockProvider` reports deterministic counts so
  tests stay real-path (rule 9).
- **B. Per-workspace loop policy.** The step ceiling and a per-run token budget become
  workspace-owned config, not the compiled `MAX_STEPS: u32 = 8` constant
  (`crates/host/src/agent/run.rs`). Additive optional fields on the existing `agent.config` record
  (`max_steps`, `max_run_tokens`) — admin-written via the shipped `agent.config.set`, bounded by a
  node-level hard cap (config can lower, never exceed). Budget exhaustion ends the run with an
  honest terminal event ("budget exhausted after N tokens"), never a silent stop. Depends on A for
  the token half (real counts to enforce against); the step half is independent.
- **C. Run progress as bus motion + transcript via outbox.** Run events today reach a watcher
  through the node-local SSE seam (`agent.watch`); an **edge** that routed `agent.invoke` to the
  hub sees nothing until the durable answer. Publish `RunEvent`s to a workspace-scoped Zenoh
  subject (`ws/{ws}/agent/run/{job}` — fire-and-forget motion; the job transcript stays the
  durable truth) so `agent.watch` serves live deltas on whichever node the watcher sits. The
  transcript's **final** persistence (the durable answer + completion) goes through the outbox
  where it must reach another node — must-deliver, not raw pub/sub. Degrades exactly as the dock
  already does: no live deltas → still the durable answer.
- **D. Token-on-the-bus for routed invocations.** The S5 edge→hub `invoke_remote` runs on
  in-process co-trust: the edge authorizes, the hub trusts the routed request's claimed principal.
  Close it: the routed payload carries the caller's **signed token**; the hub verifies signature +
  workspace + caps itself before deriving `agent ∩ caller` — the hub re-checks, never trusts.
  Same posture as the gateway's signed-token seam; no new token format.

Ship order: **A → B**, **C** and **D** independent of both and of each other. Each slice is its
own session; the topic closes when all four are green and `public/agent/agent.md`'s "Not yet"
section shrinks to the deferred items with their owning-topic links.

## Non-goals

- **Provider fallback chains** and the **served OpenAI face** — `scope/ai-gateway/` and
  `scope/external-agent/model-routing-scope.md` #4 own these. The in-house agent is finished
  without them.
- **The curated/bounded tool-menu subset.** ~~Deferred as "a solution without a symptom"~~ —
  the symptom arrived (a confused external agent over the full tool surface). Owned now by
  `../agent-personas/agent-personas-scope.md`, which widens it from a menu-trim into
  user-selectable personas (curated tools + pinned grounding skills + identity). Still a
  non-goal *here*; A's real token counts remain the tuning instrument that topic consumes.
- **The external-agent sandbox / capability wall** — `scope/external-agent/capability-wall-scope.md`.
- **The S6 coding workflow** that composes the agent.
- **Vector/semantic memory recall** — `scope/agent-memory/` v2.

## Intent / approach

The through-line: **the agent's remaining gaps are all "honesty at a seam", not new machinery.**
Every slice threads real data through a seam that already exists, rather than adding a surface:

- A extends the `Turn` struct the gateway already returns — the adapter already *receives*
  `usage` from every OpenAI-compat response and drops it on the floor.
- B extends the `agent.config` record the workspace already writes — `resolve_effective_runtime`
  / `resolve_workspace_model` already read it per run; the loop already takes a `RunContext`.
- C reuses the `RunEvent` encoding both runtimes already emit and the bus the platform already
  moves everything else on. State vs motion is the *point* of the slice: the transcript (state,
  SurrealDB job) is already right; the missing half is the motion.
- D reuses the signed-token verification the gateway already performs — applied at the routed
  queryable (`ws/*/agent/invoke`) instead of trusting co-process claims.

**Rejected: one big "agent v2" rework.** Nothing here changes the loop, the wall, or the
intersection — they're proven. Reworking them to add accounting/policy would risk the shipped
deny/isolation guarantees for zero user-visible gain. Additive fields + one new bus subject.

**Rejected (B): a new `agent_policy` table/verbs.** `agent.config` is already the workspace's
one agent-settings record with admin-gated writes and an invalidation hook; a second record would
split the source of truth and need its own caps for no benefit.

**Rejected (C): SSE-only cross-node relay** (hub SSE → edge re-serve over HTTP). That builds a
bespoke relay for exactly what Zenoh exists to do, and violates state-vs-motion by making the
gateway a message broker.

## How it fits the core

- **Tenancy / isolation:** B's policy lives on the ws-scoped `agent.config`; C's subject is
  `ws/{ws}/agent/run/{job}` and `agent.watch` re-checks the caller's ws before subscribing —
  a ws-B watcher can never receive ws-A deltas (mandatory isolation test, across store + bus);
  D *strengthens* isolation (the hub verifies ws membership itself instead of trusting the edge).
- **Capabilities:** no new caps. A is internal. B writes ride the existing
  `mcp:agent.config.set:call` (admin); reads are internal to the run. C's watch rides the existing
  `mcp:agent.watch:call`; the deny path (no watch cap → no subscription, durable answer still
  arrives) is a mandatory test. D moves cap-checking to the hub side of the routed call — the deny
  is a hub-side refusal with an attributed error, not an edge-side assumption.
- **Placement:** either, all four. B's node hard cap is config; C is exactly the symmetric-nodes
  story (a watcher on any node sees the same feed); no `if cloud` anywhere.
- **MCP surface:** **no new verbs.** A: none. B: two additive optional fields on
  `agent.config.set/get` (existing CRUD pair — API shape: update + get, no list/feed/batch
  needed). C: `agent.watch` (existing live-feed verb) gains cross-node reach — the live-feed
  shape was already chosen; this makes it true everywhere. D: transport hardening under the
  existing `agent.invoke`. Batch: N/A throughout.
- **Data (SurrealDB):** no new tables. B: two nullable fields on `agent.config` (the
  prefs-closed-struct move — additive axes, serde-default, old records read clean). A/C/D: none
  (the transcript already lives on `job:{id}`).
- **Bus (Zenoh):** C publishes `RunEvent`s on `ws/{ws}/agent/run/{job}` — **fire-and-forget**
  motion (a missed delta is recovered from the durable transcript, the dock's existing
  degrade-path); the completion effect that must reach another node goes **must-deliver via the
  outbox**. D rides the existing routed-query namespace, payload now carrying the signed token.
- **Sync / authority:** the job transcript stays the single durable authority (C changes nothing
  about resume/idempotency — deltas are advisory). B's policy is read at run start; an offline
  edit applies on next run (LWW on `agent.config`, the shipped behavior).
- **Secrets:** none new. D carries the signed token over the bus — the same material the gateway
  already transports; never logged, verified-then-dropped on the hub.
- **Stateless / hot-reload:** untouched — no extension holds any of this state.
- **SDK/WIT impact:** **none.** `Turn.usage` is host↔gateway internal; nothing crosses the guest
  ABI. (Flagged per checklist: the `Turn` shape is shared with the ai-gateway role crate — a
  workspace-internal struct, not a plugin boundary.)
- **No mocks (rule 9):** everything proves against the real store/bus/caps/gateway/loop.
  The one sanctioned fake stays `MockProvider` (true-external provider HTTP), extended to report
  deterministic `usage` so A/B are testable for real.
- **One responsibility per file:** A: `usage` lives with `Turn` (`crates/ai/` types) + one edit in
  `providers/openai_compat.rs`. B: `agent/policy.rs` (resolve + clamp), consumed by `run.rs`.
  C: `agent/publish_events.rs` (the bus half) beside the existing watch; outbox target wiring in
  the existing outbox seam. D: verification in the routed-invoke handler file, not spread.
- **Skill doc:** `skills/agent/SKILL.md` exists and must be updated by the implementing sessions
  (B adds the policy fields to the config how-to; C documents cross-node watch). No new SKILL.md.

## Example flow

The finished picture, one run:

1. An admin sets `agent.config { max_steps: 6, max_run_tokens: 20_000 }` in Settings → Agent
   (B; clamped by the node hard cap; `mcp:agent.config.set:call` gated).
2. A member on an **edge** node posts to their dock channel; `agent.invoke` routes to the hub
   carrying the member's **signed token** (D). The hub verifies signature + ws + caps, derives
   `agent ∩ caller`, and starts the loop with the workspace policy in its `RunContext` (B).
3. Each model turn returns `Turn { …, usage: { prompt, completion, total } }` from the real
   provider (A). The loop accumulates usage against `max_run_tokens` and steps against
   `max_steps`.
4. Every `RunEvent` is persisted to the job (unchanged) **and** published on
   `ws/{ws}/agent/run/{job}` (C). The member's dock — on the edge — receives live deltas through
   `agent.watch` subscribing on its own node; Working→Answering states animate cross-node.
5. The run finishes (or exhausts budget → honest terminal event). Completion + the durable answer
   flow through the outbox to the channel (must-deliver). The dock lands on Done; the transcript
   on `job:{id}` is the authority; the SSE feed and the bus deltas were only ever motion.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`), per slice, all against the real
store/bus/caps/gateway/loop (`MockProvider` the only fake, now usage-reporting):

- **Capability-deny (§2.1):** B — a non-admin `agent.config.set { max_steps }` is denied, nothing
  persists. C — a watcher without `mcp:agent.watch:call` gets no subscription and no deltas, yet
  the durable answer arrives (the dock degrade-path, asserted host-side). D — a routed invoke
  whose token lacks `mcp:agent.invoke:call` is refused **by the hub**; a forged/expired token is
  refused before any derivation.
- **Workspace-isolation (§2.2):** C — ws-B subscribing `ws/A/agent/run/*` receives nothing
  (bus + verb both walled); B — ws-B's policy never applies to a ws-A run. D — a valid ws-B token
  cannot invoke against ws-A (the hub's own check, no edge trust).
- **Offline/sync (§2.3):** C — a watcher that missed deltas (late subscribe) still converges via
  the durable transcript; a resumed run does not re-publish already-persisted steps' effects
  (idempotent completion through the outbox — no double-deliver). B — a policy edit mid-run does
  not change the running run (read-at-start), applies to the next.
- **Unit/integration keys:** A — `openai_compat` maps a real `usage` body onto `Turn.usage`;
  absent `usage` (a lax server) → `None`, never a fabricated count; rules' budget meter prefers
  real usage and falls back to the estimate, stated. B — clamp logic (workspace may lower, never
  exceed the node cap); token-budget exhaustion ends with the honest terminal event and the job
  records why; `max_steps` still bounded by the compiled absolute ceiling. C — event ordering
  per job subject; fire-and-forget loss tolerated (assert convergence, not delivery). D — the
  happy routed path is byte-equivalent to today's for a valid token (no behavior change for
  legitimate callers).
- **Hot-reload:** N/A — no extension state.

## Risks & hard problems

- **C's dual-write discipline.** Persist-then-publish must not let the bus outrun the store
  (a watcher acting on a delta whose step isn't durable yet). Keep deltas advisory-only (the
  existing dock contract) and publish *after* the append lands; assert order in the test.
- **B's budget semantics mid-turn.** A turn's cost is known only after the provider answers —
  the budget check is therefore *before* starting a turn (may end ≤ one turn over budget). State
  that contract in the doc and the terminal event; do not pretend to intra-turn precision.
- **D's compatibility window.** The hub must reject tokenless routed invokes *after* every edge
  sends tokens — a flag-day on a self-hosted fleet. Ship hub-verifies-when-present first, then
  flip required; the scope's exit is required-mode green.
- **A's provider variance.** `usage` shapes drift across OpenAI-compat servers (some omit it,
  some stream it only in the final chunk). `Option<Usage>` end-to-end; never fabricate.
- **Bus fan-out cost (C)** on chatty runs is bounded per job and ws-scoped — note it, don't
  engineer for it yet.

## Open questions

1. **C — delta payload:** publish full `RunEvent`s or thin `{job, seq}` notify + read-back?
   Proposal: full events (they're small, the dock consumes them as-is; read-back doubles store
   load for nothing). Decide against a measured chatty run.
2. **B — the node hard caps:** proposal `max_steps ≤ 32`, `max_run_tokens ≤ 200_000`, both node
   config with these defaults; workspace values clamp. Confirm the numbers at implementation.
3. **D — rollout switch:** is the verify-when-present → required flip a node config flag or a
   release note ("all nodes ≥ vX")? Proposal: config flag, default required after one release.
4. **A — rules meter migration:** does the rules budget UI switch to real usage immediately, or
   dual-display (real when present, estimate labelled otherwise)? Proposal: real-when-present,
   labelled estimate fallback — never an unlabelled guess.

## Related

- `public/agent/agent.md` — "Not yet (follow-ups)": the list this scope closes; the deferred
  remainder links to its owning topics.
- Siblings: `agent-scope.md` (the loop + intersection), `default-agent-wiring-scope.md` (the
  wall + boot), `active-agent-wiring-scope.md` (the provider adapter + per-workspace model),
  `agent-catalog-scope.md`, `agent-catalog-test-and-secrets-scope.md`.
- `scope/ai-gateway/ai-gateway-scope.md` (`Turn`, `Provider`, fallback chains — deferred),
  `scope/jobs/jobs-scope.md` (the durable transcript), `scope/inbox-outbox/` (must-deliver
  completion), `scope/external-agent/model-routing-scope.md` (the served OpenAI face — deferred).
- `skills/agent/SKILL.md` — updated by the implementing sessions (B, C).
- README `§6.16` (shared AI agents), `§6.14`/`§6.15` (AI gateway), `§6.9`/`§6.10` (jobs +
  durability), `§6.4` (bus), `§7` (tenancy).
