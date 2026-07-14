# Ingest scope — a generic buffered read/write surface for high-volume external data

Status: scope (the ask). Promotes to `public/ingest/` once shipped. Target stage: **S8 — the data
plane** (after the S7 registry + native tier; see STAGES.md). Depends on the
`scope/store/persistent-backend-scope.md` enabling slice (slice 0) shipping first.

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

- A **canonical `Sample` envelope** — `{ series, producer, ts, seq, payload, labels, qos }` — that every producer,
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
  `series.latest` (last value per series — kept generic; *not* a "device shadow"). Capability-gated, workspace-first.
- **Robustness primitives**: bounded buffer + explicit **overflow policy** (**both ends** — producer
  staging *and* cloud staging are bounded), **idempotent commit** on the dedup identity
  **`(series, producer, seq)`** (NOT `(series, seq)` — see below), **per-workspace/principal rate
  limits**, and **offline buffer + idempotent replay** on reconnect (reuse §6.8 sync, not a new mechanism).
- **A safe dedup identity for a multi-producer series.** The opening use case is "a fleet of edge nodes
  reporting state" — so two producers may write the *same* series. The dedup/commit key is therefore
  **`(series, producer, seq)`**, and the sample record id is `[series, producer, seq]`. Keying on
  `(series, seq)` alone would let producer-B's `seq=5` silently upsert over producer-A's — data loss
  disguised as idempotency. `seq` is monotonic **per `(series, producer)`**, not per series. (Alternative:
  a series provably single-producer by grant — kept as a narrowing, not the default.)
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
`(series, producer, seq)`." So the design reuses the outbox's proven discipline — a durable staging set,
idempotent commit, backoff/dead-letter — rather than inventing a queue.

**Three seams, each an existing primitive:**

1. **Motion in** — producers publish `Sample`s as Zenoh motion on `ws/{id}/series/{series}` (best-effort,
   high-rate). This is the cheap, real-time path; a live dashboard subscribes here directly.
2. **The ingest buffer** — a cloud-side subscriber (the new `lb-ingest` crate, run by a node holding the
   *ingest role*) drains the stream into a **durable, append-only staging table**, applies the
   overflow/rate/batch policy, and commits batches to the `series` tables. A restart re-drains
   uncommitted staging — no loss, no double-commit.
3. **State out** — `series.read` / `series.latest` read the committed series (range queries over the
   record-ID-range layout; `series.latest` is the single newest record per series).

**Why a buffer at all, vs. writing each sample straight to the `series` tables? (And why staging isn't
just the same write storm.)** The relief is **not** "avoid disk writes" — staging *is* durable, so
acceptance is a disk write, not an in-memory O(1) op (an earlier draft claimed O(1); that was wrong). The
relief is that a **staging append is cheap** where a **direct `series` write is expensive**: staging is
append-only, single table, **no secondary indexes, no rollup-view maintenance, no tag-graph edges**;
the `series` commit is where all of that index/rollup/edge cost lives, and the buffer does it **in
batches** (one transaction per batch, amortizing the expensive part) at the store's pace rather than
once per sample under burst. So the firehose hits the cheap path; the expensive path runs batched and
backpressured. **Durability window:** a sample is durable once its staging append is fsync'd; staging
batches fsyncs, so a bounded window (one fsync interval) of just-accepted samples may sit in the OS
buffer — losable on a hard power-cut. That window is the explicit, bounded cost of throughput; name it,
don't hide it. This decoupling — plus the explicit overflow policy (drop-oldest for best-effort;
dead-letter for must-deliver) — is why "buffer the data in the cloud" is the right framing.

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
  acting as a **LAN sub-hub** can run its own ingest buffer for its local peers. No `if cloud {…}`; the
  role selects whether the buffer subscriber is mounted, like the gateway and sync relay do today.
  **One authoritative ingest path per producer (no double-delivery).** A producer whose samples are
  ingested by a sub-hub is committed *there*; that committed series syncs upward as `(table,id)` upserts
  (§6.8) and the cloud does **not** also re-ingest the raw stream for it. A producer with no sub-hub
  ingests directly at the cloud. The rule: a sample is committed by exactly one ingest node; the other
  tier receives it only via series **sync**, never via a second ingest. Otherwise a sample arriving both
  by sync-of-committed-series and by re-ingest would double-count.
- **MCP surface:** `ingest.write(samples)` (a **heterogeneous `Sample[]`** — many series, many payload
  types, one call), `series.read(series, range)`, `series.latest(series)`, `series.list(prefix)`, and a
  tag-driven discovery verb `series.find(tags)` (faceted `key:value` query over the graph). Producers
  and the UI call them identically (MCP is the universal contract).
- **Tags / graph (one source of truth):** the **tag-graph edges are authoritative**; the inline
  `Sample.labels` are only the **producer's raw declaration on the wire**. At commit, the buffer
  **converts** declared labels into `series ->tagged-> tag:[key,value]` edges (the existing tag service,
  README §6.11, `scope/tags/`) **once per series, not per sample** (a label describes the series, so the
  edge is written when the series is first seen / when its label set changes — not on every sample). The
  series rows do **not** also store labels — no parallel store. The producer pays the cardinality cost of
  the dimensions it declares, and the tag-cardinality cap (`scope/tags/`) bounds it. Discovery is then a
  graph query over edges — by metadata/relationship, not by schema. Tags are for dimensions you filter
  by, never high-cardinality values (those stay in the payload).
- **Data (SurrealDB):** a durable append-only **`ingest_staging`** table (the buffer's backing — survives
  restart), and the **`series`** tables (record-ID ranges per series + rollup table views). Labels via
  the tag graph (above). State = staging + series; motion = the Zenoh stream feeding the buffer.
  **Commit boundary:** **one batch = one SurrealDB transaction**, and the commit is an **UPSERT keyed on
  `[series, producer, seq]`** — so a die-mid-batch rolls the whole batch back (atomic) and a re-drain
  upserts each sample exactly once. This is what makes "no double-commit on restart" true rather than
  hoped. **Storage is typed, not opaque JSON:** a sample record is `id = [series, producer, seq]`
  (composite, so a per-producer time range is a fast ID-range scan), `ts` a `datetime`, and `payload`
  stored in its **richest typed form** — a scalar as a
  `number`/`bool` (so `AVG`/`MAX`/`GROUP BY` rollup views work), structured data as a **native nested
  object** (queryable, no app-side parsing), arbitrary/schemaless data as an object/array as-is, and
  **binary/large payloads as a file-bucket reference** (the S4 assets path), never streamed inline
  through the buffer. The buffer is uniform across all of these; only the commit step picks the shape,
  by **series class**.
- **PREREQUISITE — the store persistent-backend slice + its GO/NO-GO matrix (hard gate).** "Buffer until
  written to disk" is **impossible on today's store**, which is in-memory only (`Store::memory()`, the
  `Mem` engine; `crates/store/src/open.rs`). Ingest **must not start** until
  `scope/store/persistent-backend-scope.md` lands `Store::open(path)` *and* its spike publishes the
  feature matrix. Ingest branches on it: **durability + transactions + composite IDs are LOAD-BEARING**
  (a ✗ is NO-GO for ingest too); **`DEFINE BUCKET` is DEGRADABLE** — if the spike marks buckets
  unavailable, **binary/large payloads fall back to the S4 record-as-content path** (inline-by-value with
  a size cap) until buckets land, and the rest of ingest ships. No engine code-branch (symmetric nodes;
  the engine is config).
- **Bus (Zenoh):** subjects `ws/{id}/series/{series}` for the live sample stream — **fire-and-forget
  motion** (best-effort; the durable copy is the committed series, so a dropped frame on the live path
  is fine). Must-deliver data (rare for samples, common for commands *down* to a producer) rides the
  **outbox**, unchanged.
- **Sync / authority:** an offline producer buffers samples in its **own node's** staging and replays
  on reconnect; commit is idempotent on `(series, producer, seq)`, so a re-delivered batch never
  double-commits (§6.8). The committed series syncs as `(table, id)` upserts on the existing channel-sync
  path (and is the *only* upward path — see Placement, no re-ingest).
- **Durability is endpoint-to-endpoint, NOT transport retention — and scoped to QoS.** Zenoh is
  **best-effort motion**: no durable commit-log, no consumer offsets, no "hold until acked" (relying on
  it for that would violate rule #3). For a **`qos: must-deliver`** series the "never lost until on disk"
  guarantee lives at the **two endpoints' disks**: the producer keeps its durable staging copy and
  **prunes it only after a commit ack**; the cloud commits to disk then acks; a dropped frame is re-sent
  from producer staging and de-duped on `(series, producer, seq)`. For a **`qos: best-effort`** series
  (the default, e.g. high-rate telemetry) the path is **lossy by design** — drop-oldest under overflow,
  no per-sample ack — and the "never lost" promise **does not apply**; the durable copy is whatever
  committed. Do not read the must-deliver guarantee as universal.
- **Producer staging is itself bounded (independent of acks).** Acks ride the same best-effort Zenoh, so
  on an asymmetric link (data up, acks dropped) a producer could re-send forever and never prune —
  filling its own disk; on a Pi that is an outage. So **producer staging has its own retention/overflow
  policy** (bounded size/age; oldest-unacked dropped or dead-lettered past the bound), *independent* of
  ack receipt. Every overflow control exists at **both ends**, not just cloud-side. (Zenoh `reliable` +
  `CongestionControl::Block` reduce loss / apply backpressure, but are an optimization, never the
  durability owner.)
- **Secrets:** none in core. A protocol bridge that needs a broker credential mediates it through the
  secrets surface (`scope/secrets/`), out of the host.

## Example flow

A Raspberry Pi (principal `client:pi-7`, a full node) reporting CPU temperature to the cloud:

1. The Pi runs a **`mqtt-bridge` extension** (out-of-core, installed from the registry) — or just its
   own code — that produces `Sample { series: "node.cpu_temp", producer: "client:pi-7", ts, seq,
   payload: 61.4, labels: {host:"pi-7"}, qos: "best-effort" }`.
2. The Pi **publishes** the sample as Zenoh motion on `ws/acme/series/node.cpu_temp`. Cheap, real-time.
   It *also* buffers it in its **local staging** (bounded — see producer overflow), so a must-deliver
   sample survives an offline Pi.
3. The cloud node (holding the **ingest role**) **drains** the stream with a **cheap append** into its
   `ingest_staging` table (no indexes/edges on that write); a burst hits the cheap path, it doesn't storm
   the indexed `series` tables.
4. The ingest worker **commits** batches to the `series` table — **one batch = one transaction**, UPSERT
   on `[series, producer, seq]`, so a re-delivered batch after a reconnect commits exactly once and a
   die-mid-batch rolls back. The **overflow policy** (drop-oldest for this best-effort series) bounds the
   buffer under sustained overload; a must-deliver series would **dead-letter** instead.
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
  per `(series, producer, seq)` (idempotency). The **cloud-restart re-drain test must kill mid-commit**,
  not after a graceful drain (a graceful "restart" proves nothing): with uncommitted samples in staging,
  **kill the node**, reopen on the persistent engine, and assert each uncommitted sample commits
  **exactly once** and any partially-applied batch rolled back. A **two-producer collision** test writes
  `seq=5` from producer-A and producer-B to the same series and asserts **both** survive (the
  `(series, producer, seq)` key, not `(series, seq)`).

Plus the load/robustness cases specific to this surface:

- **Backpressure / overflow at BOTH ends** — a sustained burst beyond the buffer bound does not OOM and
  the overflow policy is honored (drop-oldest observable for best-effort; dead-letter for must-deliver);
  **and** a producer whose acks are dropped (asymmetric link) honors its **own** staging bound rather
  than filling its disk. A hot single-series partition doesn't stall other series.
- **Rate limiting** — a producer exceeding its per-workspace/principal rate is throttled, not crashed,
  and the limit is workspace-scoped.
- **Read correctness** — `series.read` range queries return the committed range ordered; `series.latest`
  returns the single newest sample; rollup views aggregate as specified.

## Risks & hard problems

- **Cardinality explosion.** Unbounded series names or label combinations blow up the store. Needs a
  **series-creation policy** (grant-gated prefixes, a cap on distinct series per workspace) — the
  highest-risk item, easy to underestimate. Surface it early.
  *(Shipped 2026-07-14, issue #55: `series_meta` registry + per-workspace cap at commit — over-cap
  samples dead-letter, never a new index entry. Grant-by-prefix series creation remains a follow-up.)*
- **Retention / GC.** Time-series grows forever. Rollup table views + an **eviction/retention policy**
  per series (raw samples age out to rollups) are required, not optional — but defer the *mechanism* to
  a follow-up; the *scope* must name it.
  *(Shipped 2026-07-14, issue #58: [`series-retention-scope.md`](series-retention-scope.md) —
  per-prefix policy, rollup tiers, `series.retention.*` verbs, on-demand GC.)*
- **Buffer durability vs. throughput (resolved in Intent — recorded here as the residual risk).** Staging
  is a **durable append** (cheap: no indexes/edges/rollups) and `series` commit is **batched** (one tx,
  amortizing the index/rollup/edge cost); the relief is cheap-append-vs-expensive-indexed-write, not
  avoiding disk. The residual risk is the **fsync-batching window**: a hard power-cut can lose the last
  un-fsync'd append interval of best-effort samples. Bounded and named, not hidden; a checkpointed
  in-memory ring is a *later* throughput optimization only if measurements demand it, never the
  correctness baseline.
  *(**Shipped 2026-07-15**: [`drain-backpressure-scope.md`](drain-backpressure-scope.md) /
  [session](../../sessions/ingest/drain-backpressure-session.md). The batching above amortizes the
  commit correctly, but the **commit worker this scope names was never given a driver** — so every
  caller became the worker, synchronously and unbounded: a one-sample `ingest.write` against a
  4,671-row backlog took 18.5s vs 21ms at backlog 0. Fixed by bounding each caller's drain to its
  own batch and finally spawning the worker (`spawn_ingest_reactors`, the outbox relay's twin) at
  node boot. The suspected `ORDER BY` superlinearity was measured and **disproven** — staging stays
  index-free as this scope intends.)*
- **The IoT-creep governance risk.** The single biggest *architectural* risk is scope drift: the moment
  a `Device` table, an MQTT dependency, or a "sensor" type lands in a core crate, the platform stops
  being generic. Mitigation: a **review gate** — any core ingest PR is rejected if it names a device
  domain concept; those go to extensions. Call this out in the session doc explicitly.
- **Clock & ordering.** `ts` from an external producer is untrusted and may skew; `seq` (monotonic per
  series) is the ordering/dedup key, `ts` is data. Don't order on wall-clock.

## Open questions

**Resolved by the shipped slice (2026-06-27) — see `sessions/ingest/ingest-session.md`:**

- **`ingest.write` acknowledgement:** QoS is a per-series property on `Sample` (`best-effort` |
  `must-deliver`); `write` returns the accepted count (durable-accept). The lean is taken.
- **Overflow policy default:** **drop-oldest** for best-effort, **dead-letter** for must-deliver, bounded
  at the staging end this slice (producer-side bound deferred — defer-list).
- **Producer identity in the dedup key:** **the authenticated calling principal** — the host overwrites
  the wire `producer` before staging (un-spoofable). Lean taken.
- **Series id grammar (still open):** dotted names work end to end (e.g. `series:node.cpu_temp` via the
  two-arg `type::thing` in tags); reserved prefixes / grant-by-prefix scoping remain a follow-up.

Resolved in this doc (no longer open): the dedup identity is **`(series, producer, seq)`**; staging is a
**durable append**, commit is **one-tx-per-batch UPSERT** (not an in-memory ring); the tag-graph is the
**single source of truth** for labels (inline `Sample.labels` are a wire declaration converted to edges
once per series); "never lost" is **scoped to `qos: must-deliver`**; one authoritative ingest path per
producer (no re-ingest double-count).

**Series lifecycle** (shipped — `sessions/ingest/series-lifecycle-session.md`): a series can be
**deleted** (`series.delete`) or **renamed** (`series.rename`), carrying/clearing its *whole*
footprint — sample rows, rollup tiers, staged rows, the `series_meta` registry row (and its
`labels_applied` latch on rename), and the `series:<name>` tag edges (both the `in` link and the
denormalized `ent` string `series.find` reads). Retention policies are **prefix-keyed**, so they are
left untouched. Rename **refuses a merge** into an occupied name (the `(series, producer, seq)` dedup
identity must stay collision-free) — a `BadInput`, not a `Denied`. Both are minted as their own caps
(`mcp:series.delete:call` / `mcp:series.rename:call`) granted to the **admin/owner role only**,
alongside `series.retention.*` — destroying a whole series is workspace-data administration, not an
author privilege.
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
