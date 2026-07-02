# ROS scope — a driver extension for a fleet of ROS controllers

Status: scope (the ask). This is 100% an extension — all docs live in this folder (no repo-root `docs/` copy). Promotes to this folder's shipped notes once built. Target stage: **post-S8**
(rides the S8 data plane — `ingest`/`series` — and the S7 native tier + registry).

We want an **out-of-core extension** that manages many **ROS** controllers (Rubix/ROS REST
appliances) as first-class, capability-gated resources: connect to N boxes, browse each box's
**networks → devices → points** tree, CRUD every level, **poll** point present-values into the
platform's time-series at a rate the operator controls, and **write** setpoints back to a point's
priority array. It is a protocol bridge — exactly the shape the `ingest` scope reserves for "device"
concepts — so **not one line of ROS vocabulary enters a core crate**: the extension owns the
`rust-ros` REST client, the poller, and the driver models; the core sees only generic MCP verbs,
`ingest.write` samples, and outbox effects.

## Goals

- **Manage many ROS boxes.** A `ros` **connection** resource = one reachable ROS appliance
  (`base_url` + `External` token). Full CRUD; list/get across the fleet, all workspace-scoped.
- **The three-level tree, mirrored as resources.** `network`, `device`, `point` — each with
  `list|get|create|update|delete` following the `core/resource-verbs` grammar, always scoped by a
  parent (`ros_uuid` → `network_uuid` → `device_uuid`). Reads proxy the ROS REST API through the
  `rust-ros` client; writes proxy back.
- **A reusable poller.** One generic poll engine that, per enabled point, reads `present_value` on
  an interval and appends a `Sample` to a series via `ingest.write`. The engine is **series-shaped
  and ROS-agnostic in its core loop** (a `Poller<Source>` seam) so it can be reused by future
  drivers; only the ROS `Source` impl knows about `rust-ros`.
- **Three-level poll gating.** Polling is disableable at **connection**, **network**, **device**,
  and **point** level. The effective decision is the **AND** of the `enable` flags up the tree (a
  point polls only if point ∧ device ∧ network ∧ connection are all enabled) — reusing the `enable`
  field that already exists on every `rust-ros` model. No bespoke "polling on/off" verb: it is
  `update {enable:false}` at the chosen level (resource-verbs D "pause is update").
- **Point write (setpoint).** `point.write` sets a value at a priority slot (1–16) on a point's
  priority array — a **must-deliver** effect staged through the **outbox**, never raw pub/sub.
- **Live values in the UI.** The frontend reads `series.latest`/`series.read` for the live value +
  chart; it never polls `point.get` on a timer (state vs motion, rule 3).
- **A shadcn/Tailwind-v4 federated UI.** A self-contained extension page (fleet → connection →
  network → device → point drill-down) plus point-write and poll-toggle controls, co-located under
  the extension's `ui/`, loaded as a module-federation remote by the shell (like `fleet-monitor`).

## Non-goals

- **Not an IoT subsystem in core.** No "device"/"sensor"/"ROS" concept leaks into a core crate; this
  is a bridge extension, same posture as `github-bridge` (ingest scope, "must NOT turn Lazybones
  into an IoT system").
- **No new persistence layer.** Driver records live in SurrealDB under the workspace namespace;
  polled values live in the S8 `series` model. No SQLite/Postgres — the `rust-ros` client's `sqlx`
  dependency is **dropped** (it was a port artifact; the ROS box owns its own DB, we only speak REST).
- **No history backfill / rollups this slice.** We append live samples; ROS `histories` import and
  series rollups are a follow-up (`ros-histories`).
- **No ROS discovery/auto-provisioning.** Connections are created explicitly with a URL + token.
- **No VPN/system control.** `rust-ros` `system.rs`/`vpn` verbs are out of scope for slice 1
  (a read-only `ros.ping` health check is in; VPN control is deferred).

## Intent / approach

**Shape:** a native (Tier-2) sidecar extension — it owns long-lived HTTP connections and a poll
timer loop, so it needs an OS process with its own PID (the `mqtt`/`fleet-monitor` posture, not
WASM). It serves MCP tools for the CRUD verbs + `point.write` + `ros.ping`, and it runs the poller
as an internal task that calls the host's `ingest.write`. The federated shadcn UI is a second part
of the same extension folder.

**The reusable poller is the core idea.** A `Poller` owns a schedule and a set of *poll targets*;
each tick it asks a `Source` trait for the current value of each enabled target and hands the batch
to a `Sink` (which wraps `ingest.write`). ROS is one `Source` impl (`RosSource`, backed by
`rust-ros`). This keeps the loop, the enable-gating, the backoff, and the batching **driver-agnostic
and unit-testable without any ROS box**, while the ROS-specific REST calls sit behind the trait in
one file. Rejected: baking the poll loop into the ROS client directly — it would make the loop
untestable without a live box and unreusable for the next driver.

**Enable-gating is computed, not stored per-point-as-a-poll-flag.** The poller, each cycle, resolves
the target set by walking connection→network→device→point `enable` flags (cached from the store /
last tree fetch) and ANDing them. Toggling a network off therefore silences all its devices' points
with a single `network.update {enable:false}` — no fan-out write. This is the natural reading of the
existing `enable` field and avoids a second source of truth.

**The `rust-ros` client is the one allowed external fake seam.** A live ROS appliance is a *true
external we can't run in CI* (testing-scope §0). So ROS REST access goes behind **one trait**
(`RosApi`) in **one clearly-named file**, with the real `rust-ros`-backed impl and a single
`ros_fake.rs` test double that serves canned network/device/point trees and accepts writes. The
poller, the MCP handlers, and the UI-gateway path are all exercised **for real** against the fake
ROS box + the **real** store, bus, ingest, outbox, and gateway. No `*.fake.ts`, no re-implemented
host behavior.

## How it fits the core

- **Tenancy / isolation:** every driver record key is `ros:{ws}:…` / `network:{ws}:…` etc.; every
  polled series is `ros.{ws}.{ros}.{net}.{dev}.{point}`. A handler resolves the caller's workspace
  first, then the resource — a token for workspace A can never list, poll, or write workspace B's
  ROS boxes. Mandatory isolation test below.
- **Capabilities:** each verb is its own MCP tool + capability, one file per verb (FILE-LAYOUT):
  `mcp:ros.{list,get,create,update,delete,ping}:call`,
  `mcp:network.{list,get,create,update,delete}:call`, same for `device`, `point`, plus
  `mcp:point.write:call`. The sidecar's own poll task calls `mcp:ingest.write:call` and stages point
  writes through the outbox under its granted set. Requested in `extension.toml`; the live grant is
  `requested ∩ admin-approved`. **Deny path:** a caller without `mcp:point.write:call` is refused
  before any REST call leaves the node; a prefix-scoped `ros.list` grant cannot see boxes outside its
  prefix.
- **Placement:** `either`. The sidecar can run on the edge node nearest the ROS boxes (LAN latency)
  or in cloud — config/role, no `if cloud`. In practice you place it where it can reach the
  appliances; that is a scheduling fact, not a code branch.
- **MCP surface** (API shape, §6.1):
  - **CRUD** — `ros|network|device|point . create|update|delete`. Real write verbs, each capped.
  - **Get / list** — `ros|network|device|point . get|list`, workspace-scoped, parent-filtered,
    keyset-paged `{items, next_cursor}` (resource-verbs envelope). Proxies the ROS REST list calls.
  - **Live feed** — **no new watch on the driver tree** (config changes are low-rate; use `get`).
    Live *values* ride the S8 `series.latest`/`series.read` + the gateway SSE the `series.watch`
    already provides — the poller is the producer, `series` is the feed. This is the correct
    state-vs-motion split: config is a record read, values are a stream.
  - **Batch** — `point.write` accepts a **single** point + priority this slice (bounded, fast). A
    bulk multi-point write is deferred; when added it is a **job** (resource-verbs / jobs), not a
    blocking loop, because it fans out N must-deliver REST writes.
  - **Runnable trait** — the poller is a runnable resource: `ros.start|stop|status` arm/disarm the
    poll task for a connection (start = begin polling its enabled tree; stop = park the timer;
    status = last-ok/last-fail + sample count). `restart` = stop+start.
- **Data (SurrealDB):** driver config records (`ros`, `network`, `device`, `point` shadows — the
  minimum identity + `enable` + poll-rate needed to schedule polling and render the tree without a
  round-trip); the ROS box remains the **authority** for the full record (we proxy on `get`). Polled
  values are **series** (S8), not driver records — motion, not state.
- **Bus (Zenoh):** point writes are **must-deliver** → staged as **outbox** effects (the write has
  to reach the ROS box; a dropped setpoint is a safety bug). Poll samples are high-volume motion →
  they go through `ingest.write` (the read-side buffer), never raw pub/sub.
- **Sync / authority:** the ROS appliance is authoritative for live point state; the platform holds
  a config shadow + a time-series projection. Offline: if a box is unreachable the poller records
  `last_fail` and backs off; the outbox retries pending writes until the box returns (durability).
- **Secrets:** the `External` API token per connection is secret material — stored via `lb-secrets`
  under `secret:ros/{ros_uuid}/token`, mediated by the host; the token never appears in a driver
  record, a log line, or the UI. `create`/`update` take the token; `get`/`list` never return it.

## Example flow

1. An admin calls **`ros.create {name, base_url, token}`** → the sidecar stores a `ros` config
   record (`enable:true`), stashes the token via `lb-secrets`, and returns the `ros_uuid`.
2. The UI opens the connection page → **`network.list {ros_uuid}`** → the sidecar's `RosApi` fetches
   `/api/networks?with_devices=…` from the box and returns the paged tree. Drill into a device →
   **`point.list {device_uuid}`**.
3. The operator arms polling: **`ros.start {ros_uuid}`**. The poller resolves the target set
   (points where point ∧ device ∧ network ∧ connection `enable` are all true) and, every interval,
   reads `present_value` per target via `RosApi` and calls **`ingest.write`** with a `Sample[]` on
   series `ros.{ws}.{ros}.{net}.{dev}.{point}`.
4. A browser widget shows the live value via **`series.latest("ros.…")`** and the trend via
   **`series.read("ros.…", last_1h)`** — capability-checked, workspace-first, no timer polling of
   `point.get`.
5. The operator disables a whole network: **`network.update {network_uuid, enable:false}`**. Next
   poll cycle, every point under it drops out of the target set — one write, fleet-wide effect.
6. The operator writes a setpoint: **`point.write {point_uuid, priority: 8, value: 21.5}`** → the
   handler checks `mcp:point.write:call`, stages an **outbox** effect that PATCHes
   `/api/points/{uuid}/write` on the box; the outbox retries until acked. On success `last_write`
   updates; the next poll reflects the new present-value.

## Testing plan

Per `scope/testing/testing-scope.md`; exercised against the **real** store/bus/ingest/outbox/gateway
with a seeded fake ROS box behind the `RosApi` trait (the one allowed external fake, §0).

- **Capability deny (mandatory):** a caller without `mcp:point.write:call` is refused and **no REST
  write leaves the node**; a reader without `mcp:series.read:call` cannot see polled values; a
  `ros.list` grant scoped to a prefix cannot list boxes outside it.
- **Workspace isolation (mandatory):** workspace A's token cannot `ros.list`, poll, or `point.write`
  workspace B's connections; a series written by A's poller is invisible to B.
- **Poller (unit, no ROS box):** the `Poller`/`Source`/`Sink` loop with a stub `Source` — enable
  gating (point off / device off / network off / connection off each silences correctly and the AND
  is exact), interval scheduling, backoff on `Source` error, and batch shaping to `ingest.write`.
- **Enable-gating integration:** toggle each of the four levels via `*.update {enable:false}` and
  assert the target set (and the series that receive samples) changes accordingly.
- **Point write → outbox:** `point.write` stages an outbox effect (must-deliver), the fake box
  records the PATCH, and a box-unreachable case retries rather than dropping.
- **CRUD round-trip:** `ros.create` → `network.list`/`device.list`/`point.list` proxy the fake tree;
  `point.update` reflects; the token is never returned by `get`/`list`.
- **Runnable trait:** `ros.start`/`stop`/`status` arm/disarm the poll task; `status` reports
  last-ok/last-fail + sample count.
- **UI against a real spawned gateway** (`pnpm test:gateway`, no `*.fake.ts`): the federated page
  drills the tree, toggles poll enable, and issues a `point.write` through the real gateway to the
  real sidecar (fake ROS box behind the trait). Live value renders from `series.latest`.
- **Hot-reload:** swap the sidecar version with an in-flight poll task; no durable state lost (state
  is in the store/series/outbox, rule 4) — the poller re-arms from the config records.

## Risks & hard problems

- **Poll storm → write storm.** N boxes × M points × a fast rate is a firehose. Mitigated by routing
  every sample through `ingest.write` (the S8 buffer batches/dedups before committing) and by a
  per-connection concurrency cap (the `rust-ros` `enable_concurrency`/`concurrency_limit` fields).
  A naive per-sample store write is the failure mode to avoid.
- **Blocking client in an async task.** `rust-ros` is `reqwest::blocking`. The poller runs many
  concurrent reads; a blocking client on the async runtime will stall. **Decision:** switch the
  `rust-ros` client to async `reqwest` (allowed — "fine to update rust-ros"), or isolate it on a
  blocking thread-pool. Async is preferred; see open questions.
- **Setpoint safety.** A dropped or double-applied write is a physical-world bug. The outbox gives
  at-least-once delivery; point writes must be **idempotent** at the priority-slot level (writing
  slot 8 = 21.5 twice is the same as once) — which the ROS priority-array model already is.
- **Effective-enable cache staleness.** The poller ANDs cached `enable` flags; a stale cache could
  keep polling a just-disabled branch for one cycle. Bounded (one interval) and acceptable; the
  `update` handler invalidates the cached branch to tighten it.
- **Token leakage.** Easy to accidentally log the `External` token or return it from `get`. Enforced
  by never storing it in the driver record (only in `lb-secrets`) and a test asserting it is absent
  from `get`/`list`/logs.

## Open questions

- **Async vs blocking `rust-ros`.** Port the client to async `reqwest` (recommended, cleaner poller)
  or wrap the blocking client on `spawn_blocking`? Leaning async — resolve in the first session.
- **Config shadow depth.** Do we persist a full `network/device/point` shadow, or only
  `{uuid, name, enable, poll_rate, parent}` and proxy the rest on `get`? Leaning minimal shadow (just
  enough to schedule + render the tree) with the box as authority.
- **Poll rate source.** Use the ROS `poll_rate`/`fast|normal|slow_poll_rate` fields as the interval,
  or a platform-side per-point interval on the shadow? Leaning: platform-side interval on the shadow,
  seeded from the ROS field, so the operator can override without touching the box.
- **Series naming + labels.** Confirm `ros.{ws}.{ros}.{net}.{dev}.{point}` as the series id and which
  ROS fields (`unit`, `data_type`, `object_type`, tags/meta_tags) become series **labels** (the
  tag-graph is the query layer over series — ingest scope).
- **Priority-array write ergonomics.** Expose the full 16-slot `Priority` on `point.write`, or a
  simplified `{slot, value}` (with a `release`/null to clear)? Leaning `{slot, value|null}`.

## Related

- `../ingest/ingest-scope.md` — the `series` model + `ingest.write`/`series.read`/`series.latest`
  this rides; the "bridge, not core IoT" posture this extension embodies.
- `../core/resource-verbs-scope.md` — the `list|get|create|update|delete` + runnable
  `start|stop|status|restart|logs` grammar every verb here conforms to.
- `../extensions/extensions-scope.md` — the `extension.toml` manifest contract.
- `../extensions/native-tier-scope.md` — the Tier-2 native sidecar posture (own PID, supervised).
- `../extensions/ui-federation-scope.md` — the module-federated shadcn page (the `fleet-monitor`
  pattern this UI copies).
- `../outbox/…` / README §6.10 — must-deliver point writes stage outbox effects.
- `../secrets/…` — the `External` token mediated by `lb-secrets`.
- `../testing/testing-scope.md` §0 — the one-trait external-fake rule the `RosApi` seam follows.
- `rust/extensions/mqtt/extension.toml`, `rust/extensions/fleet-monitor/` — the reference sidecar +
  federated-UI extensions this one is modeled on.
- `/home/user/code/rust/rust-ros` — the REST client to vendor into `rust/extensions/ros/` (drop
  `sqlx`; port to async).
- `docs/skills/SKILL.md` (co-located) — the drivable how-to the implementing session must write.
