# Extensions — SDK / Devkit (session)

- Date: 2026-06-28
- Scope: ../../scope/extensions/ext-sdk-scope.md
- Stage: S10 — extension developer experience
- Status: in-progress (full workspace gate blocked on unrelated agent compile break)

## Goal

Build the local extension SDK end to end: shared signing, scaffold/build/inspect, local-only
`devkit.*` bridge, server-side publish from the node's key, and a built-in Studio view that proves the
generated templates are real.

## What changed

- Added `lb-devkit` (`rust/crates/devkit/`) with shared artifact signing, template metadata,
  root-safe scaffolding, real cargo/pnpm builds behind the single `Toolchain` trait, and inspect.
- Made `lb-pack` a thin wrapper over `lb-devkit::sign_artifact` and the shared dev publisher key
  helpers.
- Added host `devkit.*` verbs (`templates`, `scaffold`, `inspect`, `build`) under
  `rust/crates/host/src/devkit/`, gated `mcp:devkit.<verb>:call`.
- `devkit.build` now creates a durable job, runs the real toolchain, and streams JSON-string log
  lines over the existing generic bus subject `devkit/build/<job_id>`.
- Extended `ext.publish` so signed native artifacts publish through the same verify/cache/catalog path
  and then install through the existing supervised native service.
- Extended gateway `POST /extensions` to accept either a signed `Artifact` or `{ "path": "..." }`.
  The path shortcut resolves under `LB_DEVKIT_ROOT`, reads built bytes, signs server-side with
  `LB_DIR/keys/dev-publisher.key`, and never exposes the key to the UI.
- Added the built-in Studio shell view (`ui/src/features/studio/`) plus devkit API/SSE clients under
  `ui/src/lib/devkit/`, routed as `/studio`.
- Added focused host/devkit/UI tests for signing, scaffold, build, cap-deny, allow-root deny,
  workspace-isolation, real-gateway Studio flow, and load-bearing scaffold→build→publish e2e.

## Decisions & alternatives

- Server-side devkit publish reuses `POST /extensions` instead of adding `devkit.publish`, because the
  scope explicitly keeps publish on the existing extension lifecycle path.
- Native publish branches inside `ext_publish` after verification/cache/catalog, so both tiers share
  one trust model. The native branch delegates to the existing `install_native` supervisor path instead
  of inventing another loader.
- Build logs are JSON strings on the generic bus. The gateway's `/bus/stream` emits JSON payloads, so
  raw bytes would parse as `null`.
- Gateway tests set `LB_DEVKIT_ROOT` to `rust/extensions` so generated templates build in the real repo
  layout and do not silently use `ui/rust/extensions`.

## Tests

Green focused output:

```text
$ cargo test -p lb-devkit -p lb-pack
running 2 tests
test builds_generated_native_with_real_cargo ... ok
test builds_generated_wasm_with_real_cargo_when_target_is_available ... ok

running 4 tests
test rejects_symlink_escape_before_writing ... ok
test rejects_traversal_before_writing ... ok
test scaffolds_native_with_native_recipe ... ok
test scaffolds_wasm_with_manifest_caps_and_ui ... ok

running 2 tests
test reuses_existing_publisher_seed ... ok
test signs_artifact_the_registry_verifies ... ok

test result: ok. 8 passed; 0 failed
```

Focused host output:

```text
$ cargo test -p lb-host --test devkit_test --test devkit_e2e_test -- --nocapture
running 2 tests
test scaffold_build_publish_native_then_call_sidecar ... ok
test scaffold_build_publish_wasm_then_call_tool ... ok
test result: ok. 2 passed; 0 failed

running 5 tests
test scaffold_without_grant_is_denied_and_writes_nothing ... ok
test templates_requires_grant ... ok
test each_devkit_mcp_verb_denies_without_its_grant ... ok
test build_refuses_path_outside_allow_root_before_job_record ... ok
test build_job_record_is_workspace_scoped ... ok
test result: ok. 5 passed; 0 failed
```

Compile/type checks after the Studio/backend wiring:

```text
$ cargo check -p lb-host -p lb-role-gateway
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.06s

$ pnpm exec tsc --noEmit
<no output; exit 0>

$ pnpm exec vitest run --config vitest.gateway.config.ts src/features/studio/StudioView.gateway.test.tsx --reporter=verbose
✓ src/features/studio/StudioView.gateway.test.tsx > Extension Studio (real gateway) > scaffolds, builds, streams logs, publishes, and calls the generated wasm tool 8331ms
Test Files  1 passed (1)
Tests  1 passed (1)

$ pnpm test
Test Files  18 passed (18)
Tests  114 passed (114)
```

Current blocker for the remaining required gates:

```text
$ cargo build --workspace
error[E0432]: unresolved import `crate::run_events`
  --> crates/host/src/agent/run.rs:36:12
   |
36 | use crate::run_events::publish_run_event;
   |            ^^^^^^^^^^ could not find `run_events` in the crate root

$ pnpm test:gateway
error[E0432]: unresolved import `crate::run_events`
```

The error is in `rust/crates/host/src/agent/run.rs`, outside the SDK slice and owned by another active
session. Per the collision rule, this session did not patch that file.

## Debugging

- ../../debugging/extensions/devkit-build-log-bus-stream-null.md — build logs sent as raw bytes arrived
  as `null` over `/bus/stream`; fixed by publishing each line as a JSON string. Regression:
  `ui/src/features/studio/StudioView.gateway.test.tsx` waits for real SSE log lines and is green.

## Public / scope updates

- `docs/public/extensions/dev-flow.md` now describes the SDK path and Studio server-side signing.
- `docs/public/SCOPE.md` notes the SDK/Studio as in progress until the blocked workspace gates are run.
- `docs/STATUS.md` has an in-flight slice row with the current blocker.

## Dead ends / surprises

- Building generated extensions under `/tmp` bypassed the repo-relative path shape and native template
  dependencies. The tests now scaffold under the real `rust/extensions` root.
- `pnpm test:gateway -- <file>` still ran every gateway test through the package script, surfacing an
  unrelated dashboard/jsdom drag failure. Running Vitest directly isolated the Studio file but then hit
  the Rust agent compile blocker above.

## Follow-ups

- Rerun `cargo build --workspace`, `cargo test --workspace`, and `pnpm test:gateway` after the
  agent-run compile break is resolved. `cargo fmt`, focused SDK/Rust tests, focused Studio gateway
  test, TypeScript, and `pnpm test` are green.
- Promote the STATUS row from building to shipped only after the full gates are green.
