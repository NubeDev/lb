# CI never builds the proof-panel guest — ~17 tests fail on a missing .wasm

**Date:** 2026-08-05 · **Status:** fixed (`.github/workflows/ci.yml`) · **Surfaces:** CI, extensions, wasm

## The symptom

The `build-and-test (host)` shard failed on every run with ~17 tests panicking on a file that was
never built:

```
panicked at crates/host/tests/proof_panel_test.rs:59:9:
missing component at .../extensions/proof-panel/target/wasm32-wasip2/release/proof_panel_ext.wasm
Build it first: bash rust/extensions/proof-panel/build.sh
```

Affected `proof_panel_*`, `simulate_*`, `callback_*`, and `store_*` tests. The panic message names
the exact fix, and it was still red for weeks — the shard had other genuine failures in it, so one
more red shard carried no new signal.

## The cause

Not a code bug — a **workflow gap**. `ci.yml` hand-listed exactly two wasm guests:

```yaml
- name: Build the wasm guest (the S1 hello component)
  working-directory: rust/extensions/hello
- name: Build the wasm guest v2 (the S2 hot-reload swap target)
  working-directory: rust/extensions/hello-v2
```

`proof-panel` was added later as a third component the tests load, and nothing updated CI. Locally
it passes for anyone who has ever run its `build.sh`, because the artifact lingers in the target
dir — so the gap is invisible outside a clean checkout.

## The fix

Added the missing build step. Verified locally: `proof_panel_test` goes from failing to
**22 passed, 1 failed**, where the remaining failure is only the *local* absence of `hello_ext.wasm`
(which CI does build).

## Why not glob `extensions/*/build.sh`

Tempting, and wrong: the devkit e2e tests generate throwaway `devkit-e2e-wasm-*` extension
directories in that tree, so a glob would build test litter. The list stays hand-maintained, with the
command to re-derive it recorded in the workflow next to the steps:

```
grep -rhoE 'extensions/[a-z0-9-]+/target/wasm32-wasip2/release/[a-z0-9_]+\.wasm' \
  rust/crates/*/tests/ rust/role/*/tests/ | sort -u
```

Today that returns exactly three: `hello`, `hello-v2`, `proof-panel`.

## Rules

- **A test that loads a build artifact needs a CI step that builds it.** The dependency is real but
  invisible to cargo, so nothing enforces it — only the hand-maintained list does.
- **When a panic message tells you the fix, the problem is not diagnosis — it is that nobody is
  reading the run.** A permanently-red suite is what let a self-describing failure survive for weeks.
- **Don't glob a directory that tests write into.** Derive the list from what the tests actually
  load, and record the derivation next to the list.
