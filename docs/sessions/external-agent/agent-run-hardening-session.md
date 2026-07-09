# Session — hardening the external-agent run against shell-flail / stall

**Date:** 2026-07-08 · **Topic:** external-agent / run-lifecycle · **Status:** shipped (code green)

## The ask

An `extension-builder` dock run crashed while coding a hello-world extension. The transcript showed
the run being killed at the wall ceiling after ~10 min of pure `/bin/bash` (`make dev`, `cargo build`,
`chmod` the repo) with **zero node MCP calls**. Harden the external-agent run so this fails fast and
honestly instead of hanging.

## Diagnosis

Not a crash — the 15-min `RUN_WALL_CEILING` reap worked. The defect was upstream: the agent had **no
host tools**, so it hand-rolled the extension in its own shell and stalled. Root cause chain:

1. **The MCP bridge shim was unreachable.** The codex-family wrapper's config points the agent's
   MCP-server child at `command = "lb-mcp-shim"` (bare PATH name). On a `make dev`/`cargo run` node the
   shim is a sibling binary in the target dir, **not on PATH**, and `register()` never called
   `with_shim_bin`. The MCP child failed to spawn → no host tools → shell fallback.
2. **External runs were bounded only by wall-time.** The in-house loop self-bounds at `MAX_STEPS`; the
   external subprocess had no early-reap for *no progress*, so a flailing run consumed the whole
   ceiling. (`run-lifecycle-scope.md` flags this exact gap open.)
3. **Grounding.** The authoring skill didn't fence "never `make dev`/`cargo build` the repo; author via
   `devkit.*`" prominently enough to survive into an external run.

## What shipped

- **Shim resolution** (`rust/role/external-agent/src/lib.rs`): `resolve_shim_bin()` finds
  `<current_exe_dir>/lb-mcp-shim` and `register()` wires it via `with_shim_bin`, PATH fallback. The
  bridge now actually works on a dev node.
- **No-progress (stall) ceiling** (`AcpRuntime::run`): a `tokio::sync::Notify` bumped by the publisher
  on every streamed `RunEvent`; `drive` is raced against a watchdog that fires after `NO_PROGRESS_CEILING`
  (90s) of silence. On stall the run future is dropped (reaps the subprocess, same seam as the wall),
  the job is marked `Failed`, and the distinct `STALL_MESSAGE` is returned. New
  `with_no_progress_ceiling` + `from_resolved` test seams.
- **Distinct message**: `STALL_MESSAGE` ("agent run stalled: no progress …") is separate from the
  worker's wall-time `TIMEOUT_MESSAGE` and the `OPAQUE_DENY`, so a *stuck* run reads differently from a
  *slow* one or a *denied* one — surfaced honestly via `agent_worker`'s `other => "agent run failed:
  {other}"` path.
- **Grounding fence** (`docs/skills/extension-authoring/SKILL.md`): a top-of-file "STOP — read before
  any shell command" block forbidding `make dev`/`cargo build`/repo edits, mandating the devkit MCP
  flow, and noting an external run that only shells is reaped as stalled.

### On "run until the user says stop"

The user raised: a stuck run should surface to the user rather than silently die. Noted as the right
direction; the machinery already supports it (`Suspended`/resumable + dock stop controls). This session
shipped the *fast honest failure* (stall → `Failed` + distinct message). **The pause-and-ask
follow-up shipped the same day** (`agent-stall-pause-and-ask-session.md`): the stall now suspends the
run and posts an actionable "Keep going / Stop" dock prompt instead of failing — so the assertions
below (`Failed` job, `STALL_MESSAGE`) were superseded by that session.

## Tests

- `rust/role/external-agent/tests/no_progress_test.rs::stalled_run_is_reaped_at_the_no_progress_ceiling`
  — real `sh -c 'sleep 30'` silent subprocess, 250ms ceiling, reaped in <1s with `STALL_MESSAGE` and a
  `Failed` job. **Green.**
- `cargo test -p lb-role-external-agent` — all green (10 tests incl. swap/scratch isolation).
- `cargo build -p node --features external-agent` — green.

## Debugging entry

`docs/debugging/external-agent/agent-flails-in-shell-then-stalls.md` (+ README index row).

## Open / follow-ups

- Turn the stall reap into a **pause-and-ask** resumable suspension (user decides keep-going vs stop),
  per the user's "run until the user says stop" note.
- The related **publish-does-not-grant-tool-call-caps** gap (separate 2026-07-08 entry) still blocks the
  WASM-tool E2E (G5b) — unrelated to this fix but on the same run's happy path.
- Consider an **iteration/token** ceiling for the external run distinct from wall-time (the other half
  of the `run-lifecycle-scope.md` open item; this session did the no-progress half).
