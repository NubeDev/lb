# Real-engine `control-engine.tree` fails "LB_GATEWAY_URL is not set" after S4

- **Area:** extensions (control-engine)
- **Date:** 2026-07-02
- **Status:** resolved
- **Slice:** control-engine S4 ([session](../../sessions/control-engine/ce-v1-s4-session.md))

## Symptom

The opt-in real-engine tier (`control_engine_against_real_ce_studio`, run against a live ce-studio on
`:7979` with NO `LB_CE_FAKE`) regressed after S4: `control-engine.tree` failed with
`Extension("... host callback failed: no callback address: LB_GATEWAY_URL is not set")`. S3 passed this
same test.

## Root cause

S4 routes every graph verb through `resolve_base(selector)`, which builds a `HostCtx` (the
`lb-sidecar-client` callback) to read the `ce_appliance` registry. But the real-engine dev tier runs the
sidecar **without a gateway**, so `SidecarClient::from_env()` (inside `HostCtx::from_env()`) fails at
construction with `NoGateway` — *before* `resolve` ever runs. That construction error propagated instead
of falling back to the S3 behaviour (treat the selector as a literal `host:port` base). The
store-unreachable fallback in `resolve` never got a chance to fire.

## Fix

In `serve::resolve_base`, treat a `HostCtx::from_env()` failure the same as a store-unreachable registry:
fall back to the literal selector as a base.

```rust
let host = match HostCtx::from_env() {
    Ok(h) => h,
    Err(_) => return Ok(selector.to_string()), // no gateway → no registry → literal base (S3 behaviour)
};
```

This cannot leak workspace isolation: with a real gateway present, `HostCtx::from_env()` succeeds and the
registry lookup runs — an unknown/other-ws id then returns `Ok(None)` → not-found, never a literal base.
The literal fallback is reachable ONLY when the sidecar has no gateway at all (the dev tier).

## Regression test

`control_engine_against_real_ce_studio` (opt-in, `#[ignore]`, `CE_ENGINE_URL=127.0.0.1:7979`) — fails
before the fix, passes after. The gateway-present resolution path (and its isolation not-found) is covered
by `extensions/control-engine/tests/appliance_registry_test.rs`.
