# Devkit build logs arrive as null over `/bus/stream`

- Date: 2026-06-28
- Area: extensions
- Status: resolved
- Session: ../../sessions/extensions/ext-sdk-session.md

## Symptom

`devkit.build` published log lines onto the generic bus as raw bytes. The gateway's
`GET /bus/stream?subject=...` route treats bus payloads as JSON, so a raw line such as `cargo build`
failed JSON parsing and streamed `null` to the Studio page.

## Root cause

The SDK build service reused the generic bus subject correctly, but forgot that the bus stream's wire
contract is JSON payloads. Dashboard `bus.publish` callers already send JSON; the new devkit path was
the first host producer sending plain text bytes to that route.

## Fix

`rust/crates/host/src/devkit/build.rs` now serializes every log line as a JSON string before calling
`lb_bus::publish`, and publishes explicit terminal lines:

- `devkit build: done`
- `devkit build: failed`

The Studio waits for those terminal frames instead of sleeping.

## Regression test

`ui/src/features/studio/StudioView.gateway.test.tsx` opens the real SSE stream for the returned
`log_subject`, asserts it receives real cargo log lines, and waits for `devkit build: done`.

Green focused output:

```text
✓ src/features/studio/StudioView.gateway.test.tsx > Extension Studio (real gateway) > scaffolds, builds, streams logs, publishes, and calls the generated wasm tool 8331ms
Test Files  1 passed (1)
Tests  1 passed (1)
```
