# External-agent scope — the run as a durable job: resume, supervision, read surface

Status: scope (the ask). Sub-scope #5 of `external-agent-scope.md`. Promotes to `public/external-agent/`.

Make an external-agent run a **first-class, durable, supervised** object — the same way the in-house
loop's run is — so it survives the edge disconnecting, can be watched live, resumes without
double-applying effects, and never leaves a zombie subprocess pinning a job. Owns the persistence of
the `RunEvent`s #2 emits, the **resume strategy** (the hard problem with a foreign loop), subprocess
**supervision**, and the small **read surface** (`agent.watch` reuse + the new `agent.runtimes`).

## Goals

- **Run = a durable `job`** (jobs scope): kind `external-agent-run`, payload `{ goal, profile_id,
  caller }`, `ws`, cursor, and a transcript built from the `RunEvent`s #2 emits. **Reuse jobs** — no new
  table. The transcript is the **source of truth**; the agent's own session memory is ephemeral scratch.
- **Watch live:** reuse the existing **`agent.watch`** SSE (`scope/agent-run/` Part 3) over the same
  `RunEvent` stream — an external-agent run is observed identically to an in-house run.
- **Resume per profile:** define and enforce the resume contract. Where the agent supports durable ACP
  `session/load` replay, use it; otherwise **restart-from-goal**, with already-applied **effects** made
  safe by idempotency (gateway idempotency key for model spend, outbox idempotency for external effects)
  — not by replaying the foreign loop's state.
- **Supervision:** a bounded run (wall-time + token/iteration ceiling), **killable** and **restartable**
  (supervisor crate), with a crashed/hung subprocess reaped so it never pins a job or leaks the sandbox.
- **Read surface:** add **`agent.runtimes`** — list the runtimes/profiles this node has configured and
  which is default (read-only, ws-scoped, gated by a read cap) — so the UI can show + pick the runtime.

## Non-goals

- Emitting the `RunEvent`s (#2) or enforcing the wall (#3) — this sub-scope **persists** what #2 emits
  and **supervises** the subprocess #2/#3 launched.
- A general jobs/resume redesign — reuse the jobs primitives as-is; only the external-agent specifics
  (foreign-loop resume, subprocess reaping) are new.
- `agent.profile.*` write CRUD — profiles are deploy config in this slice (open question to promote).
- The model token lifetime across a long run — that's #4 (this sub-scope *consumes* it during resume).

## Intent / approach

**Same run object, foreign engine.** The in-house loop and the external agent both produce a
`RunEvent` stream; persisting that stream into a `job` transcript makes the *run* uniform even though the
*engine* differs. Everything downstream (watch, channel motion, the durable record) is therefore reused,
not rebuilt — the payoff of the one-event-vocabulary stance (`agent-run`).

**Resume safety lives in effects, not loop-state.** The in-house loop resumes from an append-addressed
transcript it owns; we **don't** own the external agent's loop, so we don't pretend to checkpoint it. The
honest contract: prefer ACP `session/load` where an agent supports it durably (encode the capability in
the `AgentProfile`), but the **safety** guarantee comes from making side effects idempotent — the
gateway already de-dupes model calls by idempotency key, and external effects already go through the
outbox with idempotency. So a restart-from-goal cannot double-charge or double-act, even if the foreign
loop starts over. Rejected: faking append-addressed resume over a loop we can't introspect — it would
*look* resumable and silently diverge.

**Supervision is mandatory, not optional.** A foreign subprocess can hang, spin, or crash. The run owns a
wall-time + ceiling, a kill path (also wired to ACP `session/cancel`), and reaping so a dead subprocess
ends its job (`failed`/`cancelled`) and tears down the sandbox + scratch. A zombie that pins a job is a
bug, not a degraded mode.

## How it fits the core

- **Tenancy / isolation:** the job carries `ws` (the hard wall); a ws-B watch/resume can't read a ws-A
  run. Scratch/cache are per-run, per-`ws` (with #3).
- **Capabilities:** `agent.watch` reuses its existing gate; `agent.runtimes` adds a **read** cap
  (`mcp:agent.runtimes:call`) — list-only, no mutation. No write verbs (profiles are config this slice).
- **Placement:** `either`. Hub-hosted runs survive edge disconnect (the reason the agent is hosted
  centrally); an edge solo run supervises its own local subprocess identically.
- **MCP surface (API shape):** **get/list** — `agent.runtimes` (list configured runtimes). **Live feed** —
  `agent.watch` (reused, SSE/bus motion). **No CRUD** — profiles aren't a ws-mutable resource here.
  **Batch** — N/A; a run is itself the long-lived **job**, so it's already "the batch that must be a
  job," not a blocking call.
- **Data (SurrealDB):** the `job:{id}` record (status, kind, payload, cursor, transcript, attempts, `ws`,
  ts) — reuse. State; the transcript is authority.
- **Bus (Zenoh):** `RunEvent`s as ephemeral motion to channel + SSE; the durable record is the job
  (state-vs-motion). `session/cancel` is driven locally to the subprocess, not over the bus.
- **Sync / authority:** hub-authoritative when hub-hosted; the edge re-reads the job's durable progress on
  reconnect (same `(table,id)` upsert the channel sync path covers). Resume idempotency as above.
- **Durability:** must-deliver external effects the agent produces go through the **outbox** (idempotent),
  never raw pub/sub — which is also what makes restart-from-goal safe.
- **Secrets:** N/A directly (the run token is #4).
- **No fake backend (rule 9):** persistence/resume/supervision tested against **real** embedded SurrealDB
  + a **real** agent subprocess; only the provider HTTP is the permitted fake.

## Example flow

1. #1 selects `AcpRuntime`; this sub-scope creates the `job` (kind `external-agent-run`, `ws`, cursor 0)
   and subscribes to #2's `RunEvent` stream, persisting each event into the transcript (durable per step).
2. A user `agent.watch`es the run over SSE — identical to watching an in-house run.
3. The edge disconnects; the hub keeps running; events keep persisting.
4. The subprocess hangs past the wall-time ceiling → supervision **kills** it, reaps the sandbox, marks
   the job `failed: timeout`.
5. A `resume` is requested → per the profile: `session/load` replay if supported, else restart-from-goal.
   A re-applied model call hits the gateway idempotency cache (no double spend); a re-attempted external
   effect hits the outbox idempotency key (no double action).
6. The agent emits `done` → the job is marked `done`; transcript retained per the job's retention policy.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`):

- **Offline / sync (§2.3):** a run survives the edge disconnecting and **resumes** without
  double-applying tool effects or re-spending gateway budget — assert the idempotency caches absorb the
  re-applied step; assert the transcript is the authority, not the agent's memory.
- **Supervision:** a scripted hung/looping agent is **killed** at the ceiling and its job ends
  `failed/cancelled`; the subprocess + sandbox + scratch are reaped (no zombie, no leaked fd). A
  `session/cancel` mid-run stops it cleanly.
- **Terminal-outcome is fail-closed (untrusted agent):** the authoritative run outcome comes from the
  **job / process exit + ceiling**, never the agent's self-reported terminal word. Assert that a `done`
  event carrying an **unrecognised status** maps to `Failed`, **not** `Done` — an untrusted agent
  reporting a status we don't understand must never be read as success. (The motion-side
  `outcome_of`/`RunFinish` mapping in #2 already fails closed on unknown words; #5 owns making the *job*
  outcome authoritative over that hint, including a process that exits non-zero after emitting a
  success-looking line.)
- **Workspace-isolation (§2.2):** a ws-B principal cannot `agent.watch`/resume a ws-A run; `agent.runtimes`
  lists only this node's config and reveals no cross-ws data.
- **Capability-deny (§2.1):** `agent.runtimes` denied without its read cap; `agent.watch` denied per its
  existing gate.
- **Read-surface unit:** `agent.runtimes` lists configured profiles + the default; feature-off node lists
  only `default`.

## Risks & hard problems

- **Resume with a foreign loop is the hard problem.** `session/load` support varies and may be lossy;
  restart-from-goal is the safe default but only *safe* because effects are idempotent — if any agent
  effect escapes the gateway/outbox idempotency, resume can double-apply. Audit every effect path for an
  idempotency key.
- **Zombie / leaked subprocess.** A missed reap pins a job and leaks a sandbox; supervision must reap on
  every exit path (done, error, timeout, cancel, node restart). Test the node-restart path explicitly.
- **Transcript vs the agent's own memory diverging.** We treat our transcript as authority; if a profile's
  resume relies on the agent's memory, the two can disagree. The contract: our transcript wins; the agent
  is re-driven to match, never the reverse.
- **Retention.** External-agent transcripts may include large tool outputs; reuse the job retention policy,
  don't invent a new one.

## Open questions

- **Resume default:** is restart-from-goal the universal default, with `session/load` an opt-in per
  profile that supports it durably — or the reverse? Proposal: restart-from-goal default; `session/load`
  only where a profile asserts durable support. *Note:* the **default agent (Open Interpreter) advertises
  `loadSession: true`** at `initialize` — so ACP `session/load` resume is available for the default and
  is the natural opt-in there; restart-from-goal remains the universal fallback for agents that don't
  advertise it. (Still: our transcript is authority regardless — see Risks.)
- **Ceiling configuration:** per-workspace policy vs a fixed node default (mirrors the agent-scope open
  question). Slice default: fixed node default.
- **Per-workspace run concurrency: DECIDED — unbounded per workspace, but ZERO cross-workspace
  bleed.** A workspace may have as many concurrent external-agent runs as the caller starts (like a
  user opening 100 `vtcode`/`codex` sessions on a PC). "One agent per workspace" means each run is
  **bound to** one `ws` (its isolation wall), **not** that a `ws` is capped at one run. The only
  numeric cap is a node-wide one for host self-protection (don't fork-bomb the machine).

  **The hard invariant: a run launched for `ws=A` can never read, write, or signal `ws=B` — at any
  concurrency.** Because runs are unbounded and concurrent, isolation must be **structural and
  per-process**, never "one at a time." Nothing is shared between runs by default; each `drive(...)`
  constructs its own. The four crossover axes and their seals:
  - **Data / tools (load-bearing):** the MCP endpoint handed to the subprocess is workspace-walled;
    every tool re-runs `caps::check` under the `ws=A` derived principal (#3). A literally cannot *name*
    B's keys. Isolation is this chokepoint, not process-counting.
  - **Filesystem:** each run gets its **own scratch dir and cwd** (never a shared one), confined by the
    OS sandbox to that dir + the ws's allowed roots (#3). Two runs in the *same* ws still get separate
    scratch dirs so they don't stomp each other.
  - **Model / secrets:** the provider token is handed via a **per-process, run-scoped** env (#4) — no
    global key A's process could read B's secret from.
  - **Job / events:** `job:{id}` carries `ws`; `agent.watch`/cancel/resume re-check `ws` (#5), so a
    ws-A principal can't watch or kill a ws-B run.

  Same-tenant note: two runs in the *same* ws are isolated from *other* workspaces but intentionally
  share that ws's data (they are one tenant) — separate scratch dirs are the only same-ws separation.
  (Serialize/queue/cancel-replace, if a product ever wants it, is an *optional* per-profile/per-ws
  policy on top — never the default and never the isolation mechanism.)

  **Code gap today:** `driver::drive(.., workspace, ..)` treats `workspace` as a plain cwd. The
  per-run **scratch dir** (filesystem seal) is addable now and locally testable; the walled MCP
  endpoint + scoped token (data + secret seals) land with #3/#4.
- **`agent.runtimes` shape:** just ids + default, or include health/version per profile? Start minimal.
- **Profile CRUD promotion:** keep profiles as deploy config, or add `agent.profile.*` + a UI later
  (ordinary ws-scoped, capability-gated CRUD if so).

## Related

- `external-agent-scope.md` (umbrella), `acp-driver-scope.md` (#2, emits the `RunEvent`s + `cancel`),
  `capability-wall-scope.md` (#3, the sandbox supervision reaps), `model-routing-scope.md` (#4, the run
  token consumed on resume).
- `scope/jobs/jobs-scope.md` (the durable run job), `scope/agent-run/agent-run-scope.md` (`RunEvent`s +
  `agent.watch`), `scope/outbox/` (idempotent external effects). README `§6.9` (jobs), `§6.10`
  (inbox/outbox), `§6.14`/`§6.16` (run-SSE), `§7`.
