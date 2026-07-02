# Reminder row controls inert: `bad input: missing u64 arg: ts` (write verbs hard-required `ts`)

**Symptom (real-gateway reminders control e2e):** the channel `/reminders` interactive table rendered
and the pause switch / Run now / Delete buttons fired the bridge, but every write was rejected at the
host before any effect:

```
FIRE(no ts)    out=null  err= bad input: missing u64 arg: ts
UPDATE(no ts)  err= bad input: missing u64 arg: ts
DELETE(no ts)  err= bad input: missing u64 arg: ts
```

`reminder.create` and `reminder.list` worked; only `update`/`delete`/`fire` failed — so the list
rendered but every row control was inert end to end. Surfaced by the real-gateway UI loop (the seam the
row controls actually use); it was **invisible** to both green halves:

- the Rust `reminder_fire_test.rs` always passes `ts` explicitly (`json!({"id":…, "ts":5000})`), so it
  never exercised the `ts`-free bridge path;
- the UI unit tests stub the bridge, so they never reach the real verb.

## Root cause

The row controls are the 100%-backend-driven design's whole point: the `reminder.list` descriptor's
`result` declares the controls' `argsTemplate` with **no `ts`** (`{id:"${id}", enabled:"{{value}}"}`
etc.) — a generic, tool-agnostic template. The shipped control views interpolate that verbatim and call
`bridge.call(tool, args)` → `invoke("mcp_call", {tool, args})`. The gateway `mcp_call` route
(`role/gateway/src/routes/mcp.rs`) forwards `args` **verbatim** — unlike the flows/workflow/datasource
routes, which fold `"ts": gw.now()` into the args. And host dispatch for `reminder.*`
(`tool_call.rs`) doesn't inject `ts` either. So the write verbs received no `ts`.

`call_reminder_tool` (`reminder/tool.rs`) then hard-required it: `update`/`delete`/`fire` used
`u64_arg(input, "ts")?`, which errors `missing u64 arg: ts` when absent — **even though `create`
already tolerated a missing `ts`** (`opt_u64(input, "ts")?.unwrap_or_else(now_ts)`). The reminder verb
family was internally inconsistent: create forgiving, the others strict.

## Fix

Make the reminder write verbs tolerant of a missing `ts`, defaulting to the host clock — exactly like
`create`. `reminder/tool.rs`, the `update`/`delete`/`fire` arms:

```rust
// was: u64_arg(input, "ts")?
opt_u64(input, "ts")?.unwrap_or_else(now_ts),
```

and removed the now-unused `u64_arg`. This keeps the frontend generic (the row-control `argsTemplate`
stays `ts`-free, as the locked contract declares) and requires no gateway/contract change. Rejected the
alternative of injecting `ts` in the gateway `mcp_call` route: it would make the generic route hold a
list of "time-based verbs", pushing tool-specific knowledge back into the transport — the opposite of
this feature's design. The `ts` is a logical `now`; the verb that owns the record is the right place to
default it (and `create` already set that precedent).

**Note the unit:** `now_ts()` returns **seconds** (the reminder clock unit — see
[ts-unit-mismatch-cron-search-limit.md](ts-unit-mismatch-cron-search-limit.md)), so the default is
correct for the cron math.

## Regression test

`reminder_fire_test.rs`: `write_verbs_default_ts_when_absent` — call `reminder.update`,
`reminder.delete`, and `reminder.fire` over the real node **without** a `ts` arg (the exact bridge shape
a row control sends) and assert each succeeds and has its effect (update flips `enabled`, fire produces
one action, delete tombstones) — fails before the fix with `missing u64 arg: ts`, passes after.
