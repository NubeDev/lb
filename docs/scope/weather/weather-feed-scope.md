# Weather scope — `weather`, a compile-optional native extension over a free, keyless feed

Status: scope (the ask). Promotes to `public/weather/weather.md` once shipped. Topic: `weather`.

> **Increment shipped 2026-07-09 — simpler direct-in-host `weather.current` first.** A follow-up
> instruction was to ship the smallest useful slice before the full extension below. That slice is a
> `weather.current` MCP tool built **directly into the host crate** (a host-native verb alongside the
> other `HOST_NATIVE_PREFIXES` entries, gated by `mcp:weather.current:call`) plus **one dashboard
> widget** — **no** sidecar, **no** `lb-jobs` 30-min poll, **no** series persistence, **no** cargo
> feature, **no** location CRUD. Open-Meteo is still reached keyless behind the one test-overridable
> base-URL seam (`LB_WEATHER_OPEN_METEO_BASE`). It is verified live (a real dashboard call returns real
> conditions). The full compile-optional extension specified in the rest of this doc — durable poll +
> run-now + persist toggle + admin-CRUD + `net:*` pre-approval — remains the next increment; nothing
> below is retracted, it is deferred. Session:
> [`../../sessions/weather/weather-feed-session.md`](../../sessions/weather/weather-feed-session.md).

We want a workspace to **pull current weather for one or more locations from a free public feed and see
it on a dashboard**, on a **30-minute schedule with a "Run now" button**, optionally **persisting each
reading into the platform series plane** — all **without an API key** and **without adding a hard
dependency to the node**. The answer is a **native (Tier-2) `weather` extension** that is
**compile-time optional** (a `weather` cargo feature, off by default — the same posture as
`external-agent` and the federation sidecar), embeds an [Open-Meteo](https://open-meteo.com) client
behind one `Feed` trait, is driven by a **durable scheduled `lb-jobs` job** (`run_at` every 30 min +
a run-now enqueue verb), and ships **one shadcn dashboard widget** over the existing federation-UI
seam. SurrealDB stays the one datastore; Open-Meteo is a **federated source reached only through the
gated extension**, never a second authority and never wired into core.

> Read with: `../extensions/reference-extensions-scope.md` (the native-extension doctrine + the `net:*`
> family — this is a sixth reference extension in that mould), `../extensions/native-tier-scope.md` (the
> Tier-2 supervisor), `../datasources/datasources-scope.md` (the federate-vs-mirror pattern this reuses),
> `../jobs/jobs-scope.md` (the scheduled/`run_at` durable job + run-now enqueue), `../ingest/ingest-scope.md`
> (the mirror target — `ingest.write` into the series plane), `../frontend/widget-kit-scope.md` +
> `../extensions/ui-federation-scope.md` (the shadcn dashboard widget seam), README §3 (rules 2/5/6),
> §6.3 (two tiers), §6.9/§6.10 (jobs/outbox).

---

## Why free + keyless matters (the constraint that picks the feed)

The ask is explicit: **no API key**. That rules out OpenWeatherMap, Tomorrow.io, WeatherAPI, and the
rest of the signup-and-token feeds — a key is a secret the admin must mint, store, rotate, and that
would force `lb-secrets` mediation into what should be a zero-config demo-grade extension.

**Chosen feed: Open-Meteo** (`https://api.open-meteo.com/v1/forecast`). No signup, no key, no credit
card; free for non-commercial use with a generous rate limit; returns current + hourly + daily by
`latitude`/`longitude`. It fits "just works after install" — the only thing the admin approves is the
`net:*` grant to the one host. If a paid/keyed provider is ever wanted, it slots in behind the same
`Feed` trait as a second file **with** a `secret:*` (a deliberate future, not smuggled in — see
Non-goals).

## Doctrine: SurrealDB is authority; Open-Meteo is a federated source (rule 2 holds)

Same split as `datasources-scope.md`, one provider narrower:

1. **SurrealDB = the one datastore + authority.** Location config, the job record, and any persisted
   readings live in SurrealDB, workspace-walled. Open-Meteo is never registered as a store or a sync
   peer.
2. **Open-Meteo = a federated source**, owned by the `weather` extension, reached only through the
   extension's outbound HTTP behind the `Feed` trait, `net:*`-gated, workspace-pinned. It is a source,
   never a node's persistence layer.

## Goals

- A **native (Tier-2) `weather` extension** (`rust/extensions/weather/`) that **embeds** an Open-Meteo
  HTTP client behind **one `Feed` trait, one file** (`feed/open_meteo.rs`) — the sanctioned "true
  external behind one trait" carve-out (CLAUDE §9).
- **Compile-time optional.** A `weather` cargo feature, **off by default**, gates the whole extension
  (crate + its native role wiring), mirroring `external-agent` (feature off by default) and the
  federation sidecar's opt-in build. A node built without the feature has **no idea weather exists**
  (rule 10); a node built with it exposes the verbs below only where the extension is installed +
  approved.
- **Location config as a workspace record** — `weather_location:{ws}:{name}` → `{label, lat, lon,
  enabled}`, with admin CRUD verbs (`weather.location.add`/`list`/`remove`), so "which places" is data,
  never hardcoded.
- **One read verb, `weather.current {location?}`** → `{location, temp_c, wind_kph, code, observed_ts}`
  — workspace-pinned by the host (the caller names a **registered** location alias, never a raw
  lat/lon into another tenant), `caps`-gated (`mcp:weather.current:call`). Live fetch, bounded result.
- **A durable scheduled poll on the job service.** A **`lb-jobs` job runs every 30 minutes**
  (`run_at`/cron, per jobs-scope) that fetches each enabled location's current reading; a **run-now**
  verb (`weather.poll.now {}`) enqueues the same job immediately (the "Run now" button), returning a
  **job id** the UI watches. The poll never blocks a tool handler (§6.1).
- **Optional persistence into the series plane.** When a location is configured `store: true`, the poll
  job `ingest.write`s each reading as samples (`weather.<location>.temp_c`, `.wind_kph`) into the
  existing **series/ingest** plane — so history, offline read, and joins-with-platform-data come for
  free, and the dashboard streams via the existing `GET /series/{s}/stream` SSE. Persistence is a
  per-location toggle, **off by default** (the "option to store the data in the db" ask, made explicit).
- **One shadcn dashboard widget** (`ui/weather-widget`) over the federation-UI seam — a shadcn `Card`
  showing current temp/wind/condition + last-updated + a **Run now** button, reading either the live
  `weather.current` verb or (when persisted) the location's series. Built on the Widget Kit contract
  (`frames-in` ctx for the series case; the value/mount bridge for the button).

## Non-goals

- **No API key, no keyed provider in v1.** Open-Meteo only. A keyed provider is a future second `Feed`
  file **with** a `secret:*` grant — named here so it isn't smuggled in, not built now.
- **Not a second platform datastore** (rule 2): Open-Meteo is an external federated source behind the
  `Feed` trait; persisted readings land in the **existing** series plane as ingest, not a new table/DB.
- **No forecast/alerting engine.** v1 is *current conditions* + persist + show. Multi-day forecast,
  threshold alerts, and "notify me when it rains" are a rules-engine consumer (`rules-engine-scope.md`
  reads `weather.current`), out of scope here.
- **No sub-30-min polling / streaming.** The feed is polled on the 30-min schedule (Open-Meteo updates
  no faster and the free tier is rate-limited); there is no live socket. "Live" on the widget is the
  series SSE over persisted samples, not a push from Open-Meteo.
- **Core never learns "weather" exists.** No core crate or core UI shell branches on the id (rule 10);
  the extension is reached only through generic MCP resolution + the federation-UI `ext.list`.

## Intent / approach

**A sixth reference extension in the `reference-extensions-scope.md` mould, made compile-optional.** It
owns an outbound socket (to Open-Meteo), so it is **native Tier-2** — not a sandboxed wasm guest. It
uses the same platform fixes the reference set defined: the `net:*` family (enforced pre-connect) and
the native host-callback (so the poll job can call `ingest.write`). It adds **no new ABI** — it is a
consumer of the shipped seams.

**Compile-optional via a cargo feature, off by default — the `external-agent` lesson.** The user asked
for "optional to compile with, like the datasources docs." The datasources/federation sidecar and the
`external-agent` runtime are both **compile-time opt-in**: the crate and its role wiring sit behind a
cargo feature that is **off by default**, so a lean edge node pays nothing for a source it will never
use. The `weather` feature follows that exact posture: `--features weather` (or a `weather` entry in the
node's feature set) compiles the extension in; a default build omits it entirely. This is the "optional
to compile" the ask means — a build-time switch, not a runtime `if`.

**Federate vs mirror — both, per the datasources pattern.** *Federate* = `weather.current` reads
Open-Meteo live (fresh, ad-hoc, the widget's default). *Mirror* = the 30-min poll job `ingest.write`s
readings into the series plane when the location opts into `store: true` (history, offline, dashboards,
joins). Same extension owns the HTTP for both; the difference is query-through vs copy-in — exactly the
`0003` external-warehouse pattern, one provider narrower.

**The schedule is a durable job, not a `tokio::interval`.** Per jobs-scope, a periodic effect is a
`run_at`-scheduled `lb-jobs` record so it **survives a node restart**, gets retries/backoff for free,
and is workspace-scoped. The 30-min cadence is the job's `run_at`; the "Run now" button enqueues the
same `kind` immediately. A raw in-process timer would lose the tick across a restart and has no resume —
the jobs-scope §6.10 rule ("a batch/periodic effect that can run long MUST be a job").

**Rejected — a weather crate in core.** Tempting (it's a small HTTP call), but it owns an outbound
socket + a third-party feed contract, which belong in a supervised, admin-approved, `net:*`-gated
process — not the symmetric node binary every edge device runs (rule 1). And baking a specific feed into
core would violate rule 10. Core stays lean; weather is an installable, compile-optional extension.

**Rejected — WASM Tier-1.** A wasm guest can't open the socket; the fetch must be native (same reason
the five reference extensions are native). The only pure-transform slice (parsing Open-Meteo JSON →
our shape) is small and lives inside the native crate, not a separate wasm module.

## How it fits the core

- **Tenancy / isolation:** `weather_location:{ws}:{name}` and the poll `job:{id}` carry `ws`;
  `weather.current`'s `{location}` resolves only within the caller's workspace (host-set, un-spoofable).
  ws-B can neither name nor read a ws-A location; a mirror write's callback `ws` is host-set, never
  sidecar-supplied. Mandatory isolation test across store + MCP + the `net:*` boundary.
- **Capabilities:** `weather.current` gated `mcp:weather.current:call`; `weather.location.add`/`remove`
  admin-only; `weather.poll.now` gated `mcp:weather.poll.now:call`. At **connect time** the supervisor
  enforces `net:tls:api.open-meteo.com:443` (`requested ∩ admin_approved`) — no grant, no fetch, opaque,
  even with the binary compiled in. The deny path is the headline (port the reference-extension deny
  test).
- **Placement:** `either`, by config — but only where the extension is **compiled in** (`weather`
  feature) **and** installed/approved. Symmetric: the binary/feature is the same on edge and cloud;
  *whether* it's built and *which* host it may reach are config (the feature flag + the grant). No
  `if cloud`.
- **MCP surface (§6.1 — judged):**
  - **Read (the core add):** `weather.current {location?}` → one reading. Live, workspace-pinned,
    bounded (one location, or all enabled if omitted — a small bounded set, not a firehose).
  - **CRUD (admin):** `weather.location.add {name, label, lat, lon, store}`, `weather.location.remove
    {name}` — each its own verb + cap.
  - **Get / list:** `weather.location.list {}` (registered locations, no secrets — there are none).
  - **Live feed:** N/A from the feed itself (it's a 30-min poll, not a stream). A persisted location's
    history streams over the **existing `GET /series/{s}/stream` SSE** — no new watch verb (the poll
    writes series, the widget streams them).
  - **Batch → a job:** the **30-min poll is a scheduled `lb-jobs` job** (all enabled locations per tick);
    `weather.poll.now {}` enqueues it on demand and returns a **job id**. Progress is the job's feed.
- **Data (SurrealDB):** `weather_location:{ws}:{name}` (config) + the poll `job:{id}` are the only
  platform records — workspace-walled, the one datastore. Persisted readings land in the **existing
  series** plane via `ingest.write` (they join the one datastore as ingest, not a new table). Live
  readings are the extension's, behind MCP, never platform state.
- **Bus (Zenoh):** none directly from a live read. A mirror write's `ingest.write` publishes series
  motion (`publish_sample`) so the widget updates live; a must-deliver effect (none here beyond ingest)
  would use the outbox.
- **Sync / authority:** SurrealDB stays source of truth on every node (rule 2). A live `weather.current`
  is node-local; the poll job is authoritative on its hosting node and **resumes** on restart (the
  `lb-jobs` `run_at` + idempotent-step model). Open-Meteo is never a sync peer.
- **Secrets:** **none** — the whole point of the keyless feed. If a keyed provider is added later, its
  key is `secret:weather/{provider}` behind the same `Feed` trait, mediated exactly as the federation
  DSN is (named in Non-goals, not built).
- **SDK/WIT impact:** the **native host-callback** (the poll job calling `ingest.write`) is the shipped
  child→host boundary (reference-extensions fix 1) — this extension is a consumer of it, not a new ABI.
  `net:*` is the shipped auth-caps grammar. **No stable-boundary change.**

## Example flow

A workspace admin adds their city and watches it on a dashboard, no key anywhere.

1. **Build + install in one command — `make dev WEATHER=1`.** This is the whole install story (it
   mirrors `make dev CE=1` for control-engine + `EXTAGENT=1` for the compile feature): the flag (a)
   compiles the node `weather` feature in, (b) builds the `weather` sidecar via its `build.sh`, and
   (c) installs + supervises it at boot with `net:tls:api.open-meteo.com:443` **pre-approved** — so
   it works on first boot. A plain `make dev` compiles **no** weather code (a lean node pays nothing;
   rule 10). **No secret / no `ZAI_API_KEY`-style step — Open-Meteo is keyless.**
2. **(Already approved by step 1.)** In production the admin installs the extension and approves
   `net:tls:api.open-meteo.com:443` at the install dialog (`granted = requested ∩ approved`); the
   `make dev WEATHER=1` path pre-approves it for the dev workspace. No secret prompt either way —
   there's no key.
3. **Configure a location.** Admin → Weather → Add → `label:"Sydney", lat:-33.87, lon:151.21,
   store: true`. Writes `weather_location:acme:sydney`.
4. **Schedule.** Adding the first enabled location ensures the **30-min poll job** is scheduled
   (`run_at` = now + 30 min, recurring). The supervisor holds the sidecar; at fetch time it checks the
   `net:*` grant, then fetches Open-Meteo for each enabled location.
5. **Federate (live).** The dashboard widget calls `weather.current {location:"sydney"}`; the host
   authorizes `mcp:weather.current:call` workspace-first, resolves `sydney` in `acme`, the sidecar
   fetches Open-Meteo, returns `{temp_c:21.4, wind_kph:12, code:"partly_cloudy", observed_ts}`. The
   shadcn `Card` renders it.
6. **Mirror (persist).** Because `store: true`, the poll job also `ingest.write`s
   `weather.sydney.temp_c` / `.wind_kph` samples (via the native callback, `caller ∩ grant`, ws
   host-set) into the series plane. The widget's history line streams via `GET
   /series/weather.sydney.temp_c/stream` — fast, offline-capable.
7. **Run now.** The admin clicks **Run now** on the widget → `weather.poll.now {}` enqueues the poll job
   immediately, returns a job id; the widget shows "updating…" off the job feed, then the fresh reading.
8. **Deny path:** installed without `net:tls:api.open-meteo.com:443` → step 4/5 refuses the fetch
   (opaque, sidecar degraded). A ws-B caller naming `location:"sydney"` resolves nothing in ws-B →
   denied. `weather.current` without `mcp:weather.current:call` → denied.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`) — the gate. **No mocks** for our own stack: the
**real supervisor**, real store, real caps, the real `net:*` enforcement, a real `lb-jobs` queue for the
poll. **Open-Meteo is the one sanctioned fake-boundary** (a true external HTTP API — §0): tests run the
`Feed` trait against a **real spawned local HTTP stub** serving a captured Open-Meteo JSON body (behind
the one `Feed` trait, one file), **not** an in-process re-implementation of the extension. A single
opt-in "live" test may hit the real endpoint (network-gated, off by default in CI).

- **Capability-deny (§2.1):** `weather.current` denied without `mcp:weather.current:call`;
  `weather.location.add`/`remove` denied without the admin cap; **`net:*` deny** — no
  `net:tls:api.open-meteo.com:443` grant → fetch refused even with the binary compiled in (the headline
  reference-extension deny).
- **Workspace-isolation (§2.2):** ws-B cannot resolve/read a ws-A location; a poll job's callback `ws`
  is un-spoofable; ws-B's persisted series are invisible to ws-A — across store + MCP + `net:*`.
- **Compile-optional (the ask):** a build **without** `--features weather` compiles green and exposes
  **none** of the weather verbs / no `weather_location` handling (a "feature absent → verb unknown"
  assertion); a build **with** the feature exposes them. Prove both build configurations.
- **Schedule + run-now:** the poll job is scheduled at 30-min `run_at` on first enabled location;
  `weather.poll.now` enqueues an immediate run returning a job id; a **node restart** between ticks
  **preserves the schedule** (jobs `run_at` durability) and does not double-fetch.
- **Persist toggle:** `store:true` location → poll writes samples → `series.read`/SSE shows them;
  `store:false` → the same poll fetches but writes **no** series (the toggle honored). Idempotent
  re-poll does not double-write (ingest dedup).
- **Feed decode:** the captured Open-Meteo body parses into our `{temp_c, wind_kph, code, observed_ts}`
  shape (unit + field mapping); a malformed/empty body yields a clean error, not a panic.
- **Frontend (real gateway):** the Weather admin page (`add`/`list`/`remove`) + the dashboard widget
  (live `weather.current` and, for a persisted location, the series line + **Run now** button) over the
  bridge (`*.gateway.test.tsx`) — against a **real spawned node** with the feature compiled in + the
  local feed stub.

## Risks & hard problems

- **The compile-optional wiring is the real trap, not the HTTP.** Getting the cargo feature to gate the
  crate **and** its native-role registration **and** the UI discovery cleanly — so a default build is
  genuinely weather-free (rule 10) and a `--features weather` build is fully wired — is the part most
  likely to be underestimated. Mirror `external-agent`'s feature plumbing exactly; test both configs.
- **Open-Meteo rate limits / outages.** The free tier is rate-limited and best-effort. The poll must
  back off on 429/5xx (jobs retry/backoff), and a live `weather.current` failure must be a clean tool
  error surfaced in the widget, never a sidecar crash. The 30-min cadence keeps well under the limit for
  a handful of locations; a workspace with hundreds of locations is a documented ceiling, not a v1 goal.
- **Series naming collisions.** `weather.<location>.<metric>` must be workspace-scoped in the series
  plane like every other series — the location name is a slug the add verb validates, so two locations
  can't clobber each other and two workspaces stay walled.
- **"Run now" spam.** The run-now verb must be idempotent-ish (coalesce if a poll is already running for
  the workspace) so a button masher doesn't queue N redundant fetches — a small guard on the job kind.

## Open questions

1. **Series metric set:** ship `temp_c` + `wind_kph` in v1, or also humidity/precip? *Recommendation:*
   temp + wind for v1 (smallest useful widget), the `Feed` shape carries the rest for a later toggle.
2. **Schedule granularity:** fixed 30 min (the ask), or a per-workspace `interval` on the location
   config? *Recommendation:* ship fixed 30 min as the default `run_at`; expose `interval_min` as a
   nullable config axis only if a caller asks — don't pre-build the knob.
3. **Widget data mode:** should the widget default to the **live** `weather.current` call or the
   **persisted** series when `store:true`? *Recommendation:* live for freshness, with the series line
   shown underneath when persistence is on (both, not either).
4. **Units:** Open-Meteo returns metric by default — do we expose an imperial toggle? *Recommendation:*
   store metric (canonical), format via the one viz field-config bridge (the `format.ts` unit path),
   not a second fetch.

## Related

- `../extensions/reference-extensions-scope.md` — the native-extension doctrine, the `net:*` family, the
  native host-callback this consumes (a sixth reference extension in that mould).
- `../extensions/native-tier-scope.md` — the Tier-2 supervisor that holds the sidecar.
- `../datasources/datasources-scope.md` — the federate-vs-mirror pattern reused one provider narrower;
  `../datasources/sqlite-datasource-demo-scope.md` — the compile-in-a-source + widget precedent.
- `../external-agent/` — the **compile-time-optional cargo-feature** posture (feature off by default)
  this mirrors for "optional to compile with".
- `../jobs/jobs-scope.md` — the `run_at`-scheduled durable poll + the run-now enqueue (§6.10 batch/
  periodic-as-job rule).
- `../ingest/ingest-scope.md` — the `ingest.write` mirror target (the series plane); the persisted
  readings' SSE (`GET /series/{s}/stream`).
- `../frontend/widget-kit-scope.md` + `../extensions/ui-federation-scope.md` — the shadcn dashboard
  widget seam (`frames-in` ctx + the mount/value bridge for the Run-now button).
- `../rules/rules-engine-scope.md` — a future rule consumer of `weather.current` (alerting), out of
  scope here.
- README `§3` (rules 2/5/6/10), `§6.3` (two tiers), `§6.9`/`§6.10` (jobs/outbox); vision
  `../../vision/0003-iot-dashboard.md` (the external-source doctrine).
- Feed: [Open-Meteo](https://open-meteo.com) — free, **no API key**, no signup (the constraint that
  picked it).
- Public: `../../public/weather/weather.md` (TODO stub until ship).
