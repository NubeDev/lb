# Weather — compile-optional native feed extension (session)

- Date: 2026-07-09
- Scope: [`../../scope/weather/weather-feed-scope.md`](../../scope/weather/weather-feed-scope.md)
- Stage: post-S8 (extension surface — a sixth native reference extension)
- Status: **shipped (revised approach)** — a simpler **direct-in-host `weather.current` tool +
  dashboard widget** landed first (see "Shipped" below); the full extension build plan below is
  retained as the next increment. Original docs-only status superseded.

## Shipped (revised approach — direct-in-host `weather.current`)

Per a follow-up instruction ("ship the simpler version first, update the scope doc"), the first
increment is **not** the compile-optional native extension below. It is a `weather.current` MCP tool
built **directly into the host crate** (a host-native tool, alongside the other `HOST_NATIVE_PREFIXES`
verbs) plus one dashboard widget. No sidecar, no `lb-jobs` poll, no persistence, no cargo feature —
the smallest thing that puts live weather on a dashboard. The extension plan below remains the path
to the durable-poll + persist + admin-CRUD surface.

**What landed (working tree; all tests green, `cargo build`/`fmt`/`tsc` clean):**

Rust (`rust/crates/host/`):

| File | Change |
|---|---|
| `src/weather/current.rs` | `weather_current(input)` → `reqwest::get` Open-Meteo → `{location, temp_c, wind_kph, code, observed_ts}`. Base URL overridable via `LB_WEATHER_OPEN_METEO_BASE` (test stub seam — the one sanctioned external boundary, rule 9). |
| `src/weather/tool.rs` | `call_weather_tool` dispatcher; gates `mcp:weather.current:call`. |
| `src/weather/mod.rs` | Module exports. |
| `src/tool_call.rs` | `"weather."` added to `HOST_NATIVE_PREFIXES` + dispatch branch (generic prefix table — no `if ext=="weather"`, rule 10). |
| `src/lib.rs` | `mod weather;` + re-exports. |
| `src/system/catalog.rs` | Catalog entry for `weather.current`. |
| `src/dashboard/widget_catalog.json` | `weather` view (kind `read`, `data true`). |
| `src/authz/builtin_roles.rs` | `mcp:weather.current:call` in `VIEWER_CAPS`. |
| `Cargo.toml` | `reqwest.workspace = true`. |
| `tests/weather_tool_test.rs` | 4 tests: stub fetch, cap-deny, bad-input, ws-isolation. **All pass.** |

UI (`ui/src/`):

| File | Change |
|---|---|
| `features/dashboard/views/weather/WeatherPanel.tsx`, `wmoCode.ts` | shadcn `Card` widget + WMO code → label/icon. |
| `features/dashboard/views/WidgetView.tsx` | `case "weather"`. |
| `features/dashboard/views/weather/weather.gateway.test.tsx` | 4 gateway tests. **All pass.** |
| `lib/dashboard/dashboard.types.ts` | `"weather"` added to `View`. |
| `features/panel-builder/VizGallery.tsx`+`.test.tsx`, `VizPicker.tsx` | picker cards. |
| `features/panel-builder/options/registry.ts` | `weather` in `NO_FIELDCONFIG_VIEWS` (fixes the `optionLiveness` throw — no fieldConfig row for weather). |
| `features/panel-builder/options/registryRoundTrip.test.ts` | fieldConfig-less assertion. |
| `test/real-gateway.ts` | starts a local HTTP stub, passes `LB_WEATHER_OPEN_METEO_BASE` to the spawned test node (rule 9 — real node, real store, stub only for the true external). |

**Verified live** (2026-07-09): `POST /mcp/call {tool:"weather.current"}` on the running dev node
(pid 2592555) returns real data:
`{"location":"-27.47,153.02","temp_c":14.3,"wind_kph":5.3,"code":1,"observed_ts":"2026-07-09T10:00"}`.
Fresh `POST /login` mints a token containing `mcp:weather.current:call`.

## Goal

Set the implementation plan for the `weather` extension: a **compile-optional** (`weather` cargo
feature, off by default) native (Tier-2) extension over the **free, keyless** Open-Meteo feed,
driven by a **30-min durable `lb-jobs` poll + a Run-now verb**, optionally mirroring readings into
the series plane, with **one shadcn dashboard widget**. Ground every file against the shipped
`federation` extension so the next session writes code, not architecture.

## What changed (this session)

Docs only — the scope, the public stub, the topic index, and this plan. **No `.rs`/`.tsx` written**
(explicit instruction: commit NO code). The build plan below is the deliverable.

## Build plan — files, mirrored on `rust/extensions/federation/`

The federation extension is the exact template (native Tier-2, its own workspace member that builds
for the host target, speaks the `lb-supervisor` stdio wire, `net:*`-gated, one-trait-per-external).
Weather is the same shape, one provider narrower and behind a default-off feature.

**Crate `rust/extensions/weather/`** (new; a workspace member like `federation`/`echo-sidecar`):

| File | Responsibility (≤400 lines, one verb) |
|---|---|
| `extension.toml` | Manifest: `id="weather"`, `tier="native"`, `placement="either"`, `[native] exec="weather" restart="on-crash"`. `[capabilities] request = ["net:tls:api.open-meteo.com:443:connect"]` — **no `secret:*`** (keyless). `[[tools]]` for `weather.current`, `weather.location.add/list/remove`, `weather.poll.now`. |
| `Cargo.toml` | `[[bin]] name="weather"`. Deps: `lb-supervisor` (the wire, verbatim), `serde`/`serde_json`, `reqwest` (rustls, no openssl — keyless HTTPS), `tokio` (rt-multi-thread, io-std). `[features] default = []` — the crate compiles standalone; the **node-side** `weather` feature gates whether the host wires + supervises it (see below). |
| `build.sh` | `cargo build --release -p weather` → `rust/target/release/weather`. No feature juggling (no openssl toolchain — rustls). |
| `src/main.rs` | Child entry: read/write own stdio, dispatch the control-line verbs to the modules below. |
| `src/feed/open_meteo.rs` | The **one `Feed` trait, one file** — the "true external behind one trait" carve-out. `reqwest` GET `api.open-meteo.com/v1/forecast?latitude&longitude&current=temperature_2m,wind_speed_10m,weather_code`. Parse → `Reading { temp_c, wind_kph, code, observed_ts }`. Malformed/empty body → clean error, no panic. Backoff on 429/5xx. |
| `src/feed/mod.rs` | `trait Feed { async fn current(lat,lon) -> Result<Reading> }` + `Reading`. One kind today (open-meteo); a keyed provider would be a second file **with** a `secret:*` (Non-goal, not built). |
| `src/current.rs` | `weather.current {location?}` handler → resolve alias in caller ws (host-set) → `Feed::current` → `Reading`. |
| `src/location.rs` | `weather.location.add/list/remove` — CRUD over `weather_location:{ws}:{name}` via host `data.*` verbs (the extension holds no durable platform state — rule 4). |
| `src/poll.rs` | The **poll job body**: for each enabled location, `Feed::current`; if `store:true`, `ingest.write` the samples via the **native host-callback** (`ingest.write` — reference-extensions fix 1). Idempotent per tick (ingest dedup); coalesce concurrent run-now. |

**Node-side wiring (the compile-optional switch — mirror `external-agent`):**

- `rust/node/Cargo.toml`: a `weather` feature, **off by default**, that turns on the host wiring
  crate/module that registers + supervises the extension. A default build has **no** weather code
  path (rule 10 — core never learns the id; the wiring is generic ext-registration gated by the
  feature only at the node-assembly layer, exactly as `external-agent` is `#[cfg(feature=…)]`).
- `rust/crates/host/…` (or the node's ext-registration): `#[cfg(feature="weather")]` registers the
  extension's supervisor `Spec` + its verbs. **No** `if ext=="weather"` branch in a core mediation
  crate — the feature only decides *whether the generic registration runs*, never widens a
  chokepoint. Confirm against how `external-agent` is `#[cfg]`-gated so this stays the same seam.
- The **30-min schedule**: on first enabled location, enqueue/ensure a recurring `lb-jobs` record
  `kind="weather.poll"` with `run_at = now + 30min` (jobs-scope `run_at`); `weather.poll.now`
  enqueues the same `kind` immediately, returns the job id.

**Makefile — `make dev WEATHER=1` (the install story, mirrors `CE=1` + `EXTAGENT=1`):**

One flag drives the three halves. `WEATHER=1` is to weather what `CE=1` is to control-engine, plus
the `EXTAGENT=1 → --features` compile toggle. Copy those two blocks verbatim; the shape:

```makefile
# Weather (weather native extension). OFF by default. Turn on for `make dev` with:
#   make dev WEATHER=1
# When on: compiles the node `weather` feature, builds the weather sidecar, installs +
# supervises it at boot, and pre-approves its net:tls:api.open-meteo.com:443 connect.
# Keyless (Open-Meteo) — no API key, no secret to set (unlike EXTAGENT's ZAI_API_KEY).
ifeq ($(WEATHER),1)
NODE_FEATURES += weather              # (1) compile-optional half — MUST be += not ?= so it
WEATHER_ON    := 1                    #     combines with EXTAGENT=1 (external-agent uses ?=,
endif                                 #     first-wins; += lets both features co-compile)
WEATHER_ENV = $(if $(WEATHER_ON),LB_WEATHER_ENABLED=1,)   # (3) install + supervise at boot

.PHONY: weather                       # (2) build the sidecar (the `federation:` pattern)
weather:
	bash $(BE_DIR)/extensions/weather/build.sh
```

- `dev:` gains `$(if $(WEATHER_ON),weather,)` in its prereq list (beside
  `$(if $(CE_BASE),control-engine,)`), so the sidecar builds only when on.
- `$(WEATHER_ENV)` joins `$(FED_ENV) $(CE_ENV) $(DEVKIT_ENV)` on the node run line (line ~235).
- An echo line: `@echo "weather → $(if $(WEATHER_ON),Open-Meteo (keyless), pre-approved, <disabled>)"`.
- **No secret/PATH warning block** — keyless is the point; unlike the `NODE_FEATURES` external-agent
  warnings (`ZAI_API_KEY`/`interpreter`), `WEATHER=1` is zero-config.

Node-side boot: `LB_WEATHER_ENABLED=1` is the env the `#[cfg(feature="weather")]` wiring reads to
register the supervisor `Spec` + pre-approve `net:tls:api.open-meteo.com:443` (the `CE_BASE`/
`FED_ENDPOINTS` env-gate pattern — the feature compiles it in, the env turns it on at boot).

Resulting UX:

| Command | Result |
|---|---|
| `make dev` | no weather (default build — nothing weather-shaped compiled, rule 10) |
| `make dev WEATHER=1` | weather compiled + sidecar built + installed + Open-Meteo connect pre-approved |
| `make dev WEATHER=1 EXTAGENT=1` | both features co-compile (the `+=` requirement above) |

**Frontend `ui/…` (one widget over the federation-UI + Widget-Kit seam):**

- `ui/src/features/weather/AddLocationForm.tsx` — admin add/list/remove (the `AddDatasourceForm`
  pattern; `store` is a boolean toggle, off by default).
- `ui/…/weather-widget` federated widget — a shadcn `Card`: current temp/wind/condition +
  last-updated + a **Run now** button (`weather.poll.now` via the mount bridge value channel).
  When the location is persisted, a series line underneath streams `GET
  /series/weather.<loc>.temp_c/stream` (frames-in ctx). Reached only via `ext.list` discovery — no
  hardcoded route/nav/icon in the core shell (rule 10).

## Decisions & alternatives

- **Open-Meteo over any keyed feed** — the ask is "no API key". Keyless → zero `secret:*`, zero
  admin key-minting; the only approval is the `net:*` grant. A keyed provider is a documented future
  second `Feed` file, not smuggled in.
- **Compile-optional via a node-side cargo feature (off by default), the `external-agent` posture**
  — not a runtime `if`. The crate itself always compiles (it's an isolated binary); the **node**
  feature decides whether the host wires + supervises it, so a lean edge build is genuinely
  weather-free. This is the "optional to compile with, like the datasources docs" the ask meant.
- **`reqwest` + rustls, not openssl** — federation needs openssl only for its Postgres connector;
  a keyless HTTPS GET to Open-Meteo needs no C TLS toolchain. Keeps `build.sh` a one-liner.
- **Schedule = a durable `lb-jobs` `run_at` job, not `tokio::interval`** — survives restart, gets
  retries/backoff, workspace-scoped (jobs-scope §6.10). Rejected the in-process timer (loses the
  tick on restart, no resume).
- **Persist = the existing series/ingest plane via `ingest.write`, per-location `store` toggle off
  by default** — not a new table (rule 2). Reuses federation's mirror path exactly.

## Tests (to write next session — none run this session)

Mandatory categories from `testing-scope.md`, all against the **real** supervisor/store/caps/jobs;
**Open-Meteo is the one sanctioned fake-boundary** (a local HTTP stub serving a captured body behind
the `Feed` trait — §0), with one opt-in network-gated live test.

- **Capability-deny:** `weather.current` without `mcp:weather.current:call`; add/remove without the
  admin cap; **`net:*` deny** — no `net:tls:api.open-meteo.com:443` grant → fetch refused even with
  the binary compiled in (the headline reference-extension deny).
- **Workspace-isolation:** ws-B can't resolve/read a ws-A location; the poll callback `ws` is
  un-spoofable; ws-B's persisted series invisible to ws-A — across store + MCP + `net:*`.
- **Compile-optional (the ask):** a build **without** `--features weather` is green and exposes
  **none** of the verbs (feature-absent → verb-unknown); a build **with** it exposes them. Prove
  both configs — the headline test for this scope.
- **Schedule + run-now:** poll scheduled at 30-min `run_at` on first enabled location;
  `weather.poll.now` enqueues an immediate run returning a job id; a node restart between ticks
  preserves the schedule and does not double-fetch.
- **Persist toggle:** `store:true` → poll writes samples → `series.read`/SSE shows them;
  `store:false` → no series written; idempotent re-poll no double-write.
- **Feed decode:** captured body → `{temp_c, wind_kph, code, observed_ts}`; malformed → clean error.
- **Frontend (real gateway):** Weather admin page + widget (live `weather.current` + persisted
  series line + Run-now) over the bridge against a real spawned node (feature on) + the local stub.

## Debugging

- **"extension error: weather fetch failed: error sending request for url (…open-meteo…)"** on the
  live node, while all tests + a standalone `reqwest` probe returned 200 OK. **Root cause: a
  transient outbound-connectivity blip** at the time of the earlier test — not a code, cap, TLS-root,
  or sandbox issue. Ruled out each hypothesis with evidence: (1) the node's parent chain is
  `make dev` ← VSCode integrated terminal, **not** a Claude background task, so no harness sandbox;
  (2) a fresh process launched with the node's **exact** `/proc/PID/environ` reaches Open-Meteo
  (200 OK) — env/DNS/roots are fine; (3) the running binary is current (built 17:00, started 17:01,
  exe not `(deleted)`) — not stale; (4) re-running the identical live `mcp/call` **now succeeds** with
  real data. Note the reqwest TLS backend here is **`rustls-tls-native-roots`** (pulled in via
  `polars-io`/`object_store`), not webpki-roots as an earlier note assumed — but `SSL_CERT_FILE`
  resolves fine, so this was not the cause. No code change needed; logged as a debugging entry rather
  than chased as a regression. See [`../../debugging/weather/weather-fetch-error-sending-request.md`](../../debugging/weather/weather-fetch-error-sending-request.md).

## Public / scope updates

- Public stub created: [`../../public/weather/weather.md`](../../public/weather/weather.md) (TODO
  until ship).
- Topic added to [`../../scope/README.md`](../../scope/README.md).
- Scope open questions (metric set / schedule granularity / widget data mode / units) carry
  forward with recommendations — resolve during the code session.

## Skill docs

**Deferred to the code session** — the extension exposes an agent-/API-drivable surface
(`weather.current`, `weather.location.*`, `weather.poll.now`), so `skills/weather/SKILL.md` (a
runnable how-to grounded in a live run: add a location → poll now → read/see it) must be written
**when the verbs are live**, not before. Named here so the code session owns it (scope §6).
