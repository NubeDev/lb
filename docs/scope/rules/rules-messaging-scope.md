# Rules scope — `messaging` verbs: a rule reaches the inbox, outbox, and channels

Status: scope (the ask). Promotes to `public/rules/rules.md` once shipped.

Today a rule only touches the messaging planes **implicitly**: an `alert(#{…})` finding is routed
*after* the run by the host, host-authorized, into the inbox + outbox. We want a rule body to drive
the **inbox, outbox, and channels explicitly and fully** — read them, write them, resolve/mark/edit
them — from inside the sandbox, but **only with the caller's own authority**: every action runs the
same `caps::check` a direct MCP call would, so a rule can do nothing on these planes its invoker
couldn't, and a missing grant throws an opaque error mid-run. This is the messaging counterpart to the
`source(...)`/`ai.*` seams the engine already has: one narrow host-implemented seam, three rhai handles
(`inbox`, `outbox`, `channel`), every call gated at the real chokepoint.

> Read with: `rules-engine-scope.md` (the engine + the existing `DataSeam`/`AiSeam` this mirrors and the
> `alert` → inbox/outbox routing this generalizes), `rules-ai-wiring-scope.md` (the sibling seam wiring),
> `../inbox-outbox/` (the durable inbox/outbox planes), `../channels/` and README §3.5 (the channel bus
> gate), `../auth-caps/auth-caps-scope.md` (`caps::check`, the chokepoint every verb runs),
> README `§6.5` (MCP — the contract), `§6.10` (inbox/outbox), `§3` (rules 3/5/6/7/10).

---

## Goals

- A **`inbox` rhai handle**: `inbox.list(channel)`, `inbox.record(#{channel, id, body})`,
  `inbox.resolve(item_id, decision)` — the full attention-item surface a rule needs to raise, read, and
  resolve items.
- An **`outbox` rhai handle**: `outbox.enqueue(#{id, target, action, payload})` and `outbox.status(id)`
  — stage and inspect must-deliver effects. The relay-driver verbs (`due`/`mark_delivered`/`mark_failed`)
  are deliberately **not** on the rule handle (a rule stages effects; it does not drain them — see
  Resolved decisions).
- A **`channel` rhai handle**: `channel.post(cid, #{…})`, `channel.history(cid, n)`,
  `channel.edit(cid, mid, #{…})`, `channel.delete(cid, mid)`, `channel.list()` — post to, read, and
  amend a channel.
- **Every action gated by the caller's caps.** The verbs route through the one MCP contract
  (`crate::call_tool`), so the host's `caps::check` runs under `caller ∩ grant` on each call; a deny is
  **opaque** (an error the rule can catch but not distinguish from "empty") and never a partial write.
- **A per-run write budget** (a `WriteMeter`, sibling to the `AiMeter`): reads are free, motion-producing
  writes are charged and capped, so a rhai loop can't enqueue ten thousand outbox effects (DoS bound).
- **Deterministic, idempotent writes** — ids derive from the injected logical `now` + a per-run counter
  (no wall-clock, no random in core), so a re-run upserts rather than duplicating.

## Non-goals

- **A new authorization path.** The verbs add *zero* new gate; they reuse the existing `inbox.*`/
  `outbox.*` MCP verbs and the channel `bus:chan/{cid}:{action}` cap verbatim. If a rule can't do it via
  a direct MCP call, it can't do it via a verb.
- **A live subscription / streaming feed inside a rule.** A single `rules.run` is bounded and returns its
  result; `channel.history(cid, n)` gives a bounded snapshot, not a `watch`. Continuous reaction to
  channel motion is a **flow** (`../flows/`), not a rule verb.
- **Long/batch fan-out in a rule handler.** A rule that must post to N channels or drain a large outbox
  is a **chain/flow** (a job), per `rules-engine-scope.md`'s "a single run is synchronous and bounded".
  The write budget enforces the boundary; it does not become an async job runner.
- **Bypassing the `alert` convenience path.** `alert(#{…})` stays — it's the ergonomic "raise attention +
  must-deliver" one-liner. These verbs are the *explicit, full-CRUD* surface for when a rule needs more
  than fire-and-alert (resolve an item, mark an effect delivered, edit a channel message).
- **New channel semantics.** We expose the *existing* channel post/history/edit/delete; we do not add
  threading, reactions, or a new message kind (those are channel scopes).

## Intent / approach

**One `MessagingSeam`, three handles, routed through the one MCP contract — this is the load-bearing
choice.** The engine already reaches the outside world only through host-implemented seams (`DataSeam`,
`AiSeam` in `rules/src/seam.rs`), each closed over the caller's principal + pinned workspace, each
re-running `caps::check` on every call. We add exactly one more: `MessagingSeam::call(tool, input) ->
Result<Json, SeamError>`, whose host impl is a thin `handle.block_on(crate::call_tool(&node, principal,
ws, tool, &input))`. The rhai `inbox`/`outbox`/`channel` handles are pushed into the scope like the `ai`
handle; each method builds the tool-id + JSON and calls the seam. Because dispatch goes through
`call_tool`, the caps check, the workspace pin, and the opaque `ToolError::Denied` all come **for free**
— the rule reaches these planes exactly the way the UI and the AI agent do (rule 7), and the tool id is
opaque data the seam never branches on (rule 10).

**The gate is the point.** "Assuming the user has access, else throw" is not new code — it is
`call_tool` running `caps::check` under `caller ∩ grant`. A rule whose `outbox.enqueue` needs a cap the
*invoker* lacks is denied mid-run, opaquely, before any write. This is the same property the data seam
already gives for `source(...)`: a rule can touch no plane a direct call in the same workspace couldn't.

**Channels need an MCP contract first (the one real gap).** `inbox.*` and `outbox.*` are already MCP
verbs in `tool_call.rs`. Channels are **not** — `post`/`history`/`edit`/`delete` are host functions gated
by their own `channel/authorize.rs` chokepoint (`bus:chan/{cid}:{action}`), driven by the gateway/WS. To
let a rule reach channels *through the same gated seam as everything else* (rather than the seam
special-casing channel host fns — a rule-7/rule-10 leak), we **add thin `channel.post`/`channel.history`/
`channel.edit`/`channel.delete` MCP verbs** to the `tool_call.rs` dispatcher, each a wrapper over the
existing host fn behind its existing cap. This closes a pre-existing gap (channels had no MCP contract)
and the UI/agent get the verbs too — the right fix regardless of rules. `channel.list` and
`channel.chart_pref.*` already route through `call_tool`; we bring the write/read surface alongside them.

**Rejected — call the host fns directly from the seam.** The seam *could* call `record_inbox`/
`enqueue_outbox`/`channel::post` directly (as `route_alerts` does today for `alert`). Those fns *do*
run the caller's `authorize_tool` internally, so this would not leak authority — but it's still the
wrong seam. Routing through `call_tool` gives **one contract** (the rule reaches these planes exactly as
the UI/agent do — rule 7), one **uniform gate** (gate-1 workspace isolation + the single opaque
`ToolError::Denied`, not three plane-specific error types the seam must re-map), and every cross-cutting
behavior wired at that chokepoint (undo-capture, history) for free. Channels have **no** verb-wrapped
direct fn at all, so they need the MCP verb regardless; routing all three through `call_tool` keeps the
seam a single, branch-free bridge (rule 10). `route_alerts` stays on the direct fns only because it is a
*post-run, engine-owned* convenience effect; the in-body verbs are the caller acting explicitly.

**Rejected — a single flat `message.*` namespace.** Considered collapsing all three planes into one
handle. Rejected: inbox (attention/state), outbox (must-deliver motion), and channels (live bus motion)
are *different planes with different caps and different durability contracts* (§3 rule 3 — state vs
motion). Three handles keep the author's mental model honest and map one-to-one onto the existing cap
grammar; one file per handle (FILE-LAYOUT).

## How it fits the core

- **Tenancy / isolation:** every verb call goes through `call_tool`, which pins the workspace from the
  caller's token (never script-set) and runs gate-1 workspace isolation before any capability is read. A
  ws-B rule cannot `inbox.list` a ws-A channel, `outbox.status` a ws-A effect, or `channel.post` to a
  ws-A channel — the pin refuses it before the cap check. Proven across store + MCP (mandatory isolation
  test).
- **Capabilities:** the verbs are the gate, reusing existing caps — `mcp:inbox.list:call`,
  `mcp:inbox.record:call`, `mcp:inbox.resolve:call`, `mcp:outbox.enqueue:call`, `mcp:outbox.status:call`,
  `mcp:outbox.due:call`, `mcp:outbox.mark_delivered:call`, `mcp:outbox.mark_failed:call`, and for
  channels the `bus:chan/{cid}:{Pub|Sub}` cap the new `channel.*` MCP verbs enforce. Inside a run each
  call re-checks under `caller ∩ grant` — a rule cannot widen beyond what its invoker holds. **The deny
  is opaque** (`ToolError::Denied` → a rhai error with no plane/cap detail). Every grant has a deny test.
- **Placement:** `either` (symmetric). The seam is a pure in-process bridge to `call_tool`; it runs
  wherever the node runs. No `if cloud`. (A rule that posts to a channel needs the bus available on that
  node — a property of the channel plane, not the seam.)
- **MCP surface (§6.1 — judged, not defaulted):**
  - **Consumed (the rule reaches these):** all pre-existing `inbox.*`/`outbox.*` verbs, plus the **new**
    `channel.post`/`channel.history`/`channel.edit`/`channel.delete` verbs this scope adds to the
    dispatcher. No new `rules.*` verb — the surface is *inside* `rules.run`, reached by the rhai handles.
  - **CRUD:** covered by the consumed verbs — inbox record/resolve, outbox enqueue, channel
    post/edit/delete. Each is its own MCP tool + cap (one responsibility per file, FILE-LAYOUT) — the new
    `channel.*` verbs follow suit (`channel/post.rs` etc. already exist; we add the dispatch arms). The
    outbox relay-ops verbs (`due`/`mark_delivered`/`mark_failed`) exist for drivers but are **not** on
    the rule handle (Resolved decisions).
  - **Get / list:** `inbox.list`, `outbox.status`, `channel.history`/`channel.list` —
    workspace-scoped reads, uncharged by the write meter.
  - **Live feed:** N/A inside a rule (a run is bounded; `channel.history` is a snapshot). Continuous
    reaction is a flow.
  - **Batch:** N/A here — a bulk post/drain is a chain/flow (a job). The **write meter** enforces the
    per-run bound so no one grows a blocking N-item loop in a rule body.
- **Data (SurrealDB):** no new tables. Writes land in the existing inbox/outbox/channel records via the
  existing verbs. Ids are deterministic (`now` + per-run counter) so a re-run upserts.
- **Bus (Zenoh):** `channel.post` produces **motion** on the channel subject via the existing `post`
  path (state vs motion held — §3 rule 3). `outbox.enqueue` stages **must-deliver** effects through the
  durable outbox, never raw pub/sub (§3 durability). `inbox.*` is state, not motion.
- **Sync / authority:** the writes are authoritative on the hosting node like any record/effect; the run
  itself is node-local and stateless (the cage holds no durable state — rule 4). A saved rule that uses
  these verbs survives a restart (it's a record); the effects it staged are drained by the existing
  outbox relay.
- **Secrets:** none. The seam carries no key or DSN — it hands an opaque tool-id + JSON to `call_tool`.
- **SDK/WIT impact:** none. `MessagingSeam` is an internal Rust trait + rhai handles; the new `channel.*`
  MCP verbs are host dispatch arms over existing host fns. No wasm/native ABI change.

## Example flow

A facilities analyst writes a rule that escalates a cooler breach and closes the loop.

1. The rule body runs inside `rules.run`, workspace pinned to `acme`, the caller's principal carried into
   every seam:
   ```
   let hot = source("cooler.temp").last("1h").col("value").max();
   if hot > 5.0 {
       // raise an attention item on the ops channel (caps: mcp:inbox.record:call)
       inbox.record(#{ channel: "ops", id: `cooler-${hot}`, body: `cooler at ${hot}C` });
       // stage a must-deliver page (caps: mcp:outbox.enqueue:call)
       outbox.enqueue(#{ id: `cooler-page-${hot}`, target: "notify", action: "page",
                         payload: #{ level: "critical", series: "cooler.temp" } });
       // post to the live ops channel (caps: bus:chan/ops:Pub)
       channel.post("ops", #{ kind: "text", body: `⚠ cooler breach ${hot}C` });
   }
   ```
2. Each verb builds its tool-id + JSON and calls `MessagingSeam::call`; the host `block_on`s
   `call_tool`, which runs the workspace pin + `caps::check` under `caller ∩ grant`, then dispatches to
   the existing `inbox.record`/`outbox.enqueue`/`channel.post` handler. The **write meter** charges each
   of the three writes; `channel.history` reads later would be free.
3. Ids are `cooler-${hot}` etc. (content-derived) — a re-run with the same reading upserts, no duplicate.
4. **Deny path:** the analyst's token lacks `bus:chan/ops:Pub`. `channel.post("ops", …)` returns an
   opaque error mid-run (the inbox + outbox writes already committed; the run surfaces the error and the
   analyst learns they can't post to `ops`, but not why beyond "denied"). A `for` loop calling
   `outbox.enqueue` past the per-run write cap aborts with a budget error.
5. A later resolve rule reads and closes the item: `inbox.resolve(item_id, #{ action: "ack" })` — gated
   by `mcp:inbox.resolve:call`, idempotent on `item_id`.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`) — the gate. **No mocks**: real store, real bus,
real caps, real MCP host; data is **seeded as real records** and read/written through the real verbs.
No sanctioned fake needed here (unlike `ai.*`, these planes all run in-process).

- **Capability-deny (§2.1):** each verb denied without its cap — `inbox.record`/`resolve`,
  `outbox.enqueue`, `channel.post`/`edit`/`delete` — mid-run, opaquely, with **no partial
  write** before the deny. A rule holding `inbox.list` but not `inbox.record` can read but not write. The
  new `channel.*` MCP verbs each get a deny test at the dispatcher level too.
- **Workspace-isolation (§2.2):** a ws-B rule cannot `inbox.list`/`resolve` a ws-A item, `outbox.status`
  a ws-A effect, or `channel.post`/`history` a ws-A channel — refused by the workspace pin before the cap
  check, across store + MCP. `channel.list` from a ws-B rule never lists a ws-A channel.
- **The write meter (DoS bound):** a rhai loop calling `outbox.enqueue`/`channel.post` past the per-run
  cap aborts with a budget error; reads (`inbox.list`, `channel.history`, `outbox.status`) are uncharged
  and never trip it. A tight-limit unit test proves the boundary.
- **Idempotency / determinism:** a rule run twice with the same inputs upserts (no duplicate inbox item
  or outbox effect) — ids are `now` + counter, no wall-clock/random in the result (testing §3).
- **The rule verb is caller-gated (regression):** a rule whose caller lacks `outbox.enqueue` (but whose
  data verbs succeed) is denied at the enqueue — the verb's authority is the caller's, never widened.
- **`channel.post` rejects worker kinds (regression):** a rule posting a `kind:"agent"`/`kind:"query"`
  item is rejected by the handle with an author error (a rule cannot spawn a run — Resolved decisions);
  a plain `kind:"text"` post succeeds.
- **Offline / sync (§2.3):** a saved rule using these verbs survives a node restart (it's a record); the
  outbox effects it staged are drained by the existing relay after restart.
- **Integration (real gateway/UI):** a Playground `*.gateway.test.tsx` runs a rule end-to-end against a
  real spawned node — seed real series, run a rule that records an inbox item + posts to a channel, then
  assert the inbox item and the channel history over the real gateway.

## Risks & hard problems

- **Partial-write on mid-run deny.** A rule that does three writes then hits a denied fourth leaves the
  first three committed (a rule is not a transaction). This is *correct* (each verb is its own gated MCP
  call, like the UI making three calls) but must be **documented loudly** in the skill/public doc so
  authors expect it — and the deny test asserts no partial write *within a single verb*, not across the
  body. A future `dry_run`/transactional wrapper is out of scope.
- **Write-meter calibration.** Too low and legitimate rules (raise 5 alerts) break; too high and the DoS
  bound is theatre. The decided default is 32 writes/run (Resolved decisions); the build should confirm it
  against the real Playground rules and expose the `env::rules::MAX_WRITES` knob so a workspace can tune it
  before the per-workspace override lands.
- **The channel MCP verbs are new surface.** Adding `channel.post`/`edit`/`delete`/`history` to the
  dispatcher exposes channel writes to *every* MCP caller (agent, UI, other extensions), not just rules.
  That's intended, but each must enforce the **exact same** `channel/authorize.rs` gate the WS path does —
  a divergence would be a cap leak. The wrappers must be thin and gate-identical; test the deny at the
  dispatcher, not only through a rule.
- **Ordering vs. logical clock.** `channel.post` motion and inbox/outbox writes share the injected `now`;
  interleaving many in one run needs the per-run counter to keep ids/ordering stable across re-runs.

## Resolved decisions

No open questions — these are the long-term answers the build follows.

- **Write meter → one shared per-run budget, reads uncharged.** A single `env::rules::MAX_WRITES` budget
  (default **32**) is charged by every motion-producing write across all three planes; reads
  (`inbox.list`, `outbox.status`, `channel.history`/`list`) are free. One meter, not per-plane: it mirrors
  the `AiMeter` the author already reasons about, and a single "writes per run" number is the honest DoS
  bound — per-plane sub-budgets are surface without a real caller. A **per-workspace override record** is
  additive later (exactly as the AI budget plans), not v1.
- **A rule `channel.post` cannot spawn a run — worker kinds are rejected at the handle.** The generic
  `channel.post` **MCP verb** keeps full parity with the WS path (a `kind:"query"`/`kind:"agent"` item
  triggers the inline query/agent worker — rule 7, no special-casing). The **rule handle** is stricter: it
  validates the item and rejects `kind:"agent"`/`kind:"query"` with an author error ("a rule cannot spawn
  a run — use a flow"). Long-term this is the only safe answer: a bounded, synchronous rule quietly kicking
  an unbounded background agent run breaks the "long work is a flow/job" boundary (and invites recursion —
  a rule posting an agent item whose run posts another). The restriction lives in the rule layer, so the
  generic verb stays uniform and only the *rule* is fenced.
- **The rule `outbox` handle is `enqueue` + `status` only.** The relay-driver verbs
  (`due`/`mark_delivered`/`mark_failed`) are the *sidecar's* surface — a driver pulls its own due effects
  and marks the outcome (the never-lost/never-double-sent invariant lives there). A rule **stages** an
  effect and may **inspect** its status; it does not drain or adjudicate the queue. Keeping those off the
  handle keeps the rule author's model coherent and prevents a rule from racing the real relay. The MCP
  verbs still exist for drivers; the rule simply gets no handle method for them.
- **`alert(#{…})` stays as-is — it is already caller-gated, so there is nothing to unify.** The
  `route_alerts` path calls `record_inbox`/`enqueue_outbox`, and **those fns run the caller's
  `authorize_tool` internally** — `alert` therefore already fails closed if the caller lacks the inbox/
  outbox cap. `alert` and the explicit verbs share **one** authority model (the caller's); `alert` is
  simply the ergonomic "raise attention + must-deliver" sugar, the verbs are the explicit full surface.
  Re-plumbing `alert` through `call_tool` would buy only the uniform-gate niceties (§ Intent) for a path
  that is already correct — deferred as a cleanup, not a v1 requirement.

## Related

- `rules-engine-scope.md` — the engine + the `DataSeam`/`AiSeam` this mirrors; the `alert` → inbox/outbox
  routing (`route_alerts`) this generalizes into explicit, caller-gated verbs.
- `rules-ai-wiring-scope.md` — the sibling seam-wiring scope (the `ai.*` model seam); same pattern.
- `../inbox-outbox/` — the durable inbox/outbox planes and their MCP verbs the seam consumes.
- `../channels/` — the channel bus + `channel/authorize.rs` gate the new `channel.*` MCP verbs enforce.
- `../auth-caps/auth-caps-scope.md` — `caps::check` under `caller ∩ grant`, the chokepoint every verb runs.
- `../flows/` — where continuous reaction to channel/inbox motion lives (a rule is bounded; a flow watches).
- `skills/rules/SKILL.md` — the rules Playground how-to the implementing session extends with the
  messaging verbs (a drivable surface — see §6 checklist; the build owns writing the verb examples,
  grounded in a live run).
- README `§6.5` (MCP — the contract), `§6.10` (inbox/outbox), `§3` (rules 3 state-vs-motion, 5
  capability-first, 6 workspace wall, 7 MCP contract, 10 core-knows-no-extension).
