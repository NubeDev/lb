# Ingest scope — a generic buffered read/write surface for high-volume external data

Status: scope (the ask). Promotes to `public/ingest/` once shipped. Target stage: **S9**
(platform maturity, after the S7 registry + native tier; STAGES.md needs an S8/S9 row added
when this is scheduled).

We want the framework to robustly absorb **high-volume, high-frequency external data**
(a sensor stream, an app's metrics, a fleet of edge nodes reporting state) and to let a
principal **read and write data points** through the same capability-gated MCP contract as
everything else. The new piece is a **cloud-side ingest buffer**: a durable landing zone that
accepts bursts, applies backpressure, batches, dedups, and *then* commits to time-series state —
so a firehose of motion never becomes a per-sample write storm. This is the **read-side analog of
the outbox** (the outbox guarantees must-deliver effects *out*; ingest absorbs high-volume data
*in*).

**The hard constraint: this must NOT turn Lazybones into an IoT system.** A "device" is just a
**principal on a node** (README §3.1 — symmetric nodes; a Raspberry Pi is a full node, not a thin
sensor). The surface is a generic `series` of timestamped values; IoT is one *caller* of it, never
a concept in the core. If a word like "device", "sensor", "firmware", or "MQTT" appears in a core
crate, the scope has failed — those belong in **out-of-core extensions** (protocol bridges), exactly
as `github-bridge` lives outside the host.

## Goals

- A **canonical `Sample` envelope** — `{ series, ts, seq, payload, labels, qos }` — that every producer,
  internal or external, normalizes to. Generic and domain-free. `payload` is **any SurrealDB-typed
  value** (scalar, nested object, array, bytes, or a file-bucket reference for large/binary) — the
  buffer is **payload-agnostic**; telemetry, structured events, documents, and binary frames all flow
  through the same path. Aggregation (numeric scalars) is a property a series *opts into*, never a
  requirement the buffer imposes.
- A **generic `series` data model** in SurrealDB using the **time-series model the design already
  reserves** (record-ID ranges + table views for rollups, README §6.1; labels = the tag service,
  README §6.11). One datastore, no new persistence layer.
- **Heterogeneous batches.** `ingest.write` takes a `Sample[]` where each element names its own series
  and carries its own typed payload — so a single call can push a scalar metric, a structured event, a
  string, and a binary-by-reference frame **at the same time**. The buffer never branches on type; the
  commit step picks the storage shape per series. (Within one series, payloads should stay type-stable
  for predictable reads/rollups — allowed to vary, not recommended.)
- **The tag-graph as the uniform query layer over non-uniform payloads.** With mixed payload shapes
  there is no common schema to query by — so discovery happens through the **tag/graph** (README §6.11),
  not the payload. Series and samples are tagged `key:value` (`host:pi-7`, `region:eu`, `kind:event`,
  `unit:celsius`) as **graph edges**, giving faceted/indexed/full-text search AND relationship
  traversal (series → producer principal, → workspace, → related doc/job). This is what makes a store
  of heterogeneous data coherent and navigable; it is the same multi-model SurrealDB (rule #2), not a
  second system.
- An **ingest buffer** (cloud-side, but a *role* any node can run): accept → backpressure/batch/dedup
  → commit. Decouples *acceptance* from *durable commit* so bursts don't OOM or write-storm.
- **Symmetric read/write MCP verbs**: `ingest.write` (append samples), `series.read` (range query),
  `series.latest` (the "shadow" — last value per series). Capability-gated, workspace-first.
- **Robustness primitives**: bounded buffer + explicit **overflow policy**, **idempotent commit** on
  `(series, seq)`, **per-workspace/principal rate limits**, and **offline buffer + idempotent replay**
  on reconnect (reuse §6.8 sync, not a new mechanism).
- **State vs motion kept clean**: the live stream is **Zenoh motion**; the buffer and the committed
  series are **SurrealDB state**. The buffer is the seam between them.

## Non-goals (the defer-list — and the guardrails that keep this generic)

- **No device domain in core.** No device registry, provisioning, onboarding, firmware/OTA-of-devices,
  or device shadows-as-a-first-class-type. A device = a principal; its "config" = a series it reads.
- **No protocol adapters in core.** MQTT / CoAP / Modbus / OPC-UA / serial bridges are **extensions**
  (the `github-bridge` pattern: a pure-transform guest that normalizes raw bytes → `Sample[]`), shipped
  through the **registry**, never linked into the host. This scope defines the `Sample` contract they
  target; it does not build any of them.
- **No second datastore / TSDB.** SurrealDB only (rule #2). No InfluxDB/Timescale/Prometheus.
- **No new transport.** Zenoh + the existing SSE/HTTP gateway. No new broker.
- **No stream-processing / analytics engine.** Ingest, buffer, store, read. Rollups are SurrealDB
  table views, not a compute plane. (A SpiceAI analytics plane stays the separate S7/S8 option.)
- **No change to the SDK/WIT boundary.** Protocol-bridge extensions use the **existing** tool WIT
  (`normalize`-style). The core ingest verbs are host MCP tools like any other. Flag loudly if any
  design pressure pushes a WIT change — it shouldn't.

## Intent / approach

**The outbox, mirrored for inbound volume.** The outbox already solved "a discrete effect must leave
the system exactly once, durably, across a flaky link." Ingest is the same shape pointed the other
way: "a high-volume stream must enter the system without loss-by-overload, committed once per
`(series, seq)`." So the design reuses the outbox's proven discipline — a durable staging set,
idempotent commit, backoff/dead-letter — rather than inventing a queue.

**Three seams, each an existing primitive:**

1. **Motion in** — producers publish `Sample`s as Zenoh motion on `ws/{id}/series/{series}` (best-effort,
   high-rate). This is the cheap, real-time path; a live dashboard subscribes here directly.
2. **The ingest buffer** — a cloud-side subscriber (the new `lb-ingest` crate, run by a node holding the
   *ingest role*) drains the stream into a **durable staging table**, applies the overflow/rate/batch
   policy, and commits batches to the `series` tables. Accept-then-commit is what makes it robust:
   acceptance is O(1) and bounded; commit is batched and idempotent. A restart re-drains uncommitted
   staging — no loss, no double-commit.
3. **State out** — `series.read` / `series.latest` read the committed series (range queries over the
   record-ID-range layout; `latest` is the single newest record per series — the generic "shadow").

**Why a buffer at all, vs. writing each sample straight to the store?** A direct write couples producer
rate to store write rate: a burst either blocks the producer or overruns the store. The buffer
**decouples** them — it absorbs the burst at bounded cost and commits at the store's pace, with an
explicit policy for what happens when the buffer is full (drop-oldest for best-effort series;
dead-letter for must-deliver). This is the single most important robustness property, and it's why
"buffer the data in the cloud" is the right framing, not "write faster."

**Why generic `series`, not `device`/`metric`?** A `series` is just a named, workspace-scoped sequence
of timestamped values with labels. A temperature sensor, an app's request-latency, a node's free-memory,
a build's test-count — all are series. Keeping the core noun generic is what stops the platform from
becoming an IoT system: the IoT-ness lives entirely in *which* series a bridge extension creates and
*what* labels it attaches, never in the host.

**Rejected alternatives:**
- *Use the inbox for samples.* Rejected — the inbox is durable state per item (README §6.10); a
  firehose of samples would turn the store into a write-storm log, violating state-vs-motion. The inbox
  stays for discrete, triage-able events; samples are motion buffered into series state.
- *A dedicated time-series database.* Rejected — violates one-datastore (rule #2). SurrealDB's
  record-ID-range time-series model is already in the design for exactly this.
- *Write each sample directly, no buffer.* Rejected — couples producer rate to store rate, no
  backpressure, no overflow policy; the first burst is an outage.
- *Bake a `Device` type into core.* Rejected — it's the exact failure mode the ask warns against; a
  device is a principal, its config is a series.

## How it fits the core

- **Tenancy / isolation:** every series key is `ws/{id}/…`; the staging table, the series tables, and
  the buffer's in-memory index are all workspace-partitioned. A ws-B producer cannot write or read a
  ws-A series — the hard wall holds for the firehose path exactly as for channels.
- **Capabilities:** `mcp:ingest.write:call` gates writes; `mcp:series.read:call` gates reads
  (optionally narrowed by a series prefix in the grant, e.g. `…:call?series=metrics.*`). The deny path
  is opaque (`Denied`, no existence signal) — an un-granted producer leaks nothing about what series
  exist. Mandatory deny-test.
- **Placement:** the **ingest role** is `either` — typically the cloud/hub runs it, but a Raspberry Pi
  acting as a **LAN sub-hub** can run its own ingest buffer for its local peers and sync committed
  rollups upward. No `if cloud {…}`; the role selects whether the buffer subscriber is mounted, like the
  gateway and sync relay do today.
- **MCP surface:** `ingest.write(samples)` (a **heterogeneous `Sample[]`** — many series, many payload
  types, one call), `series.read(series, range)`, `series.latest(series)`, `series.list(prefix)`, and a
  tag-driven discovery verb `series.find(tags)` (faceted `key:value` query over the graph). Producers
  and the UI call them identically (MCP is the universal contract).
- **Tags / graph:** labels are **graph edges** (`series ->tagged-> tag:{key,value}`), the existing tag
  service (README §6.11, `scope/tags/`) reused as the time-series label model. This is the discovery
  and relationship layer over heterogeneous payloads — query by metadata/relationship, not by schema.
  Keep label cardinality bounded (a known risk below) — tags are for dimensions you filter by, not for
  high-cardinality values (those belong in the payload).
- **Data (SurrealDB):** a durable **`ingest_staging`** table (the buffer's backing — survives restart),
  and the **`series`** tables (record-ID ranges per series + rollup table views). Labels via the tag
  graph. State = staging + series; motion = the Zenoh stream feeding the buffer. **Storage is typed,
  not opaque JSON:** a sample record is `id = [series, seq]` (composite, so a time range is a fast
  ID-range scan), `ts` a `datetime`, and `payload` stored in its **richest typed form** — a scalar as a
  `number`/`bool` (so `AVG`/`MAX`/`GROUP BY` rollup views work), structured data as a **native nested
  object** (queryable, no app-side parsing), arbitrary/schemaless data as an object/array as-is, and
  **binary/large payloads as a file-bucket reference** (the S4 assets path), never streamed inline
  through the buffer. The buffer is uniform across all of these; only the commit step picks the shape,
  by **series class**.
- **PREREQUISITE — persistent backend:** "buffer until written to disk" is **impossible on today's
  store**, which is in-memory only (`Store::memory()`, the `Mem` engine; `crates/store/src/open.rs`).
  A small prerequisite slice must add `Store::open(path)` on the persistent SurrealDB engine
  (`surrealkv`/rocksdb) — **config, not a code branch** (symmetric nodes; the engine is config). Until
  that lands, ingest is non-durable. Flag this as a hard dependency of the S9 slice.
- **Bus (Zenoh):** subjects `ws/{id}/series/{series}` for the live sample stream — **fire-and-forget
  motion** (best-effort; the durable copy is the committed series, so a dropped frame on the live path
  is fine). Must-deliver data (rare for samples, common for commands *down* to a producer) rides the
  **outbox**, unchanged.
- **Sync / authority:** an offline producer buffers samples in its **own node's** staging and replays
  on reconnect; commit is idempotent on `(series, seq)`, so a re-delivered batch never double-commits
  (§6.8). The committed series syncs as `(table, id)` upserts on the existing channel-sync path.
- **Durability is endpoint-to-endpoint, NOT transport retention.** Zenoh is **best-effort motion** — it
  has no durable commit-log, no consumer offsets, no "hold until acked" (relying on it for that would
  violate rule #3, motion-as-state). The "never lost until on disk" guarantee lives at the **two
  endpoints' disks**: the producer keeps its durable staging copy and **prunes it only after a
  commit ack**; the cloud buffer commits to disk then acks; a dropped Zenoh frame is re-sent from the
  producer's staging and de-duped on `(series, seq)`. This survives the producer *and* the cloud
  crashing — strictly stronger than buffering in the transport. (Zenoh `reliable` + `CongestionControl::Block`
  reduce loss and apply backpressure, but are an optimization, never the durability owner.)
- **Secrets:** none in core. A protocol bridge that needs a broker credential mediates it through the
  secrets surface (`scope/secrets/`), out of the host.

## Example flow

A Raspberry Pi (principal `client:pi-7`, a full node) reporting CPU temperature to the cloud:

1. The Pi runs a **`mqtt-bridge` extension** (out-of-core, installed from the registry) — or just its
   own code — that produces `Sample { series: "node.cpu_temp", ts, seq, value: 61.4, labels: {host:"pi-7"} }`.
2. The Pi **publishes** the sample as Zenoh motion on `ws/acme/series/node.cpu_temp`. Cheap, real-time.
   It *also* buffers it in its **local staging** (so an offline Pi loses nothing).
3. The cloud node (holding the **ingest role**) **drains** the stream into its `ingest_staging` table.
   Acceptance is bounded and O(1); a burst fills the buffer, it doesn't block the Pi or storm the store.
4. The ingest worker **commits** batches to the `series` table, **idempotent on `(series, seq)`** —
   a re-delivered batch after a reconnect commits once. The **overflow policy** (drop-oldest for this
   best-effort series) bounds memory under sustained overload; a must-deliver series would **dead-letter**
   instead.
5. A browser dashboard calls **`series.latest("node.cpu_temp")`** for the live value and
   **`series.read("node.cpu_temp", last_1h)`** for the chart — both **capability-checked, workspace-first**.
   It also `watch_presence`es to show `pi-7` as **connected** (the existing S2 presence path).
6. To push a *setpoint down* to the Pi, the cloud writes it as a series the Pi reads
   (`series.latest("node.fan_setpoint")`) **or** sends a must-deliver command through the **outbox** —
   reusing the proven egress path, no new mechanism.

No "device" anywhere in the host. Swap the Pi for a phone app reporting battery, or a server reporting
request latency — identical path, different series names.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md` that apply:

- **Capability deny** — a producer without `mcp:ingest.write:call` is refused (no sample lands); a
  reader without `mcp:series.read:call` is refused; a prefix-scoped grant cannot read outside its prefix.
- **Workspace isolation** — a ws-B producer cannot write a ws-A series; a ws-B reader cannot read or even
  enumerate ws-A series; the buffer's staging is workspace-partitioned (a ws-B drain never commits into
  ws-A). Across **store + MCP**, the standard two surfaces.
- **Offline / sync** — an offline producer buffers locally and replays on reconnect with **one** commit
  per `(series, seq)` (idempotency); the **cloud ingest buffer survives a restart** and re-drains
  uncommitted staging with **no loss and no double-commit** (the new durability case — mirror the outbox
  relay tests).

Plus the load/robustness cases specific to this surface:

- **Backpressure / overflow** — a sustained burst beyond the buffer bound does not OOM; the configured
  overflow policy is honored (drop-oldest count is observable for best-effort; dead-letter for
  must-deliver). A hot single-series partition doesn't stall other series.
- **Rate limiting** — a producer exceeding its per-workspace/principal rate is throttled, not crashed,
  and the limit is workspace-scoped.
- **Read correctness** — `series.read` range queries return the committed range ordered; `series.latest`
  returns the single newest sample; rollup views aggregate as specified.

## Risks & hard problems

- **Cardinality explosion.** Unbounded series names or label combinations blow up the store. Needs a
  **series-creation policy** (grant-gated prefixes, a cap on distinct series per workspace) — the
  highest-risk item, easy to underestimate. Surface it early.
- **Retention / GC.** Time-series grows forever. Rollup table views + an **eviction/retention policy**
  per series (raw samples age out to rollups) are required, not optional — but defer the *mechanism* to
  a follow-up; the *scope* must name it.
- **Buffer durability vs. throughput.** A fully-durable staging write per sample re-introduces the write
  storm the buffer exists to avoid. The likely answer: an **in-memory ring backed by periodic durable
  checkpoints** of the staging cursor — durable enough to bound loss to the last checkpoint window,
  fast enough to absorb bursts. This is the core engineering tension; get it wrong and the buffer is
  either slow or lossy. (Open question below.)
- **The IoT-creep governance risk.** The single biggest *architectural* risk is scope drift: the moment
  a `Device` table, an MQTT dependency, or a "sensor" type lands in a core crate, the platform stops
  being generic. Mitigation: a **review gate** — any core ingest PR is rejected if it names a device
  domain concept; those go to extensions. Call this out in the session doc explicitly.
- **Clock & ordering.** `ts` from an external producer is untrusted and may skew; `seq` (monotonic per
  series) is the ordering/dedup key, `ts` is data. Don't order on wall-clock.

## Open questions

- **Buffer backing:** pure in-memory ring + durable cursor checkpoints, or a durable `ingest_staging`
  table drained transactionally? Recommendation: **durable staging table** for correctness first
  (true to one-datastore, survives restart trivially), optimize to a checkpointed ring only if measured
  throughput demands it.
- **`ingest.write` acknowledgement:** synchronous ack on durable-accept (staged), or fire-and-forget with
  the live stream as the only fast path? Recommendation: **ack-on-stage** for must-deliver series,
  **fire-and-forget motion** for best-effort — the QoS is a per-series property.
- **Overflow policy default:** drop-oldest vs. drop-newest vs. dead-letter — per-series, with what
  default? (Lean drop-oldest for best-effort telemetry.)
- **Series id grammar & label model:** dotted names (`node.cpu_temp`)? Reserved prefixes? How do grants
  scope by prefix? How do labels map onto the tag graph without a cardinality blowup?
- **Rate-limit granularity:** per-workspace, per-principal, or per-series — and where enforced (the
  ingest verb, the bus subscriber, or both)?
- **Retention policy shape:** raw→rollup aging rules, who configures them (a workspace admin grant?),
  and whether eviction is a job (`lb-jobs`) or a SurrealDB-native sweep.
- **Does the ingest role need its own crate `lb-ingest`, or is it a mode of the gateway/sync role?**
  (Lean: its own crate, mounted by role — mirrors `lb-outbox` being separate from the host.)

## Related

- README **§3** (the non-negotiables — state vs motion, one datastore, symmetric nodes), **§6.1**
  (SurrealDB time-series model), **§6.11** (the tag service = series labels), **§6.8** (sync/authority).
- `scope/inbox-outbox/outbox-scope.md` — the durability pattern this mirrors (must-deliver out ↔
  high-volume in).
- `scope/bus/bus-scope.md` — the motion path + presence (the "connected clients" view this complements).
- `scope/sync/sync-scope.md` — offline buffer + idempotent replay (§6.8), reused wholesale.
- `scope/store/` — the SurrealDB record/table model the `series` tables extend.
- `scope/extensions/` + `scope/registry/` — where protocol-bridge adapters (MQTT/Modbus/…) live, **out
  of core**, installed as signed artifacts.
- `scope/node-roles/` — the **ingest role** (currently a stub); the "every node is full-stack, a Pi can
  be a LAN sub-hub" principle this scope leans on.
- `scope/tags/` — the label model.
- `extensions/github-bridge/` — the worked precedent for a pure-transform `normalize → canonical
  envelope` adapter that the host composes but does not depend on.
