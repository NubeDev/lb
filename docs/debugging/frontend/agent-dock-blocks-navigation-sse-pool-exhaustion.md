# Agent dock "blocks" the app — SSE streams exhaust the browser's HTTP/1.1 connection pool

- **Date:** 2026-07-09
- **Area:** frontend / gateway transport
- **Status:** open — fix scoped in `docs/scope/bus/unified-event-stream-scope.md`

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

Scoped as the **unified event stream** (`scope/bus/unified-event-stream-scope.md`): one
multiplexed SSE connection per app session, all feeds as subjects with per-subject cap
re-check, client hub with refcounted subject dedupe. TLS+HTTP/2 alone was rejected as the
fix (browsers won't h2c; plain-HTTP dev/edge posture keeps the cap).

## Lesson

Every `new EventSource(...)` on an HTTP/1.1 origin permanently spends 1 of the browser's ~6
connection slots for its whole lifetime. A per-feature-stream architecture hits the ceiling
as soon as a handful of live views coexist, and the failure presents *elsewhere* — as
unrelated REST calls hanging. Count established sockets (`ss -tn '( dport = :8080 )'`)
before blaming the server.
