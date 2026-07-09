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

Real infra, seeded via the real write path — no mock, no fake backend (CLAUDE §9).

**Backend — `rust/role/gateway/tests/events_stream_test.rs` (6/6 green).** Live socket + real
node + real bus. Drives subjects via `bus:{subject}` (`POST /bus/publish`) for deterministic
timing (Zenoh is fire-and-forget). Covers both mandatory categories + parity + unsubscribe:

```
running 6 tests
test the_stream_without_a_token_is_401 ... ok
test hello_frame_mints_a_sid_and_registers_the_connection ... ok
test a_denied_subject_is_an_opaque_error_frame_the_connection_survives ... ok   # MANDATORY cap-deny
test a_cross_workspace_subject_is_the_same_opaque_deny ... ok                    # MANDATORY ws-isolation
test two_subjects_interleave_on_one_connection_with_parity ... ok               # mux + parity vs /bus/stream
test unsubscribe_stops_frames_and_releases_the_subject ... ok                   # unsubscribe releases the task
test result: ok. 6 passed; 0 failed; 0 ignored
```

- **cap-deny:** a session without `mcp:agent.watch:call` subscribes `run:{job}` → opaque `error`
  mux frame; the connection stays up and a permitted `bus:` subject on the SAME connection still
  streams.
- **ws-isolation:** ws-B subscribes `bus:cooler/alerts`, ws-A publishes the same subject name in
  its own workspace → ws-B's stream never sees ws-A's payload (the wall holds; the mux is not an
  existence oracle).
- **parity:** the mux `data` for a publish is byte-identical (`"data":{"v":1}`) to what the
  dedicated `/bus/stream` route emits for the same publish.
- Regression: the existing `gateway_routes_test` + `bus_routes_test` stay green (8 + 4).

**Frontend — `ui/src/lib/events/hub.test.ts` (5/5 green).** The hub's multiplexing invariants,
driven with a counting `EventSource` stub (a jsdom transport polyfill, not a fake backend):

```
✓ src/lib/events/hub.test.ts (5 tests)
  ✓ holds exactly ONE EventSource across N subscribers on different subjects
  ✓ dedupes N subscribers on ONE subject to a single server subscription, fanning frames to all
  ✓ releases the server subscription only when the LAST subscriber unsubscribes (refcount)
  ✓ routes a mux frame only to its own subject's listeners
  ✓ re-subscribes the whole live set on reconnect (a fresh hello mints a new sid)
```

`tsc --noEmit` clean across the 9 migrated openers.

**Frontend e2e — `ui/src/lib/events/hub.gateway.test.ts` (written, currently blocked).** A real
fetch-streaming `EventSource` shim over the spawned node; opens the hub, subscribes a `bus:`
subject, publishes via `POST /bus/publish`, asserts the frame round-trips + exactly one connection.
It is **blocked by a pre-existing harness breakage on this `update-auth` branch**: `test_gateway`
now hard-refuses password-less `/login` with `401` (`Gateway::boot` selects `PasswordHash` unless
`LB_DEV_LOGIN` is set — commit `2e58677 updates to login/access`), so EVERY `*.gateway.test.ts`
(incl. the known-good `channel.api.gateway.test.ts`) fails at `signInReal`. Not this slice's
regression; the auth work owns the `real-gateway.ts` env fix. The hub e2e passes as soon as login
does. The backend integration test spawns its OWN `DevTrustAny` gateway (`Gateway::new`), so it is
unaffected and is the authoritative end-to-end proof.

## Debugging

`debugging/frontend/agent-dock-blocks-navigation-sse-pool-exhaustion.md` — updated to
**fix implemented; live browser verify pending** (not "fixed"): the mechanism is proven
(two subjects on one TCP connection in the Rust parity test; exactly one `EventSource` across N
subscribers in the UI unit test), but the symptom's `ss -tn '( dport = :8080 )' < 6` browser
check needs a running dev node + the browser app, which this environment can't drive headlessly
(and the dev node is entangled with the concurrent auth session). Recorded as the one remaining
manual step, honestly.

## Public / scope updates

- Promoted to `doc-site/content/public/bus/bus.md` (the unified-event-stream section) + a
  `content/public/SCOPE.mdx` row.
- Scope open questions resolved in `scope/bus/unified-event-stream-scope.md`: subject grammar =
  flat `kind:id` (first-colon split, so `bus:` keeps its `/`s); queue depth = 1024 drop-oldest for
  motion; dedicated routes stay silently supported (no deprecation warning yet); `app/sdk` = later
  follow-up; resume cursors = deferred (v1 re-runs snapshot catch-up).

## Skill docs

`skills/event-stream/SKILL.md` — written, grounded in the backend integration run (open the mux
stream, read `hello {sid}`, POST subscribe, observe `mux` frames, the opaque deny frame). The
control surface is API-drivable; the skill documents the exact wire shapes from the green test.

## Dead ends / surprises

- A concurrent session on the same repo transiently commented out the `Gateway.events` field
  mid-write; the interleaved snapshot looked like my modules had been deleted. Re-checking from
  disk showed everything intact and compiling — a false alarm from reading a mid-write state, but
  worth noting: this branch has parallel edits landing on `state.rs`/`server.rs`/`login`.
- The `update-auth` login gate (above) broke the whole UI gateway harness independently of this
  slice.

## Follow-ups
- STATUS.md — slice added (in flight / shipped-backend).
- Live browser `ss < 6` verify once a dev node is up on the auth-fixed branch.
- `app/sdk` (RN shell) adopts the hub (scope non-goal → follow-up).
