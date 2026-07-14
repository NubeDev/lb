# Session: native-artifact upload limit on `POST /extensions`

- Date: 2026-07-14
- Area: extensions / gateway
- Scope touched: lifecycle-management (the `ext.publish` HTTP upload path)
- Debugging entry: ../../debugging/extensions/native-artifact-upload-413-body-limit.md

## The ask

Fix the gateway's `POST /extensions` upload rejecting real native-extension artifacts with HTTP 413
(body limit exceeded), which made extension publish/hot-load unusable for native Tier-2 sidecars.
An embedder (NubeIO/ems) publishing its Modbus sidecar (a ~50 MB artifact for a 6.2 MB binary; the
full ems bundle is ~317 MB) hit `413 "length limit exceeded"`. The same artifact boot-installs fine.

## What shipped

1. **`BootConfig::max_extension_upload_bytes: u64`** (default `DEFAULT_MAX_EXTENSION_UPLOAD_BYTES` =
   384 MiB) — sized to the ~317 MB ems bundle with headroom, bounded so a runaway upload can't OOM
   the node. `from_env` reads `LB_MAX_EXTENSION_UPLOAD_BYTES` (unset/malformed ⇒ default with a
   warning). Read only at the binary boundary; an embedder fills the field directly.
   (`rust/node/src/config.rs`, re-exported from `rust/node/src/lib.rs`.)

2. **Threaded to the gateway** via a new `Gateway::with_max_extension_upload_bytes` builder + a
   `Gateway.max_extension_upload_bytes` field (default equal to the config default). `boot_full`
   applies `cfg.max_extension_upload_bytes` in the `GatewayMode::Addr` arm.
   (`rust/role/gateway/src/state.rs`, `rust/node/src/builder.rs`.)

3. **Route-scoped body limit** — `router` reads the value and sizes the `DefaultBodyLimit` on
   `POST /extensions` to `limit + 16 MiB margin`. Still route-scoped (rule 10 — never a global
   bump). (`rust/role/gateway/src/server.rs`.)

4. **Descriptive over-limit error** — `publish_extension` checks the declared `Content-Length`
   against the configured limit and returns a descriptive 413 ("extension artifact 480.0 MiB
   exceeds the upload limit 384.0 MiB …") for a body that clears the layer margin. A body past
   `limit + margin` still hits the layer's bare 413 (the OOM backstop).
   (`rust/role/gateway/src/routes/ext.rs`.)

## Design notes / rejected alternatives

- **Global body-limit bump — rejected.** Rule 10: don't widen a global chokepoint. The limit is
  scoped to the one upload route.
- **Unlimited — rejected.** A 317 MB in-RAM buffer per upload is a DoS surface; the ceiling is
  bounded and configurable, not removed.
- **Stream-to-disk — deferred.** The handler decodes `Json<Artifact>` and `ext_publish` takes an
  in-memory `Artifact`; streaming the body to the install dir is a larger change (chunked upload
  verb + file-backed artifact). Shipped the bounded limit now; TODO noted in the debugging entry.
- **Descriptive error via a header preflight, not a handler-side re-buffer.** The axum layer rejects
  before the handler for a truly oversized body, so the descriptive path uses the layer margin +
  the `Content-Length` header (what curl/the browser send) to land the just-oversized case in the
  handler.

## Tests (all green)

`rust/role/gateway/tests/publish_install_test.rs`:
- `publish_accepts_a_body_larger_than_the_2mib_default_native_binary_size` (pre-existing, still green)
- `a_body_over_the_configured_limit_is_413_with_a_descriptive_error` — pins a 1 MiB ceiling
  (`with_max_extension_upload_bytes`), a ~2 MiB `wasm` field → descriptive 413 with size + limit.
- `a_body_under_the_configured_limit_clears_the_size_gate` — same pinned ceiling, a real hello-v2
  artifact clears the size gate (not 413).
- Existing cap-deny (`publish_without_the_capability_is_denied_server_side`) + workspace-isolation
  (the untrusted-publisher 422 + the trusted-publish-then-callable flow) tests unchanged and green.

`rust/node/tests/embed_test.rs::from_env_defaults_match_the_binary` — asserts the 384 MiB default.

```
running 6 tests (publish_install_test) ... test result: ok. 6 passed
```

## Follow-ups / notes for the PR

- **Tag a `node-v` release when this lands** — unblocks `make publish-modbus` for every embedder
  (ems, cc-app, rubix-ai); note it in the PR so ems can bump its pin.
- **Streaming-to-disk** for the artifact body (avoid the in-RAM buffer) — separate issue.
- **Separate embedder bug (NubeIO/ems, not lb):** the ems `publish-modbus` recipe logs in as
  `{"user":"dev"}` and 403s — it should use a seeded admin credential. Mention in the PR, don't fix
  here.

## Build caveat during this session

The workspace-wide `cargo build`/`cargo test` was transiently red due to a **concurrent, unrelated
in-flight edit** in `lb-host` (a "viz grafana-parity-backend / P1" feature adding `query_options`,
`transparent`, `links` to `dashboard::model::Cell` with some constructor sites not yet updated —
`docs/scope/viz/`, `rust/crates/host/tests/dashboard_query_options_test.rs`). That breakage is not
part of this change and was left untouched. The gateway tests here were run and passed against a good
build; the only file that couldn't be re-run through the broken host tree is the `lb-node` embed
default assertion (its own lines compile clean — every build error is in the concurrent host edit).
