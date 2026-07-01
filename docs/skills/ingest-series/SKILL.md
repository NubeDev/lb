---
name: ingest-series
description: >-
  Ingest high-volume external data and read it back as time-series over the node gateway. Append a
  heterogeneous `Sample[]` with `ingest.write`, then range-read (`series.read`), get the newest value
  (`series.latest`), enumerate (`series.list`), and tag-discover (`series.find`). Use when a task says
  "write samples/telemetry", "ingest sensor/metric data", "read a series range", "get the latest
  value", "list/find series", or "call ingest/series verbs over REST/MCP". The core noun is a generic
  workspace-scoped `series` of timestamped values — NOT a device/IoT type; a producer is just a
  principal on a node. SurrealDB holds the committed series (state); Zenoh moves the live stream
  (motion); the buffer is the seam.
---

# Ingesting & reading time-series (`ingest.*` / `series.*`)

Lazybones absorbs high-volume external data through a **generic `series`** surface — the read-side
analog of the outbox. A producer normalizes to a canonical `Sample` envelope and appends a batch;
the host stages it durably (a cheap append), then **commits batches** to the indexed `series` tables
(idempotent on `(series, producer, seq)`). Reads run over the committed series. There is **no
device/sensor/IoT concept** in the core — a "device" is a principal, its "config" is a series it
reads.

The crate is `rust/crates/ingest/` (`lb-ingest`, the `Sample` + buffer) with the host bridge at
`rust/crates/host/src/ingest/`. Two call styles, as with the other host surfaces:

1. **Dedicated REST routes** — `/ingest`, `/series…`.
2. **The universal MCP bridge** — `POST /mcp/call {tool, args}` by dotted name.

Both derive **workspace + principal from the token** (the hard wall). Every verb is capability-gated;
a denial is opaque (an un-granted producer leaks nothing about what series exist).

## 1. Authenticate

```bash
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login \
  -H 'content-type: application/json' -d '{"user":"user:pi-7","workspace":"acme"}' | jq -r .token)
```

Capabilities: `mcp:ingest.write:call` gates writes; `mcp:series.read:call` gates reads (a grant may
be narrowed by a series prefix). The **producer identity is host-forced** — the ingest verb overwrites
the wire `producer` with the authenticated principal (un-spoofable), so it's half of the dedup key you
cannot forge.

## 2. The verbs

| Action | REST route | MCP bridge (`POST /mcp/call`) | Args |
|---|---|---|---|
| Write samples | `POST /ingest` | `{"tool":"ingest.write","args":{"samples":[…]}}` | `samples` (`Sample[]`) |
| Read a range | `GET /series/{series}/samples?from=&to=` | `{"tool":"series.read","args":{"series":"…","from_seq":?,"to_seq":?}}` | `series`, `from_seq?`, `to_seq?` |
| Latest value | `GET /series/{series}/latest` | `{"tool":"series.latest","args":{"series":"…"}}` | `series` |
| List series | `GET /series?prefix=` | `{"tool":"series.list","args":{"prefix":"…"?}}` | `prefix?` |
| Tag-discover | `POST /series/find` | `{"tool":"series.find","args":{"facets":[…]}}` | `facets` |
| Live stream (SSE) | `GET /series/{series}/stream?token=<jwt>` | — | — |

`from_seq`/`to_seq` bound the range by the monotonic `seq`; **omit for open bounds** (never a
`u64::MAX` sentinel). `series.list` `prefix` is optional (absent → every series). `ingest.write` over
the bridge **drains staging immediately**, so a write-then-`series.latest`/`read` round-trips on the
same call.

## 3. The `Sample` envelope

`ingest.write` takes a **heterogeneous** `Sample[]` — one call can push a scalar metric, a structured
event, and a string, each naming its own series:

```jsonc
{
  "series": "node.cpu_temp",       // named, workspace-scoped sequence (dotted names OK)
  "producer": "client:pi-7",       // HOST-FORCED to the authenticated principal — half the dedup key
  "ts": 1719800000000,             // logical timestamp — DATA, never the ordering key (external clocks skew)
  "seq": 42,                       // monotonic per (series, producer) — the ordering + dedup key
  "payload": 61.4,                 // any SurrealDB-typed value: number/bool, nested object, array, string
  "labels": { "host": "pi-7" },    // producer's raw dimension declaration → tag edges at commit (once/series)
  "qos": "best-effort"             // best-effort (default, lossy) | must-deliver (durable, never lost)
}
```

- **Dedup is `(series, producer, seq)`** — the record id is `[series, producer, seq]`, so a re-delivered
  batch UPSERTs each sample exactly once (idempotent replay), and two producers writing the same
  series don't clobber each other (producer-B's `seq=5` never overwrites producer-A's).
- **`payload` is stored in its richest typed form** — a scalar as a `number`/`bool` (so `AVG`/`MAX`/
  rollup views work), structured data as a native nested object, binary/large as a bucket reference —
  never an opaque JSON blob. Keep a series' payload type stable for predictable reads/rollups.
- **`labels` are a wire declaration, not the source of truth** — at commit they become
  `series ->tagged-> tag:[key,value]` graph edges (the tag service), written once per series, not per
  sample. Discovery is a graph query over those edges (`series.find`), not over the payload. Tag only
  dimensions you filter by; high-cardinality values stay in the payload.
- **`qos`** (`kebab-case`): `best-effort` (default — overflow drops oldest, no per-sample ack, "never
  lost" does NOT apply) or `must-deliver` (durable at both endpoints, re-sent and de-duped on the key).

```bash
# push a heterogeneous batch, then read it back on the same round-trip
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{"tool":"ingest.write","args":{"samples":[
    {"series":"node.cpu_temp","producer":"client:pi-7","ts":1719800000000,"seq":42,"payload":61.4,"labels":{"host":"pi-7"}},
    {"series":"node.event","producer":"client:pi-7","ts":1719800000500,"seq":7,"payload":{"kind":"boot","ok":true}}
  ]}}'

curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{"tool":"series.latest","args":{"series":"node.cpu_temp"}}'
```

## 4. Discovery by tag (`series.find`)

`facets` is an array of `{key, value?}` — `value` present is an exact match, absent is a key-only
("has this dimension") match. It queries the tag graph, not the payload:

```bash
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d '{"tool":"series.find","args":{"facets":[{"key":"host","value":"pi-7"},{"key":"region"}]}}'
# → {"series":[…]}   — series tagged host=pi-7 AND having a region dimension
```

## Gotchas

- **`producer` is host-forced** — whatever you put on the wire is overwritten with the authenticated
  principal before staging; you can't spoof another producer into the dedup key.
- **Order and dedup on `seq`, not `ts`** — `ts` is untrusted data (external clocks skew); `seq` is the
  monotonic-per-`(series,producer)` ordering + dedup key.
- **Re-delivery is safe** — commit UPSERTs on `[series, producer, seq]`; a replayed batch never
  double-commits. This is what makes offline buffer + reconnect replay idempotent.
- **`qos: best-effort` is lossy by design** — overflow drops oldest, no per-sample ack; the "never
  lost until on disk" promise applies ONLY to `must-deliver`.
- **Labels ≠ a second store** — they're converted to tag edges once per series; the series rows don't
  also store labels. Query dimensions via `series.find`, not by scanning payloads.
- **Denials are opaque** — a missing cap, a prefix-scoped grant reading outside its prefix, and a
  missing series all look the same; no existence signal leaks.
- **Workspace wall** — a ws-B producer can't write or even enumerate a ws-A series; staging is
  workspace-partitioned.
- **No device/IoT nouns** — if you're reaching for "device"/"sensor"/"MQTT", that's a protocol-bridge
  *extension* (the `github-bridge` pattern) that normalizes raw bytes → `Sample[]`, never a core concept.

## Related

- Scope + session: `docs/scope/ingest/ingest-scope.md`, `docs/sessions/ingest/ingest-session.md`.
- The tag/label model `series.find` queries: `docs/scope/tags/`.
- The durability pattern this mirrors (must-deliver out ↔ high-volume in):
  `docs/scope/inbox-outbox/outbox-scope.md`, `docs/skills/channels-inbox-outbox/SKILL.md`.
- Rendering a canonical unit-tagged sample per-user: `docs/skills/prefs/SKILL.md` (`format.quantity`).
- Rules read series via `source(…)`: `docs/skills/rules/SKILL.md`.
- README §3 (state vs motion, one datastore, symmetric nodes), §6.1 (time-series), §6.11 (tags), §6.8
  (sync). Source: `rust/crates/ingest/`, `rust/crates/host/src/ingest/`, routes in
  `rust/role/gateway/src/server.rs`.
