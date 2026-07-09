# Bus scope — unified event stream (the browser leg of the bus)

Status: **built (2026-07-09)** — backend + client shipped and tested; promoted to
`doc-site/content/public/bus/bus.md`. Session: `sessions/bus/unified-event-stream-session.md`.
One manual step left (the live browser `ss < 6` verify).

The browser app opens **one `EventSource` per live thing it watches** — a run stream, a
channel stream, a series stream *per dashboard cell*, flow debug/run streams, telemetry,
insights. The gateway is plain-HTTP/1.1 (`axum::serve`, no TLS), and browsers cap HTTP/1.1
at **~6 connections per origin** — a limit HTTP/2 would lift, but **browsers refuse
cleartext HTTP/2 (h2c)**, so on our plain-HTTP dev/edge posture the cap is structural.
Once ~6 SSE streams are open, every new `fetch` (flows, rules, dashboards — the whole REST
surface in `ui/src/lib/ipc/http.ts`) queues at the browser until a slot frees. This is the
live-verified "agent dock blocks navigation" defect
(`debugging/frontend/agent-dock-blocks-navigation-sse-pool-exhaustion.md`: exactly 6/6
established connections held by SSE while a run was active). The fix is to stop spending a
connection per subject: **one multiplexed SSE event channel per browser session**, every
live feed a *subject* riding it, and a client-side hub that fans frames out to features.

## Goals

- **One long-lived streaming connection per app session**, regardless of how many runs,
  channels, cells, or consoles are live. The other ~5 connection slots stay free for REST.
- **Dynamic subscribe/unsubscribe** without reconnecting — navigating around the app adds
  and removes subjects on the existing connection.
- **Per-subject capability re-check on subscribe** — the exact same gate the dedicated
  route runs today (`mcp:agent.watch:call` for a run, `flows.debug.watch` for a debug tail,
  …). A deny is a per-subscription error frame, never a connection kill, and never a leak.
- **Frame-shape compatibility**: every existing stream's frames ride unchanged inside a mux
  envelope, so all client fold logic (`useRunFeed.fold`, channel merge, series append) is
  untouched.
- **Subscriber sharing for free**: N dashboard cells on one series = one subscription (the
  hub dedupes by subject) — closing the "one EventSource per series, fanned to N cells"
  follow-up `useSeries.ts` explicitly deferred.
- Incremental migration: each `ui/src/lib/**/*.stream.ts` opener keeps its public API and
  becomes a thin adapter over the shared hub — feature code does not change.

## Non-goals

- **Not a WebSocket migration.** SSE + control POSTs keeps every existing frame shape, the
  gateway's SSE-first posture (§6.13), and the browser's native `EventSource` reconnect.
- **Not TLS/HTTP-2.** That is deployment posture (config, not code) and *complementary* —
  a TLS deployment gets h2 multiplexing on the REST surface too — but it cannot be the fix
  because the plain-HTTP dev/edge node (the default posture, Pi-class hardware, no certs)
  would still be capped. Note: browsers do not do h2c, so "just enable HTTP/2" is a no-op
  without TLS.
- **Not removing the dedicated per-feature SSE routes** (`/runs/{job}/stream`,
  `/channels/{cid}/stream`, …). They stay for curl debugging, tests, extensions, and the
  ACP/stdio side. The browser app's libs stop using them; deprecation is a later decision.
- **Not the RN app shell** (`app/`). Its SDK should adopt the same hub later — named as a
  follow-up, not part of this scope.
- **Not durable delivery.** The stream is motion (fire-and-forget frames + per-subject
  snapshot catch-up on subscribe); must-deliver stays with the outbox/durable reads, as
  today (§3 rule 3, §6.10).

## Intent / approach

One new gateway surface, one new client lib:

1. **`GET /events/stream?token=`** — the session's single SSE connection. First frame is
   `event: hello` carrying a server-minted stream id (`sid`). Every subsequent frame is
   `event: mux` with `data: {"sub": "<subject>", "event": "<original event name>",
   "data": <original frame verbatim>}` — the envelope adds routing, the payload is
   byte-identical to what the dedicated route emits today.
2. **`POST /events/{sid}/subscribe`** `{ subject }` and **`POST /events/{sid}/unsubscribe`**
   `{ subject }` — header-authed control verbs on the live connection. Subscribe runs the
   subject's capability gate + workspace wall, then replays the subject's snapshot/catch-up
   (exactly what its dedicated route does on connect) and joins the live feed. A gate
   failure emits `event: mux` `{"sub": …, "event": "error", "data": {opaque}}` on the
   stream — the connection lives on.
3. **Subject grammar** (opaque strings to the mux; each maps 1:1 to an existing route's
   semantics, reusing the SAME handler logic extracted behind a shared function):
   `channel:{cid}`, `run:{job}`, `series:{series}`, `bus:{subject}`,
   `flow-run:{run_id}`, `flow-debug:{flow_id}`, `insights`, `telemetry`.
4. **Client hub** — `ui/src/lib/events/` (FILE-LAYOUT: one verb per file): a singleton
   holding the one `EventSource`, `subscribe(subject, onFrame): () => void` with refcounted
   dedupe (two cells on one series share one server subscription), auto re-subscribe of the
   full subject set after an `EventSource` reconnect (each subject then re-runs its
   snapshot catch-up — same semantics as today's per-stream reconnect). Every existing
   `*.stream.ts` opener (`run.stream.ts`, `channel.stream.ts`, `series.stream.ts`, …)
   keeps its exported signature and delegates to the hub.

**Alternative rejected — TLS + HTTP/2 only:** lifts the browser cap to ~100 streams on one
connection, but only under TLS (browsers refuse h2c), so the default plain-HTTP node keeps
the defect; it also does nothing about server-side fan-out (N axum SSE tasks and N bus
subscriptions per browser tab) or the per-cell stream multiplication. Kept as recommended
*deployment* posture, rejected as the *fix*.

**Alternative rejected — client-side stream budgeting** (pause background views' streams,
keep only the visible view live): a mitigation, not a fix — the ceiling remains, every
feature grows visibility bookkeeping, and a single busy view (dashboard with 6 live cells +
the dock mid-run) still saturates alone.

**Alternative rejected — WebSocket channel:** genuinely bidirectional (subscribe could ride
the socket instead of control POSTs), but it reframes every existing SSE payload, adds a
second auth path, and buys nothing SSE + POST doesn't for this problem. Zenoh already owns
node-to-node motion; the browser leg only needs server→client frames plus rare control
writes.

## How it fits the core

- **Tenancy / isolation:** the workspace comes from the connection's token; every subject
  is resolved inside that workspace only. A subscribe naming another workspace's subject is
  the same opaque deny as an unknown subject — the mux is not an existence oracle.
- **Capabilities:** subscribe re-runs the exact per-subject gate its dedicated route runs
  today (run → `mcp:agent.watch:call`, flow debug → `flows.debug.watch`, telemetry → its
  grant, channel/series → their read gates). Deny = per-subject opaque error frame. The
  mux itself grants nothing; it is a lens over feeds the session could already open.
- **Placement:** gateway role only (the browser-facing surface, §6.13). Symmetric nodes —
  no edge/cloud branch; whether a node serves it is role config.
- **MCP surface (§6.1):** live feed + two small synchronous control verbs
  (`events.subscribe` / `events.unsubscribe` — bounded, no I/O fan-out, never long). No
  CRUD, no batch, N/A by design. Existing `watch`-style tools are unchanged.
- **Data (SurrealDB):** none. Subscriptions are connection-scoped ephemeral state in the
  gateway process (a dropped connection drops them; reconnect re-subscribes). Snapshot
  catch-up reads whatever the subject's dedicated route already reads.
- **Bus (Zenoh):** each subject's server side keeps its existing ws-walled bus
  subscription; the mux consolidates *browser* connections, not bus subjects. Frames stay
  fire-and-forget motion; per-subject bounded queues drop-oldest under backpressure so one
  chatty subject cannot wedge the connection (must-deliver truth stays durable, as today).
- **Sync / authority:** node-local; nothing cross-node changes.
- **Secrets:** none new. The token rides `?token=` on the one stream (EventSource cannot
  set headers) — the same exposure as every existing stream route, now on one URL instead
  of nine.

## Example flow

1. The app logs in and opens `GET /events/stream?token=…` once. `hello {sid}` arrives.
2. The user opens a dashboard with four live cells on two series. The hub dedupes to two
   `subscribe {subject: "series:…"}` POSTs; each replays its latest-sample catch-up, then
   live samples interleave on the one connection.
3. The user asks the agent in the dock. The dock subscribes `run:{job}` and
   `channel:{dock-cid}` — still one TCP connection. RunEvents stream live.
4. Mid-run the user navigates to Flows. The flows list `fetch` fires and completes
   immediately — 5 of 6 browser connection slots are free (this is the defect, fixed).
5. Leaving the dashboard unmounts the cells; refcounts hit zero; the hub unsubscribes the
   two series subjects. The connection stays up.
6. The laptop sleeps; `EventSource` reconnects; a new `hello {sid}` arrives; the hub
   re-subscribes the current subject set; each subject re-runs its snapshot catch-up — the
   UI heals exactly as today's per-stream reconnect does.

## Testing plan

Per `scope/testing/testing-scope.md` — real gateway, real store, real bus, no fakes:

- **Capability deny (mandatory):** a session without `mcp:agent.watch:call` subscribes
  `run:{job}` → an opaque per-subject error frame; the connection stays open and a
  permitted subject on the same connection still streams.
- **Workspace isolation (mandatory):** a ws-B token subscribes ws-A's `channel:{cid}` /
  `series:{s}` → the same opaque deny as an unknown subject; no frame ever crosses.
- **Mux correctness:** subscribe two subjects (a channel + a run), drive both server-side,
  assert both feeds interleave on one connection with payloads byte-identical to the
  dedicated routes' frames (parity test against `/runs/{job}/stream`).
- **Catch-up parity:** subscribing a subject mid-history replays the same snapshot the
  dedicated route serves on connect.
- **Unsubscribe:** after unsubscribe, further server-side events for that subject emit
  nothing on the stream (and the subject's bus/task resources are released).
- **UI (gateway harness, `pnpm test:gateway`):** the hub holds exactly ONE `EventSource`
  across N feature subscribers; two subscribers on one subject share one server
  subscription; refcount-zero unsubscribes; reconnect re-subscribes the live set; the
  migrated `run.stream.ts`/`channel.stream.ts`/`series.stream.ts` adapters drive the
  existing feature tests green unchanged.
- **Live verify (the symptom):** with a run active and a live dashboard open, count
  established browser→gateway connections (`ss -tn state established '( dport = :8080 )'`)
  — must be well under 6 (expected: 1 stream + transient fetches), and a flows/rules REST
  call must complete during the run.

## Risks & hard problems

- **Head-of-line within the one connection:** a chatty subject (a fast series) shares the
  pipe with everything. Per-subject bounded queues with drop-oldest for motion frames keep
  the connection healthy; the risk is choosing bounds that never visibly lag the dock's
  run feed. (Today's separate connections had the same total bandwidth — this is about
  fairness, not capacity.)
- **Bigger reconnect blast radius:** one drop now interrupts every live feed at once, and
  recovery re-runs every subject's catch-up. The hub must stagger/re-subscribe robustly;
  the per-subject catch-up semantics (already restart-safe) do the healing.
- **Extracting shared subject handlers without widening:** each subject must reuse the
  dedicated route's gate + snapshot logic (one owner), not a re-implementation — a second
  copy is where a cap check silently goes missing.
- **Ordering:** per-subject ordering is preserved (one server task per subject, one pipe);
  cross-subject ordering is not guaranteed — same as today's independent connections, but
  worth stating so nobody builds on an accident.
- **Token expiry mid-stream:** one long-lived connection outlives short-TTL tokens more
  often than nine short ones did. Subscribe re-checks at subscribe time; a 401 on the
  stream reconnect follows the existing session-clear path.

## Open questions

Resolved as built (2026-07-09, `sessions/bus/unified-event-stream-session.md`):

- **Subject grammar — RESOLVED: flat `kind:id` strings**, split on the FIRST colon (so a
  `bus:` subject keeps its own `/`s and inner colons; `insights`/`telemetry` are all-kind, empty
  id). Opaque to the mux; parsed only by the subject registry (`session/events/subject.rs`).
- **Per-subject queue depth + drop policy — RESOLVED: one bounded (1024) per-connection queue,
  drop-oldest for motion frames** (`try_send`; a full queue drops the frame, a closed queue stops
  the task). All feeds share the connection queue rather than a per-subject one — simpler, and the
  connection is what the browser cared about. Run/flow catch-up stays bounded by the host's snapshot
  read (unchanged). A per-subject queue with per-class policy is a refinement if a chatty series is
  ever seen to starve the run feed (not observed).
- **Dedicated route deprecation — RESOLVED: stay silently supported.** No deprecation warning; the
  routes remain for curl/tests/extensions/ACP (a Non-goal was removing them). The browser app's
  libs stopped using them.
- **`app/sdk` (RN shell) — RESOLVED: follow-up**, as assumed (a scope Non-goal). The hub is
  browser-only (`ui/src/lib/events/`); the RN SDK adopts the same shape later.
- **Per-subject resume cursors — RESOLVED: deferred.** v1 re-runs the snapshot catch-up on
  reconnect (matches today's per-stream reconnect). `Last-Event-ID`-style cursors remain the named
  optimization.

Remaining (not a design question — an environment step): the live browser `ss < 6` symptom verify,
pending a running dev node on the auth-fixed branch.

## Related

- README §3.3 (state vs motion), §6.2 (bus), §6.13 (gateway/SSE surface).
- `scope/bus/bus-scope.md` — the Zenoh (node-to-node) half; this scope is the browser leg.
- `scope/agent-run/` — the `RunEvent` stream that becomes the `run:{job}` subject.
- `debugging/frontend/agent-dock-blocks-navigation-sse-pool-exhaustion.md` — the verified
  defect this fixes (6/6 HTTP/1.1 connections held by SSE; REST queued).
- `ui/src/features/dashboard/useSeries.ts` — the deferred "one EventSource per series,
  fanned to N cells" follow-up this closes.
- Session doc (when work starts): `sessions/bus/unified-event-stream-session.md`.
- Skill doc (on ship): `skills/event-stream/SKILL.md` — the subscribe surface is
  API-drivable (open the stream, subscribe a subject, observe frames); the implementing
  session writes it grounded in a live run.
