# Sink node could never write — its request shapes didn't match the target verbs ("missing field `producer`" / "missing arg: id")

- **Date:** 2026-06-30
- **Area:** flows (sink node execution)
- **Status:** resolved

## Symptom

User tested the `sink` node on the live canvas: the **series** and **inbox** targets both errored.
The series sink failed with **`bad input: samples: missing field \`producer\``**; the inbox sink failed
with **`missing arg: id`**. "I didn't test the others, I gave up."

## Root cause

`dispatch_sink` (`crates/host/src/flows/execute_node.rs`) built request bodies that did **not** match the
contracts of the tools it called:

- **`series` → `ingest.write`**: built `{ samples: [{ series, value, ts }] }`. But `ingest.write`
  deserializes each sample into `lb_ingest::Sample`, whose required fields are `series`, **`producer`**,
  `ts`, **`seq`**, **`payload`** (none have `#[serde(default)]` — `producer` is stamped to the
  authenticated principal *inside* the verb but must still be present to deserialize). So serde failed on
  the first missing field, `producer`. It also used the wrong key (`value` instead of `payload`).
- **`inbox`/`channel` → `inbox.record`**: built `{ channel, body }`. But the `inbox.record` dispatch
  requires a caller-supplied **`id`** (idempotent on `(channel, id)`) and treats `body` as a string
  (a non-string `body` silently became `""`).

There was **no sink-execution test** for these targets — the only sink coverage asserted descriptor
ports, never drove a run through a sink — so it shipped broken. (The `outbox` target uses
`enqueue_outbox` directly, not a tool dispatch, so it was unaffected.)

## Fix

In `dispatch_sink`:

- **series** now sends a full valid sample: `{ series: name, producer: "", ts: now, seq: now, payload:
  value }`. `producer` is `""` (the verb overwrites it with the principal); `seq` is `now` — monotonic
  across firings, and a retry of the *same* firing reuses it so the idempotent `[series,producer,seq]`
  upsert never double-writes.
- **inbox/channel** now sends `{ channel: name, id: "{run_id}:{node_id}", body, ts: now }`. The id is
  derived from (run, node) so a resume/retry upserts the same item (no duplicate) while each new firing
  (a fresh run id) records a new item; a structured `value` is stringified so the body is never empty.

## Regression test (real store/caps/ingest/inbox — no mocks)

`crates/host/tests/flows_sink_test.rs`:
- `sink_series_writes_a_sample_readable_by_series_latest` — drives a real run through a `series` sink,
  asserts the step settles `ok {accepted:1}`, then reads it back via `series.latest` (`payload == 42`).
- `sink_inbox_records_an_item_readable_by_inbox_list` — same for an `inbox` sink; asserts `{recorded:
  true}` then finds the item via `inbox.list`.
- `sink_inbox_records_a_structured_value_as_a_stringified_body` — a JSON object value lands as a
  non-empty stringified body.

**Fail-before verified:** reverting just the series shape reproduced the exact user error —
`bad input: samples: missing field \`producer\`` — and the test failed; with the fix all three pass.
`cargo fmt`/build clean.

> Note: the fix is in the host binary, so the user's running dev node must be rebuilt + restarted
> (`make kill && make dev`) for the live canvas sink to work.
