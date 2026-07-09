# Agent dock "blocks" the app — SSE streams exhaust the browser's HTTP/1.1 connection pool

- **Date:** 2026-07-09
- **Area:** frontend / gateway transport
- **Status:** fix implemented (2026-07-09) — the unified event stream shipped backend + client;
  live browser `ss < 6` verify is the one remaining manual step (see "Fix").

## Symptom

While an agent run is active in the dock, navigating around the app appears to hang: the
MCP-backed views (Flows, Rules, Dashboards) don't load — their REST calls sit pending until
the run finishes, then everything pops in at once. Reads as "the agent is blocking the
node".

## What it is NOT

- **Not a blocking agent invoke.** The dock's `ask()` posts a `kind:"agent"` channel item
  and returns immediately (`useDockSession.ts` → `channel.postAgent`); the background
  `agent_reactor` drives the run detached (`rust/crates/host/src/agent_reactor.rs`). No
  request is held open for the answer.
- **Not the store session mutex.** `lb_store`'s `use_ws` guard is per-query, released
  before the LLM turn — brief contention only.
- **Not the reactor job scan.** `lb_jobs::pending` is indexed (the earlier 100%-CPU walk is
  fixed).

## Root cause (live-verified)

The gateway serves plain HTTP/1.1 (`axum::serve`, no TLS — and browsers refuse cleartext
HTTP/2, so h2 multiplexing is unavailable on this posture). Browsers cap HTTP/1.1 at ~6
connections per origin, and **every live feed in the app is its own `EventSource`**, each
permanently occupying one slot:

- dock run stream (`/runs/{job}/stream`) — held for the entire run
- dock + collaboration channel streams (`/channels/{cid}/stream`)
- **one series stream per live dashboard cell** (`useSeries.ts` — subscriber-sharing was
  an explicitly deferred follow-up)
- flows debug/run streams, telemetry, insights, bus streams

Measured on the live dev node (2026-07-09) while a run was active with views open:

```
$ ss -tn state established '( dport = :8080 )' | tail -n +2 | wc -l
6            # exactly the browser's per-origin cap
# same 6 sockets still present 5s later; lastrcv ~6.8s on all six (periodic SSE frames)
```

6/6 slots held by long-lived SSE ⇒ every new `fetch` (the whole REST surface in
`ui/src/lib/ipc/http.ts`) queues at the browser until a stream closes — which is when the
run's stream settles. Hence "the agent blocks navigation": the agent merely guarantees two
of the six slots stay occupied for minutes.

## Fix

Built as the **unified event stream** (`scope/bus/unified-event-stream-scope.md`, session
`sessions/bus/unified-event-stream-session.md`): one multiplexed SSE connection per app session
(`GET /events/stream` + `POST /events/{sid}/{subscribe,unsubscribe}`), all feeds as subjects with
per-subject cap re-check (the gateway reuses each dedicated route's `lb_host::watch_*` gate, never
re-implements it), and a client hub (`ui/src/lib/events/hub.ts`) holding ONE `EventSource` with
refcounted subject dedupe + reconnect re-subscribe. Every `*.stream.ts` opener became a thin adapter
over the hub (signatures unchanged). TLS+HTTP/2 alone was rejected (browsers won't h2c; plain-HTTP
dev/edge posture keeps the cap).

**Proven mechanically** (2026-07-09):
- `rust/role/gateway/tests/events_stream_test.rs::two_subjects_interleave_on_one_connection_with_parity`
  — two subjects ride ONE TCP connection, payloads byte-identical to the dedicated route (6/6 green,
  incl. cap-deny + ws-isolation as opaque per-subject error frames).
- `ui/src/lib/events/hub.test.ts` — the hub holds **exactly one** `EventSource` across N subscribers,
  dedupes N-on-one-subject to one server subscription, refcount-zero unsubscribes, re-subscribes on
  reconnect (5/5 green).

**Remaining (manual):** the symptom's own check — with a run active + a live dashboard,
`ss -tn state established '( dport = :8080 )'` well under 6 and a Flows/Rules REST call completing
mid-run — needs a running dev node + the browser app (couldn't be driven headlessly this session; the
`update-auth` branch's login gate also currently 401s the dev/test node until `LB_DEV_LOGIN` is set).

## Lesson

Every `new EventSource(...)` on an HTTP/1.1 origin permanently spends 1 of the browser's ~6
connection slots for its whole lifetime. A per-feature-stream architecture hits the ceiling
as soon as a handful of live views coexist, and the failure presents *elsewhere* — as
unrelated REST calls hanging. Count established sockets (`ss -tn '( dport = :8080 )'`)
before blaming the server.
