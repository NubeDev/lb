# Interactive rule messaging writes collapse to `now=0` (id + ts)

- **Area:** rules
- **Symptom:** In the rules Playground, `channel.post("abc", #{ body: "…" })` only ever
  showed ONE message — each new post replaced the last instead of appending — and the
  posted message had no timestamp, always sorting as the oldest.
- **Status:** resolved
- **Date:** 2026-07-04

## What was observed

Running `channel.post(...)` twice interactively produced one row, not two. The row's
`id` was always `rule-channel-0-0` and its `ts` was `0`.

## Root cause

The `rules.run` MCP verb derives the run's logical clock `now` from the request's `ts`
field, defaulting to `0` when absent
([`rust/crates/host/src/rules/mod.rs:140`](../../../rust/crates/host/src/rules/mod.rs)):

```rust
let now = input.get("ts").and_then(|v| v.as_u64()).unwrap_or(0);
```

`now` then flows into the messaging handles, which derive a **deterministic** id from it
(`rust/crates/rules/src/verbs/channel.rs::post`, and likewise `inbox.rs`/`outbox.rs`):

```rust
let id = map_str(&item, "id").unwrap_or_else(|| format!("rule-channel-{}-{seq}", self.now));
json!({ "cid": cid, "id": id, "body": body, "ts": self.now });
```

The determinism is intentional — a scheduled/programmatic re-run with the same inputs
must upsert rather than duplicate (`scope/rules/rules-messaging-scope.md`, "Deterministic,
idempotent writes"). The bug: the **interactive** path never supplies `ts`. The UI client
(`ui/src/lib/rules/rules.api.ts`) doesn't send it, and the gateway route
(`rust/role/gateway/src/routes/rules.rs::run_rule`) — unlike `routes/datasources.rs` /
`routes/dashboard.rs`, which thread `gw.now()` — did not inject the live clock. So `now`
collapsed to `0` on *every* interactive run: same deterministic id (`rule-channel-0-{seq}`)
→ the store upserted the same row; `ts: 0` → it sorted as the oldest message.

All three messaging handles (`channel.post`, `inbox.record`, `outbox.enqueue`) derive
their id from the same `now`, so all three carried the same latent bug — the clock fix
resolves them together.

## Fix

Inject the live wall clock at the gateway edge for `rules.run` when the caller omitted
`ts` ([`rust/role/gateway/src/routes/rules.rs::run_rule`](../../../rust/role/gateway/src/routes/rules.rs)):

```rust
if input.get("ts").is_none() {
    input["ts"] = json!(gw.now());
}
```

`Gateway::now()` returns the pinned test clock if a test fixed one, else live
unix-seconds — so tests stay deterministic and production runs get a real, advancing `now`.
The clock is injected at the edge; core crates stay clock-free (the "no wall-clock in core"
rule holds). A caller that DOES supply its own `ts` (scheduler, programmatic re-run) keeps
its deterministic, idempotent write — the fix only fills `ts` when it is missing.

## Regression test

`rust/role/gateway/tests/rules_routes_test.rs::interactive_channel_posts_append_with_distinct_ids_and_ascending_ts`:
two interactive runs (over gateways with an advancing fixed clock, sharing one node) post
one message each; a third read-only run asserts the history holds **2** distinct-id,
ascending-, non-zero-`ts` rows — and a fourth run at the SAME clock upserts (still 2 rows),
proving the idempotency contract is intact.

## Lessons

- A deterministic id derived from an injected clock is only safe when the clock actually
  advances. When the edge forgot to supply it, the "idempotent" write silently became
  "overwrite-forever" — the determinism turned a missing input into data loss.
- Match the sibling routes: `datasources`/`dashboard` already thread `gw.now()`; a new
  edge route that omits it is the tell.
