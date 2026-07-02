# Honor the stored workspace default in `agent.invoke` (runtime-id resolution) — session

- Date: 2026-07-02
- Scope: ../../scope/external-agent/agent-config-scope.md (the named follow-up: "Honor the stored
  default in `agent.invoke` when `runtime` is omitted") — resolves the last open item of that scope.
- Builds on: ./agent-config-settings-session.md (the record + verbs + Settings UI that shipped the
  selection but did NOT yet wire the invoke path).
- Stage: post-S10 (agent-config follow-up slice).
- Status: **done** — the precedence seam is real end to end; the endpoint-override is an explicitly
  named, deferred follow-up (below), not a silent gap.

## Goal — the selection actually takes effect on a run

`agent.config.set` let an admin persist a workspace default runtime, but `agent.invoke` / the channel
`/agent` path ignored it: with `runtime` omitted they fell back to the registry default (`"default"` =
the in-house loop). So picking `open-interpreter-default` in Settings → Agent did nothing on a run.
This slice closes that: **an omitted `runtime` now resolves the workspace's stored default.**

Exit gate (restated): with the workspace default set to `open-interpreter-default`, a run that names
no runtime (the channel `/agent` command, or `agent.invoke` with no `runtime`) dispatches that runtime
— and a stored id the node can't currently run falls back to the registry default rather than erroring.

## What shipped

### The ONE resolution seam (best long-term, not a symptom patch)

`rust/crates/host/src/agent/resolve_default.rs` — a single function
`resolve_effective_runtime(node, registry, ws, explicit) -> Option<String>` (one responsibility per
file, FILE-LAYOUT). Precedence, in one place, no second copy:

1. **explicit** `runtime` arg → returned verbatim (the registry still errors on a named-unknown id, so
   an explicit typo is never a silent downgrade — the decided rule is unchanged);
2. else the workspace's persisted **`agent.config.default_runtime`** — *if the node's registry offers
   it now*;
3. else the **registry default** (returned as `None`, which `RuntimeRegistry::resolve(None)` maps to
   the in-house loop).

**Registry drift is fail-open:** a stored id the node no longer offers → registry default + a `warn!`
(a run is never errored because a workspace's stored choice went away). A store read that itself fails
is treated as "unset" (best-effort — never fail a run on a transient read hiccup).

Wired into `invoke_via_runtime` (`agent/dispatch.rs`) — the ONE place runtime selection already
happened — **after** the invoke gate (`authorize_invoke`) and **before** `registry.resolve`. Because
BOTH entrypoints reach a run through `invoke_via_runtime`:

- `agent.invoke` (routed) → `agent/serve.rs::run_one` → `invoke_via_runtime`
- the channel `/agent` worker → `channel/agent_worker.rs::drive_run` → `invoke_via_runtime`

…they resolve identically with **no second copy** of the precedence logic. The channel worker's
existing opaque-deny pre-check (`registry.resolve(Some(id)).is_err()`) only fires when a runtime is
named explicitly, so the stored-default resolution (which runs only when `runtime` is `None`) does not
interact with it.

`get_agent_config` (the raw store read) is re-exported from `agent/config/mod.rs` so the seam reads the
record directly — this is the **host resolving its own dispatch**, not a caller-facing read, so it
requires no `mcp:agent.config.get:call` and confers nothing.

### Security invariant intact (no widening)

Choosing/deriving a runtime is an **argument**, not a grant. `mcp:agent.invoke:call` stays the only
gate and fires FIRST (an unauthorized caller is refused before any config is read); every tool the run
calls is still re-checked under the derived `agent ∩ caller` principal. A test asserts the gate still
denies without the cap even with a stored default present.

## Decision: the `model_endpoint` override is a NAMED, deferred follow-up (not this slice)

**Question (scope/prompt item 2):** should the stored `model_endpoint` also override the runtime's
endpoint at invoke time?

**Decision: ship runtime-id resolution now; defer the per-workspace endpoint override as an explicit
follow-up.** Rationale (the alternative I rejected, and why):

- Today each runtime is constructed **at boot** with a fixed endpoint (`node/src/external_agent.rs`
  `install` builds the `AcpRuntime` entries from `default_model_endpoint()`). The registry holds
  fully-built runtime objects; `invoke_via_runtime` selects one **by id** and calls `runtime.run(ctx)`.
  `RunContext` has no endpoint field.
- Honoring a per-workspace endpoint at invoke time therefore means threading a per-run endpoint through
  the **stable `AgentRuntime::run` seam** + `RunContext` + the external-agent role crate's wrapper
  command builder (or rebuilding a per-workspace runtime). That touches a stable boundary and the
  external-agent crate — clearly **more than a small change**, and exactly the "SDK/WIT-ish boundary,
  stop and confirm" case.
- **Rejected alternative:** quietly rebuild a runtime per invoke from the stored endpoint. It would
  double the runtime-construction path, put endpoint-selection logic in the hot invoke path, and risk
  drifting from the boot-time construction — the opposite of "one place selects the runtime". Not
  worth it for a slice whose job is runtime-*id* resolution.

So the endpoint stays **display/record data** for now (the Settings UI shows it; the boot endpoint is
what actually runs). The override is filed as a named follow-up in the scope + SKILL + public, not a
silent gap. This matches the scope's own framing (the record was designed names-only, endpoint wiring
its own slice).

## Tests (rule 9 — real infra, seeded via the real write path, NO mocks / NO fake backend)

### Backend — `rust/crates/host/tests/agent_default_runtime_test.rs` (5, green)

Boots a **real** `Node`; the stored default is seeded via the **real** registry-validated write path
(`agent_config_set`); a deterministic stub `AgentRuntime` (a runtime trait-object the seam abstracts
over — NOT a mocked backend) stands in for an external engine so "the stored runtime ran" is
observable vs "the default ran".

- `absent_runtime_uses_the_stored_workspace_default` — omitted `runtime` → the stub runs (`external-ran`).
- `explicit_runtime_overrides_the_stored_default` — explicit `default` beats the stored stub.
- `a_stored_but_unavailable_default_falls_back_to_the_registry_default` — seed a valid stub default,
  then "drift" the node (re-install a default-only registry): the run **falls back**, never errors.
- `workspaces_are_isolated_for_the_stored_default` — ws-A's stored default never affects a ws-B run.
- `invoke_is_still_denied_without_the_cap_even_with_a_stored_default` — the gate still denies (the
  resolution widens nothing).

```
# cargo test -p lb-host --test agent_default_runtime_test
running 5 tests
test invoke_is_still_denied_without_the_cap_even_with_a_stored_default ... ok
test explicit_runtime_overrides_the_stored_default ... ok
test a_stored_but_unavailable_default_falls_back_to_the_registry_default ... ok
test absent_runtime_uses_the_stored_workspace_default ... ok
test workspaces_are_isolated_for_the_stored_default ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.27s

# regression guard — the seam + config + worker paths still green together
agent_config_test        6 passed
agent_runtime_seam_test  5 passed
channel_agent_worker_test 7 passed
# full `cargo test -p lb-host` suite: all green (no regressions). `cargo fmt --check` clean.
```

### Frontend — `ui/src/features/settings/AgentDefaultRuntime.gateway.test.tsx` (1, green)

Against a **real spawned gateway** (default-only test node): an admin sets the workspace default via the
real Settings → Agent UI (`agent.config.set`), `agent.config.get` reflects it, then a `kind:"agent"`
channel item is posted **with the `runtime` field OMITTED** (asserted: the body carries no `runtime`
key) and driven through the real host reactor (`drainAgentRuns`). The run resolves the stored default
(`default`) and posts a durable `agent_result` (`"runtime":"default"`), NOT an opaque `agent_error` —
proving the omitted-runtime path is honored over the wire.

> The test gateway is default-only, so the observable stored default here is `"default"`; the
> *distinguishing* precedence (a stored EXTERNAL id selecting a different engine, and drift-fallback) is
> proven in the backend test against the registered stub. Over the gateway we prove the **wiring**.

```
# pnpm test:gateway AgentDefaultRuntime.gateway
 ✓ src/features/settings/AgentDefaultRuntime.gateway.test.tsx (1 test)
 Test Files  1 passed (1)
      Tests  1 passed (1)
# SettingsView.gateway (the sibling suite) — 3 passed, no regression.
```

## Live verification (the "working in the UI" acceptance)

Built `cargo build -p node --features external-agent`; booted with the provided Z.AI GLM-4.6 key as
`ZAI_API_KEY` and `interpreter 0.0.17` on PATH. Verified over the **live gateway** (`lb`, remote):

- Boot print: `external-agent: runtimes installed = ["codex-default","default","open-interpreter-default","vtcode-default"]`.
- `lb call agent.runtimes '{}'` → lists `open-interpreter-default`.
- `lb call agent.config.set '{"patch":{"default_runtime":"open-interpreter-default","model_endpoint":{…}}}'`
  → `{ok:true}`; `lb call agent.config.get` reads it back verbatim (names-only endpoint).
- **Real external subprocess through the resolved seam:**
  `EXTAGENT_SMOKE=1 ZAI_API_KEY=… cargo test -p lb-role-external-agent --test smoke_test -- --nocapture`
  → `external agent (open-interpreter-default) answered: "PONG"` (a live `interpreter` subprocess over
  Z.AI GLM-4.6 drove to an answer through `AcpRuntime`).

**Honest gap in the live path (not a code gap in THIS slice):** the demo `node/src/main.rs` does not yet
wire `agent.invoke` as a callable path (the `serve_agent` "serve-wiring TODO" in
`node/src/external_agent.rs`), so a full channel `/agent` run over the live gateway couldn't be driven
by `lb` here (`agent.invoke` returns a routing error, not a cap deny — confirmed in the node telemetry).
The **resolution seam** it would use is nonetheless proven: by the backend test (omitted runtime →
stored external stub) and the UI gateway test (omitted runtime → stored default → durable answer). When
the serve-wiring TODO lands, no change to this seam is needed — both entrypoints already route through
`invoke_via_runtime`.

## Debugging

- `docs/debugging/frontend/gateway-node-modules-symlink-corruption.md` — a `git worktree` I created to
  compare against the pre-change commit symlinked its `node_modules` into `ui/node_modules`; removing
  the worktree left 24 dangling top-level symlinks (vite/vitest/react/…) → module-not-found on
  `pnpm test:gateway`. Fixed by `rm -rf ui/node_modules && pnpm install`. Not a product bug; logged so
  the symptom (`Cannot find module 'vite'`) maps to the cause for the next person.
- **Pre-existing (NOT this slice):** `CommandPalette.agent.gateway.test.tsx` "renders the runtime
  dropdown + goal, and settles to an answer" fails at the arg-rail focus step (`findByLabelText("runtime")`
  after committing the goal field). **Exonerated:** with my resolution wiring temporarily bypassed the
  test failed identically, and I touched nothing in `CommandPalette.tsx` / `agent.runtimes`. Logged as a
  known-failing UI-interaction flake, not introduced here. The sibling `is capability-filtered` case in
  the same file passes.

## Follow-ups (named, not silent gaps)

- **Per-workspace `model_endpoint` override at invoke time** — deferred with rationale above (touches the
  stable `AgentRuntime`/`RunContext` seam + the external-agent wrapper; its own slice).
- **Serve-wiring** — boot `serve_agent` (and a callable `agent.invoke`) from the node binary so the live
  channel `/agent` run is drivable over the gateway (the pre-existing `external_agent.rs` TODO).
- **Full `AgentProfile` authoring** (`granted_tools`/`persona_skill`) — still deferred (scope open Q).

## Related

- Scope: [`../../scope/external-agent/agent-config-scope.md`](../../scope/external-agent/agent-config-scope.md)
  (open question now resolved).
- Public: [`../../public/external-agent/external-agent.md`](../../public/external-agent/external-agent.md)
  ("Agent config" — follow-up promoted to shipped).
- Skill: [`../../skills/external-agent/SKILL.md`](../../skills/external-agent/SKILL.md) (§4/§5 — the
  precedence is now real).
- Prior session: [`agent-config-settings-session.md`](./agent-config-settings-session.md).
