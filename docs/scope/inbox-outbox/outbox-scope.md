# Inbox-outbox scope — the transactional must-deliver outbox

Status: scope (the ask). Promotes to `public/inbox-outbox/` once the S6 slice ships.

> Read with: `inbox-outbox-scope.md` (the inbox half + the S2/S3 history), `../../README.md`
> §6.10 (inbox/outbox), §3.3 (state vs motion), `../sync/sync-scope.md` (the append-style
> idempotent-apply subset S3 shipped), `../jobs/jobs-scope.md` (jobs and the outbox are peers —
> both are SurrealDB records driving durable work).

The **transactional outbox**: a durable, must-deliver backstop for every *external effect* a
workspace produces (open a PR, post a GitHub comment, notify a reviewer, publish a sync update,
start a downstream workflow). The domain change and the intent-to-deliver are written in **one
SurrealDB transaction**, so an effect can never be lost (committed) nor double-sent (idempotent
on the receiver). A relay then publishes pending rows at-least-once with retry. This is the
durability promise §6.10 makes and the **driver of the S6 coding workflow** — the previous slices
shipped only the append-style idempotent-apply *subset* (S3 channel sync); the cursor-driven
must-deliver outbox is new here.

## Goals

- An **`Effect` record** — the normalized must-deliver shape every target collapses into
  (`target`, `action`, `payload`, `idempotency_key`, `status`, `attempts`, `ts`). State, in
  SurrealDB, behind the workspace wall — no separate queue or datastore (§3.2).
- **Transactional enqueue** — the domain write (a job step, a doc, an inbox resolution) AND the
  outbox row land in **one transaction**. Either both commit or neither does; an effect is never
  orphaned from the change that justified it, and a change never silently fails to schedule its
  effect.
- **At-least-once relay with retry** — a relay scans `pending` (a durable backstop; a LIVE query
  is the push optimization, not the source of truth — §6.1/§6.2), delivers each, and marks it
  `delivered` or bumps `attempts` and leaves it `pending` for the next pass. A delivery that
  crashed mid-flight re-delivers on the next scan — never lost.
- **Idempotent delivery** — every effect carries a stable `idempotency_key`; the receiver
  (a target adapter, e.g. `github-bridge`) dedups on it, so an at-least-once re-delivery is a
  no-op on the outside world (never double-sent). This is the receiver-side extension of the
  `(channel,id)` upsert rule the inbox/sync already rely on.
- **Workspace-scoped** — every effect carries `ws`; a relay for workspace B never sees, claims,
  or delivers workspace A's rows. The hard wall holds at store *and* relay.

## Non-goals (S6)

- **Per-target transport adapters** (the real GitHub HTTP client, an SMTP sender). The relay
  delivers to a **`Target` trait**; S6 ships a deterministic in-test target (records what it was
  asked to deliver, can be told to fail-then-succeed) — the only external mocked (testing §3).
  Real adapters are S7 and ride behind the same trait.
- **Backoff/scheduling math** (exponential delay, `run_at`, dead-letter after N). S6 retries on
  the next relay pass and records `attempts`; the backoff curve + dead-letter policy are the
  jobs-queue follow-up (deferred there too — same reason: no contention/throughput pressure yet).
- **Multi-relay contention** (two relays racing for one row) — the atomic-claim primitive is
  deferred with the jobs queue (jobs scope). S6 has one hub relay.
- **Ordering guarantees across effects** — each effect is independent and idempotent; the S6
  workflow does not require a total order, so none is promised. (FIFO-per-target is a later ask.)

## Intent / approach

**The outbox row is written in the same transaction as the change that caused it** — that is the
whole pattern, and the one thing that makes it *transactional* rather than best-effort pub/sub.
A new store verb `write_with_effect` issues one `BEGIN … COMMIT` containing both upserts. If the
process dies after the commit, the relay finds the pending row and delivers it; if it dies before,
neither the change nor the effect exists, and the caller's retry re-runs the whole transaction. No
window exists where the change is durable but the effect is lost.

**The relay is a function over pending rows, not a service that holds state** (stateless-extension
spirit, §3.4): `relay_outbox` reads `pending`, calls the target, and marks the row — all durable
in the store. Kill it mid-pass and the next pass resumes from the same `pending` set, because a
not-yet-`delivered` row is still pending. Re-delivery is safe because the receiver dedups on
`idempotency_key`.

**Why not reuse the job queue for effects** (jobs-scope open question): a job is a *resumable
session* (a transcript + cursor the agent drives); an effect is a *fire-once intent to the outside
world*. They share the "SurrealDB record + idempotent + ws-scoped" shape but differ in lifecycle —
a job completes when its loop ends; an effect completes when an external system acknowledges it.
Keeping them as separate tables keeps each verb single-responsibility (FILE-LAYOUT) and lets the
relay scan only effects. **Rejected:** one polymorphic `work` table — it would force every job
verb to filter out effects and vice-versa, and blur "session state" with "delivery intent".

**Why a `Target` trait and not a direct call** — the relay must be testable deterministically and
the target set must be extension-provided (a new target doesn't touch the core, §6.10 finding). A
trait seam (`deliver(&Effect) -> Result`) is the same shape as the agent's `ModelAccess` seam:
the host owns the trait; an adapter (or the test) supplies the impl. **Rejected:** the relay
calling a concrete HTTP client — un-testable without network and un-swappable.

## How it fits the core

- **Tenancy / isolation:** every `Effect` carries `ws`; `enqueue`/`pending`/`mark_*` select the
  workspace namespace first (structural isolation, §7). A relay is invoked per workspace; a ws-B
  relay scan returns only ws-B rows. Mandatory isolation test: a ws-B relay never delivers a ws-A
  effect (store + relay).
- **Capabilities:** the raw outbox verbs assume the caller passed `caps::check` — exactly like
  `lb-inbox`/`lb-jobs`. The **host `workflow` service** is the chokepoint: enqueuing an effect is
  part of a capability-checked workflow action (e.g. `start_coding_job` requires the workflow
  grant). Deny path: a caller without the workflow capability cannot enqueue an effect (the change
  *and* its effect are both refused, because the gate is before the transaction).
- **Placement:** *either*, by config (symmetric nodes). The outbox table exists on every node; the
  relay runs where the target is reachable (default: the hub). No `if cloud` — placement is which
  node mounts the relay loop, not a code branch.
- **MCP surface:** consumes nothing; exposed indirectly — the workflow verbs that enqueue effects
  are MCP tools (`workflow.*`). The outbox itself is a host-internal durability mechanism, not a
  tool surface (like the inbox: state behind the verbs, not a verb itself).
- **Data (SurrealDB):** a new `outbox` table; rows are `outbox:{id}`. **State**, not motion — the
  row is the durable source of truth; the relay's act of delivering is the motion. Status is
  `pending | delivered | failed`. The `write_with_effect` verb is the transactional seam.
- **Bus (Zenoh):** the **must-deliver** message class (§6.2). An effect is precisely the thing
  that must NOT ride raw fire-and-forget pub/sub — it goes through the durable outbox so a relay
  re-tries it. (Progress *chatter* during the job is the fire-and-forget class and rides the bus
  directly — the two classes are kept distinct, §6.2.)
- **Sync / authority:** the hub is authoritative for the outbox when hub-hosted; the row is the
  `(table,id)` upsert the sync path already covers, so an edge cache converges on delivery status.
  Offline: an edge that produced an effect while partitioned keeps the pending row; on reconnect
  the hub relay delivers it (at-least-once) — the offline/sync mandatory category.
- **Secrets:** N/A to the outbox row (no secret material in the payload). The *target adapter*
  (S7) holds the provider credential, mediated by the secrets surface — never the effect row.

## Example flow (the S6 driver)

1. The coding job, mid-loop, decides to open a PR. It does NOT call GitHub. It calls the workflow
   verb that, in **one transaction**, appends the job step (the domain change) AND enqueues an
   `Effect { target: "github", action: "create_pr", payload, idempotency_key: "pr:issue-2451",
   status: pending }`.
2. The transaction commits. The job step and the outbox row are both durable, atomically.
3. The hub **relay** scans `pending` for the workspace, finds the row, and calls the `github`
   `Target`. The target performs the effect and acknowledges.
4. The relay marks the row `delivered`. A second relay pass sees no pending row — no double-send.
5. **Failure path:** the target is down. The relay bumps `attempts`, the row stays `pending`. The
   next pass re-delivers; because the row carries `idempotency_key: "pr:issue-2451"`, the receiver
   recognizes the retry and does not open a second PR. The effect survives the outage — the
   durability backstop, not best-effort.

## Testing plan

Mandatory categories (testing §2) — these are the S6 gate, not extras:

- **Capability-deny** (§2.1): a caller without the workflow grant cannot enqueue an effect — the
  gate is before the transaction, so neither the change nor the effect lands.
- **Workspace-isolation** (§2.2): a ws-B relay scan returns no ws-A effects; a ws-B `mark_delivered`
  cannot touch a ws-A row — across store + relay.
- **Offline / sync** (§2.3, the headline): an effect **survives a disconnect and is delivered
  at-least-once, idempotently** — (a) a relay that fails the first delivery re-delivers on the next
  pass and the row ends `delivered` (never lost); (b) a duplicate delivery is a no-op on the
  receiver (dedup on `idempotency_key` — never double-sent); (c) the `write_with_effect`
  transaction is atomic — a forced failure leaves **neither** the change nor the effect (no
  orphaned effect, no silent drop).
- Unit: the `Effect` shape + status transitions; the relay's pending-scan filter; injected
  clock/ids (determinism, §3 — no wall-clock).
- Integration (real embedded SurrealDB + in-proc Zenoh; the `Target` is the only stub): the full
  enqueue → relay → deliver → mark path, and the retry path.

## Risks & hard problems

- **The transaction is the load-bearing claim.** A bug that writes the effect outside the
  transaction (or the change outside it) reintroduces the lost-update / orphaned-effect window the
  whole pattern exists to close. The `write_with_effect` verb is one small, heavily-tested
  function issuing one `BEGIN…COMMIT`; there is no second enqueue path.
- **At-least-once means the receiver MUST dedup.** The outbox guarantees delivery, not
  exactly-once; correctness depends on the `idempotency_key` being stable and the receiver
  honoring it. The key is derived from the domain change (e.g. `pr:issue-2451`), not generated, so
  a retry computes the same key.
- **Relay liveness vs the LIVE query.** A LIVE query gives instant pickup but is ephemeral
  (§6.2) — the durable `pending` scan is the backstop. S6 uses the scan (correct, simple); the
  LIVE push is the latency optimization layered on later.

## Open questions

- ~~**Backoff + dead-letter:**~~ **RESOLVED (S7):** an effect carries `max_attempts` (default 5) and
  `next_attempt_ts`; on each failure `mark_failed` either pushes the next retry out by an exponential,
  capped `backoff(attempts)` or — at `max_attempts` — moves it to the terminal `DeadLettered` status
  (parked, off the schedulable set, readable via `dead_lettered` for audit/replay). The relay scans
  `due` (schedulable AND past the backoff gate) instead of `pending`. See
  `../../sessions/coding-workflow/outbox-egress-session.md`.
- ~~**Real `Target` adapter:**~~ **RESOLVED (S7):** `lb-role-github-target` delivers `create_pr` /
  `comment` effects to the GitHub REST API over `reqwest` (the in-test target was the only stub).
  Idempotency rides GitHub's `422 "already exists"` for `create_pr`. Email / sync-publish adapters +
  search-before-create dedup stay open.
- **Atomic claim for multi-relay:** the `UPDATE … WHERE status='pending' RETURN BEFORE` primitive
  (same as the jobs queue) — recorded, deferred (one hub relay at S6).
- **FIFO-per-target ordering:** some targets need ordered delivery (comment-before-close). Not
  required by the S6 flow; opens when a target needs it.
- **Target registration:** the relay's `Target` set is a static map at S6; making it
  extension-provided (a target is an installed artifact, §6.10 finding) opens with the registry
  (S7).
- **Retention/compaction:** `delivered` rows accumulate; a compaction policy (like channel
  history) is deferred.

## Related

- README `§6.10` (inbox/outbox), `§6.2` (message classes — must-deliver vs fire-and-forget),
  `§6.9` (jobs — the peer record), `§6.8` (sync authority), `§3.3` (state vs motion).
- `inbox-outbox-scope.md` — the inbox half + why the outbox was deferred to a multi-node stage.
- `../jobs/jobs-scope.md` — the resumable-session record the workflow drives; the outbox is its
  must-deliver sibling.
- `../coding-workflow/coding-workflow-scope.md` — the S6 orchestrator that drives this outbox.
- `../../vision/0002-coding-agent-workplace.md` — the worked example (steps 8–9 are this outbox).
</content>
</invoke>
