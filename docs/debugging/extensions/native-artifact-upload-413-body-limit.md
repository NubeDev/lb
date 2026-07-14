# `POST /extensions` rejects a real native artifact with 413 (body limit)

- Area: extensions
- Status: resolved
- First seen: 2026-07-14
- Resolved: 2026-07-14
- Session: ../../sessions/extensions/native-artifact-upload-limit-session.md
- Regression test: rust/role/gateway/tests/publish_install_test.rs
  (`a_body_over_the_configured_limit_is_413_with_a_descriptive_error`,
  `a_body_under_the_configured_limit_clears_the_size_gate`,
  `publish_accepts_a_body_larger_than_the_2mib_default_native_binary_size`)

## Symptom

An embedder (NubeIO/ems) publishing its native Modbus sidecar over the HTTP upload path got:

```
POST <gateway>/extensions --data-binary @modbus.artifact.json
→ HTTP 413 "Failed to buffer the request body: length limit exceeded"
```

Modbus is forced native (RTU serial + TCP + timers are denied to WASM). Its `lb-pack` artifact
is base64 binary inside JSON — ~50 MB for a 6.2 MB release binary. The SAME extension installs
fine via the boot-install path (`install_native` / `load_enabled`): a 317 MB ems sidecar artifact
loads at boot with no limit. **Only the HTTP upload path was capped** — the asymmetry was the bug:
two install paths, one silently unusable for real binaries.

## Cause

`POST /extensions` had a route-scoped `DefaultBodyLimit::max(32 * 1024 * 1024)` (32 MiB). That was
already a bump over axum's 2 MiB default (added when the first native ext published over the wire),
but 32 MiB is still far below a real native artifact. The boot path buffers nothing over HTTP, so it
never hit the limit.

## Fix

- The ceiling is now `BootConfig::max_extension_upload_bytes` (default **384 MiB**, sized to the
  ~317 MiB ems bundle with headroom, bounded so a runaway upload can't OOM the node). Read at the
  binary boundary from `LB_MAX_EXTENSION_UPLOAD_BYTES` in `BootConfig::from_env`; an embedder fills
  it directly. Threaded to the gateway via `Gateway::with_max_extension_upload_bytes` in `boot_full`.
- The route-scoped `DefaultBodyLimit` is sized from that value **plus a 16 MiB margin**, so a body
  that is only just over the semantic ceiling still reaches `publish_extension`, which returns a
  **descriptive 413** ("extension artifact 480.0 MiB exceeds the upload limit 384.0 MiB …") instead
  of the layer's bare "length limit exceeded". A body past `ceiling + margin` is bounced by the
  layer — the true-abuse / OOM backstop.
- Still **route-scoped** (rule 10 — never a global body-limit bump), no engine/tier code branch.

## Why not stream-to-disk

Buffering a 317 MB artifact fully in RAM per upload is its own DoS surface. Streaming the body
straight to the install dir would remove it, but the current handler decodes `Json<Artifact>`
(the same wire shape the registry-host accepts) and `ext_publish` takes an in-memory `Artifact`.
Streaming is a larger change (a new chunked upload verb + a file-backed artifact) and is left as a
follow-up; the bounded 384 MiB ceiling caps the in-RAM exposure in the meantime.

## Related

- The ems `publish-modbus` recipe logs in as `{"user":"dev"}` and 403s — an embedder-side recipe
  bug (should use a seeded admin credential), tracked in NubeIO/ems, not lb. Separate from the 413.
