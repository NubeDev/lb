# Bus — unified event stream (session)

- Date: 2026-07-09
- Scope: ../../scope/bus/unified-event-stream-scope.md
- Stage: S10 (frontend/bus, per STATUS.md — the SSE-pool defect)
- Status: in-progress

## Goal

Kill the "agent dock blocks navigation" defect (SSE pool exhaustion,
`debugging/frontend/agent-dock-blocks-navigation-sse-pool-exhaustion.md`) by
multiplexing every browser live feed onto ONE SSE connection per session:
`GET /events/stream` + `POST /events/{sid}/subscribe|unsubscribe`, each subscribe
re-running the EXACT per-subject cap gate + workspace wall of the dedicated route,
plus a client hub that fans frames out with refcounted dedupe. Exit gate: with a run
active + a live dashboard open, established browser→gateway sockets well under 6 and a
REST call completes mid-run.

## What changed

### Gateway (Rust) — `rust/role/gateway/src/`

- `session/events/` (the mux): a per-connection registry of live subscriptions.
  - `hub.rs` — `EventHub`: process-wide map `sid -> ConnHandle`. A `ConnHandle` owns an
    mpsc sender to the SSE task + a set of active subject → abort-handle. Connection drop
    removes the sid and aborts every subject task (ephemeral, connection-scoped — scope
    "Data (SurrealDB): none").
  - `subject.rs` — the **subject registry**: parses an opaque subject string
    (`run:{job}`, `channel:{cid}`, `series:{s}`, `bus:{subject}`, `flow-run:{run}`,
    `flow-debug:{flow}`, `insights`, `telemetry`) and, for each, calls the SAME
    `lb_host::*` gate+snapshot+feed the dedicated route calls, adapting its heterogeneous
    handle into one boxed `Stream<Item = MuxFrame>` (`{event, data}`). The gate is NEVER
    re-implemented — a deny from `lb_host` becomes a per-subject error frame.
- `routes/events.rs` — the three HTTP verbs:
  - `GET /events/stream?token=` → verify token, mint `sid`, emit `event: hello {sid}`,
    then fold the connection's mpsc into `event: mux` frames.
  - `POST /events/{sid}/subscribe {subject}` / `/unsubscribe {subject}` — header-authed;
    subscribe re-runs the subject's gate (via the registry) inside the sid's workspace,
    spawns the feed task piping `mux` frames to the connection; a gate failure pushes one
    `event: mux {sub, event:"error", data}` and returns 200 (connection lives).

### Client (UI) — `ui/src/lib/events/`

- `hub.ts` — singleton: ONE `EventSource` to `/events/stream`, `subscribe(subject, onFrame)`
  with refcounted dedupe (N callers on one subject = one server subscription), re-subscribe
  of the whole live set on reconnect.
- Existing `*.stream.ts` openers become thin adapters over the hub (signatures unchanged).

## Decisions & alternatives

- **Subject registry adapts existing `lb_host` handles into one boxed frame stream** rather
  than a shared trait rewrite of each route. Each route keeps calling its `lb_host::watch_*`
  fn; the mux calls the *same* fn. This is the scope's "reuse the SAME handler, one owner"
  constraint met with the least surface — the gate physically cannot drift because it is one
  function call, not a copy.
- Rejected: a `Subject` trait each feature implements — more churn, and it would tempt a
  re-implementation of the gate inside the trait impl.

## Tests
(pending — see below)

## Debugging
(the defect entry gets flipped to fixed on live-verify)

## Public / scope updates
(pending)

## Skill docs
`skills/event-stream/SKILL.md` — pending, grounded in a live subscribe run.

## Dead ends / surprises
(none yet)

## Follow-ups
- STATUS.md row — pending.
