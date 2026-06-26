# Inbox / outbox (as built)

One **normalized item** shape that every source collapses into, persisted as **state** in
SurrealDB behind the workspace wall. The bus moves a live copy (motion); the inbox keeps the
durable record (§3.3). Promoted from `scope/inbox-outbox/` after S2; the transactional **outbox**
and the **resolution facet** landed at S6 (see below).

## The item

`lb_inbox::Item` — `{ id, channel, author, body, ts }`.

- `id` is caller-supplied and **stable**: re-delivering the same `(channel, id)` upserts one row
  (idempotent delivery). The store record id is `inbox:<channel>__<id>`.
- `ts` is a caller-injected **logical** timestamp (no wall-clock — deterministic), the order key.

## Verbs (shipped S2)

- `record(store, ws, item)` — persist into the workspace namespace (idempotent on `(channel,id)`).
- `list(store, ws, channel)` — every item in a channel, **sorted by `ts` oldest→newest**. The
  generic store `list` is a pure filter; the inbox owns the order key and sorts in Rust.

## Guarantees

- **Workspace-isolated:** a `list` in workspace B never returns workspace A's items (items live in
  A's namespace). Tested across store + inbox.
- **Idempotent:** stable item ids make re-delivery safe — the precondition the future must-deliver
  outbox relies on (a receiver dedups on item id).
- **Capability-first:** `record`/`list` are raw verbs; the host `channel` service runs
  `caps::check` before them — no unauthorized path to an item.

## The resolution facet (shipped S6)

An approval is an `Item` tagged `needs:approval` plus a `lb_inbox::Resolution` sibling —
`{ item_id, decision: approved|rejected|deferred, actor, ts }`, keyed by the item id. `resolve` /
`resolution` persist + read it; re-resolving upserts (last decision wins). The `Item` shape stayed
**stable** — the resolution is a separate record, not a new column. This is what the S6 coding
workflow's approval gate reads (see `../coding-workflow/coding-workflow.md`).

## The transactional outbox (shipped S6 — the must-deliver path)

The durability backstop for every external effect (open a PR, post a comment, notify, sync). The
new `lb-outbox` crate: an `Effect` record (`outbox:{id}`) + raw verbs, workspace-namespaced, no auth
(the host `workflow` service is the chokepoint).

- **Transactional enqueue.** `lb_outbox::enqueue` (over the new `lb_store::write_tx`) writes the
  **domain change AND the effect row in one `BEGIN…COMMIT` transaction** — both commit or neither
  does. No window where the change is durable but the effect is lost, nor the reverse. This is *the*
  transactional-outbox pattern (§6.10) — the thing that makes it a backstop, not best-effort pub/sub.
- **At-least-once relay with retry.** `relay_outbox` scans `pending` (status `pending ∪ failed` —
  both schedulable; the durable scan is the backstop, a LIVE push is a later optimization), delivers
  each through a host-owned `Target` trait, and marks it `delivered` (stops re-delivery) or `failed`
  (stays schedulable; retries next pass). An effect that crashed mid-delivery is found again — never
  lost.
- **Idempotent delivery.** Every effect carries a stable `idempotency_key`; the receiver dedups on
  it, so an at-least-once re-delivery is a no-op on the outside world — never double-sent.
- **Workspace-isolated.** Every effect carries `ws`; a ws-B relay scan/mark never sees or touches a
  ws-A effect.

**Tested** (testing §2): the transactional enqueue is atomic; an effect survives an outage and is
delivered on retry; a duplicate delivery is a no-op; ws-isolation across store + relay.

## The egress, hardened (shipped S7 — real adapter + backoff/dead-letter)

The relay went from "retry every pass forever, deliver to an in-test target" to a real, bounded edge:

- **A real GitHub `Target`.** `lb-role-github-target` delivers `create_pr` / `comment` effects to the
  GitHub REST API over `reqwest` (the last mock behind the host `Target` seam — the egress counterpart
  to `lb-role-github-webhook`'s ingress; `reqwest` stays in the role crate, never core). Idempotency
  rides GitHub's own `422 "already exists"` for `create_pr`: a re-delivery is acknowledged, never a
  second PR. The token is mediated, never logged.
- **Backoff.** Each `Effect` carries `next_attempt_ts`; on failure the relay pushes it out by an
  exponential, capped `backoff(attempts)`. The relay scans `due` (schedulable AND past the backoff
  gate), so a tight retry loop no longer hammers a down target.
- **Dead-letter.** Each `Effect` carries `max_attempts` (default 5). At the cap, a failure moves the
  effect to the terminal `DeadLettered` status — parked off the schedulable set, kept for audit/replay
  via `dead_lettered`. A poison message (e.g. a permanently malformed payload) stops retrying.

**Tested:** happy delivery + 422-idempotency through the real adapter over a socket; backoff (owed but
not yet `due`); dead-letter at the cap; a transport failure stays schedulable and delivers on recovery.

## Not yet built

Email / sync-publish `Target` adapters behind the trait + search-before-create dedup; the producer
payload enrichment a live PR needs; the multi-relay atomic claim; FIFO-per-target ordering; the
LIVE-query relay reactor; a resolution reactor that auto-starts the job on approval; item `meta` for
richer payloads; retention/compaction. See `scope/inbox-outbox/`
(`outbox-scope.md`) open questions.
