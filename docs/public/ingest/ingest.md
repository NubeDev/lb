# Ingest — buffered read/write surface for high-volume external data

Status: **TODO** (stub). Promoted from `scope/ingest/ingest-scope.md` when the S9 slice ships.

The generic, capability-gated surface for absorbing high-volume external data (sensor streams,
app metrics, edge-node reports) into workspace-scoped **time-series state**, via a durable
**cloud-side ingest buffer** (the read-side analog of the outbox). A "device" is just a principal
on a node — IoT is one caller, never a core concept.

Filled in on ship with: the `Sample` envelope, the `series` data model, the `lb-ingest` buffer
(accept → backpressure/batch/dedup → commit), the `ingest.write` / `series.read` / `series.latest`
MCP verbs, and the green deny + isolation + offline-replay + overflow tests.

See `scope/ingest/ingest-scope.md` for the ask.
