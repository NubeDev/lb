# External agent flails in its own shell, then burns the 15-min wall

**Symptom.** A dock run (`builtin.extension-builder`, "write me a hello world extension and build
and load it and e2e test it") ran ~10 min doing nothing useful and ended with
`agent run exceeded its time limit`. The live tool-call capture showed **30+ `exec: /bin/bash`
calls and ZERO node MCP calls** — `make dev` (×3, exit 2), `cargo build --release` on the repo,
`chmod -R +w rust/extensions/` (exit 1, read-only), then a hand-written `Cargo.toml` +
`extension.toml` + `build.sh` and a `cargo build --target wasm32-wasip2` that died exit 101.

**Root cause (the load-bearing one).** The wrappers' MCP config points the agent's MCP-server child
at the shim by the **bare name `lb-mcp-shim`**, assuming it's on `PATH`. On a `make dev` / `cargo run`
node the shim is a **sibling binary in the target dir**, NOT installed on `PATH`. `register()` never
called `with_shim_bin`, so the agent's MCP child failed to spawn → the agent had **no host tools** →
it fell back to authoring the extension by hand in its own shell → `make dev`/`cargo` flailing →
stall.

**Contributing.** (a) The external run was bounded **only by wall-time** — the in-house loop
self-bounds at `MAX_STEPS`, but a flailing subprocess had nothing to reap it early, so it consumed the
whole `RUN_WALL_CEILING` (15 min) before failing. (b) The `extension-authoring` skill did not fence
"never `make dev`/`cargo build` the repo; author only via `devkit.*`" prominently enough to survive
into an external-agent run.

**Fixes.**
1. `lb_role_external_agent::register` now resolves the shim **next to the node binary**
   (`<current_exe_dir>/lb-mcp-shim`, `resolve_shim_bin()`), falling back to the PATH name only if
   absent — so the bridge actually works on a dev node. (`role/external-agent/src/lib.rs`.)
2. **No-progress (stall) ceiling** in `AcpRuntime::run`: a watchdog on the `RunEvent` stream reaps a
   run that emits nothing for `NO_PROGRESS_CEILING` (90s), dropping the `drive` future (reaping the
   subprocess, same seam as the wall). Closes the open item in `run-lifecycle-scope.md` (external runs
   were wall-time-only). **Updated the same day to PAUSE-AND-ASK** (see
   `sessions/external-agent/agent-stall-pause-and-ask-session.md`): instead of marking the job `Failed`,
   the stall now `suspend`s the run (resumable) and returns `AgentError::Stalled`; the worker posts a
   durable `kind:"agent_stalled"` item and the dock renders a "Keep going" (resume) / "Stop" (cancel)
   prompt. The run is no longer a dead end — the user decides.
3. Skill fence: a "STOP — read before any shell command" block at the top of
   `docs/skills/extension-authoring/SKILL.md` forbidding `make dev`/`cargo build`/repo edits and
   mandating the devkit MCP flow; notes an external run that only shells is reaped as stalled.

**Regression test.** `role/external-agent/tests/no_progress_test.rs` —
`stalled_run_is_reaped_at_the_no_progress_ceiling`: a real `sh -c 'sleep 30'` subprocess via a silent
scripted wrapper is reaped in <1s (250ms ceiling) with `STALL_MESSAGE` and a `Failed` job, instead of
running to the 30s sleep / 600s liveness timeout.

**Not a bug (worked as designed).** The 15-min wall reap itself is the supervision layer doing its
job — the honest `agent_error` is correct. The defect was everything upstream that made the run
*need* reaping.
