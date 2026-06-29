# Every untruncated `query_result` item silently parsed back as plain chat

- **Area:** channels (`rust/crates/host/src/channel/payload.rs`)
- **Status:** resolved
- **First seen:** 2026-06-29
- **Resolved:** 2026-06-29
- **Session:** ../../sessions/channels/channels-query-charts-session.md
- **Regression test:** `rust/crates/host/src/channel/payload.rs` (`untruncated_result_omits_truncated_yet_round_trips`, `truncated_result_round_trips`, plus the pre-existing `result_round_trips` which started failing)

## Symptom

The channel query worker's own unit test `result_round_trips` panicked at `parse_payload(&body).expect("parsed")`. Tracing the failure, `parse_payload` reported `missing field 'truncated'`. In production this is worse than a panic: `parse_payload` swallows the error to `None`, so a successful `query_result` item (the common case, `truncated:false`) would parse back as **chat** — the result card never renders, the chart never shows, and nothing errors. The whole happy path silently degrades to "a JSON blob posted as a chat message".

## Reproduce

```rust
let body = result_body("s", "SELECT 1", vec!["v".into()], vec![json!({"v":1})], None, /*truncated*/ false);
assert!(matches!(parse_payload(&body), Some(ItemPayload::QueryResult(_)))); // FAILS before the fix
```

## Investigation

`QueryResultPayload::truncated` carried `#[serde(skip_serializing_if = "is_false")]`, so the **false** case (the overwhelming majority of results) is dropped from the serialized body to keep the wire small. But the struct had no `#[serde(default)]`, so deserialization treated the now-absent field as a hard error. Serialize and deserialize disagreed: the writer omits the field, the reader demands it.

## Root cause

Asymmetric serde attributes on `QueryResultPayload::truncated`: `skip_serializing_if` without a matching `default`. The wire form a writer produces (`{...,"columns":[...],"rows":[...]}` — no `truncated`) is a form the reader rejects.

## Fix

Add `#[serde(default)]` alongside `skip_serializing_if` so the omitted-when-false form round-trips:

```rust
#[serde(default, skip_serializing_if = "is_false")]
pub truncated: bool,
```

## Verification

`cargo test -p lb-host --lib channel` — `result_round_trips`, `untruncated_result_omits_truncated_yet_round_trips`, and `truncated_result_round_trips` all green.

## Prevention

The `untruncated_result_omits_truncated_yet_round_trips` regression test asserts both that the false case is dropped from the wire **and** that it still parses back to `QueryResult` — pinning the round-trip the bug broke. Guardrail/class rule: any field with `skip_serializing_if` on a payload type that must round-trip needs a matching `#[serde(default)]`.
