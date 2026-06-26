# Store write fails: "Serialization error: invalid type: enum, expected any valid JSON value"

- Area: store
- Status: resolved
- First seen: 2026-06-26
- Resolved: 2026-06-26
- Session: ../../sessions/core/s0-s1-spine-session.md
- Regression test: rust/crates/store/tests/isolation_test.rs

## Symptom

`store::write` returned `Backend("Serialization error: invalid type: enum, expected any
valid JSON value")` for a plain `serde_json::Value` object, so both store-isolation tests
failed at the first write.

## Reproduce

`store::write(&store, "a", "note", "1", &json!({ "body": "x" })).await` → the error above.

## Investigation

- The value was a perfectly ordinary JSON object; the failure was in SurrealDB's serializer,
  not our data.
- SurrealDB 2.x's `.content(value)` serializes the argument through its *own* value model.
  Feeding it a `serde_json::Value` makes serde present an externally-tagged enum
  (`Value::Object(...)`) which SurrealDB's content serializer rejects — it expects either a
  concrete `#[derive(Serialize)]` type or its native `surrealdb::value`.

## Root cause

A type-impedance mismatch at the store seam: `serde_json::Value`'s enum encoding is not what
SurrealDB's `.content()` accepts. The defect is passing `Value` straight into `.content()`.

## Fix

Write/read via a **parametrized SurrealQL query** binding the JSON as a `$data` param, which
SurrealDB accepts and stores as a document, and project the row back to `serde_json::Value`
on read. Keeps the store API in `serde_json::Value` (the host's lingua franca) without
leaking SurrealDB types upward. See `rust/crates/store/src/{write,read}.rs`.

## Verification

`cargo test -p lb-store` — both isolation tests pass (output in the session doc).

## Prevention

The two isolation tests are the regression guard (they were failing-before / passing-after).
Guardrail: the store crate's public API is `serde_json::Value` only; the SurrealDB-specific
encoding stays inside `write.rs`/`read.rs`, so this mismatch can't leak to callers. If a
typed-record API is added later, it serializes a concrete type and avoids the `Value` path
entirely.
