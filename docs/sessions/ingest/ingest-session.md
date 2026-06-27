# Ingest — durable buffered read/write surface (session)

- Date: 2026-06-27
- Scope: ../../scope/ingest/ingest-scope.md
- Stage: S8 — data plane (durable store + ingest + tagging) (slice 1). See STAGES.md.
- Status: done

## Goal
Prove the durable exactly-once round-trip end to end: a generic `Sample` firehose lands cheaply in a
durable staging table, a commit worker drains batches into typed `series` state one-transaction-per-batch
(UPSERT on `[series, producer, seq]`), and `series.read`/`series.latest`/`series.find` read it back —
all capability-gated and workspace-first. Built on slice 0's persistent engine.

## What changed
- New crate `crates/ingest` (`lb-ingest`): `Sample`/`Qos` envelope; `write` (durable append to
  `ingest_staging`, no indexes/edges); `commit_batch` (one batch = one tx, UPSERT on
  `[series,producer,seq]` + delete-staged-in-same-tx → atomic + exactly-once on re-drain); `read`/
  `latest`; `enforce_bound`/`OverflowPolicy` (drop-oldest / dead-letter, bounded at the staging end).
- Host service `crates/host/src/ingest/`: the MCP gate (`mcp:<verb>:call`), `ingest_write` (stamps the
  **authenticated principal** as `producer` — un-spoofable), `series_read_range`/`series_latest_value`,
  `series_find` (tag-driven discovery, on top of tags), `drain_workspace` (the ingest-role commit
  worker), and `call_ingest_tool` (the MCP bridge for `ingest.write`/`series.read`/`series.latest`/
  `series.find`).

## Decisions & alternatives
- **Dedup identity `(series, producer, seq)`** (the resolved lean), `producer` = the authenticated
  calling principal. The host overwrites the wire `producer` so it can't be forged to collide with or
  overwrite another producer's stream. Keying on `(series, seq)` is rejected (silent data loss across
  producers).
- **`ingest_write(samples, bound)` returns accepted count; commit is a separate worker.** Acceptance is
  a durable disk write (staging), not an in-memory O(1) op — the relief is cheap-append vs
  expensive-indexed-write, batched at commit. Rejected an in-memory ring (not the correctness baseline).
- **Binary payloads → record-as-content.** The slice-0 spike marked `DEFINE BUCKET` unavailable, so the
  DEGRADABLE fallback applies (inline typed value); scalars stay numbers, structured stays nested
  objects — never opaque JSON.
- **Ingest role mounts the drain worker** (config, like gateway/sync relay). No `if cloud`.
- **Overflow bounded at the staging end this slice** (drop-oldest for best-effort, dead-letter for
  must-deliver). Producer-side staging bound + rate-limiting + checkpointed-ring deferred per scope.

## Tests
`cargo test -p lb-ingest` and `cargo test -p lb-host --test ingest_test --test ingest_isolation_test`
— all green (output in STATUS / final verify). Mandatory + scope-specific:
- **Capability deny** — `denies_write_without_capability`, `denies_read_without_capability`.
- **Workspace isolation** (store + MCP) — `ws_b_cannot_read_ws_a_series`, `ws_b_cannot_write_ws_a_series`.
- **Offline/sync — kill MID-COMMIT** (not graceful): `durable_redrain_test` spawns `crash_ingest`,
  SIGABRTs it after staging / after commit, reopens the persistent store, and asserts exactly-once
  re-drain + no double-commit + atomic rollback.
- **Two-producer collision** — `two_producers_same_seq_both_survive` (crate + host): producer-A and
  producer-B both write seq=5 to one series → BOTH survive.
- **Overflow at both QoS** — `best_effort_overflow_drops_oldest`, `must_deliver_overflow_dead_letters`.
- **Typed read** — scalar stays number, structured stays nested object.

## Anti-IoT discipline
No `device`/`sensor`/`firmware`/`MQTT` concept appears anywhere in `lb-ingest` or the host ingest
module — a producer is a principal, the surface is a generic `series`. Verified by reading the crate;
the module docs state the rule explicitly. Protocol bridges remain out-of-core extensions.

## Debugging
- debugging/ingest/u64-max-bound-coerces-to-float.md — `seq <= u64::MAX` returned nothing (the huge int
  coerces to a float and mis-compares); fixed by making range bounds `Option<u64>` and omitting the
  clause when open. Regression: `write_commit_read_round_trips_typed` reads with open bounds.
- debugging/ingest/delete-order-by-limit-unsupported.md — `DELETE … ORDER BY … LIMIT` is unsupported;
  drop-oldest now deletes via a subquery selecting the oldest id. Regression:
  `best_effort_overflow_drops_oldest`.

## Public / scope updates
- Promoted to `public/ingest/ingest.md`.
- Scope open questions: ack-on-stage for must-deliver / fire-and-forget for best-effort (QoS per series)
  — modeled by `Qos`; producer is the authenticated principal (lean taken); overflow default drop-oldest
  (lean taken). Deferred (named in scope): producer-side staging bound, rate-limiting, retention/GC,
  one-authoritative-ingest-path sub-hub wiring.

## Dead ends / surprises
- The staging drain query tripped the same selected-idiom rule as the existing inbox ORDER BY note —
  fixed by selecting the order keys in the projection.

## Follow-ups
- Producer-side staging bound + rate limits (scope defer-list).
- `series.find` faceted discovery now lands on top of tags (slice 2) — wired this session.
- STATUS.md updated: slice 1 shipped.
