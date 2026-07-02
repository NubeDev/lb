# External agent: no runtime picker / can't select `open-interpreter-default` in the channel

**Status:** FIXED (UI). Root cause was in the command palette, NOT the node. The sandbox fix (below)
is a separate, also-applied change.

## ROOT CAUSE (confirmed this session)

The node was innocent. With `EXTAGENT=1` the boot line prints all four runtimes, and hitting the real
gateway proved the read + catalog surfaces are correct:
- `external-agent: runtimes installed = ["codex-default","default","open-interpreter-default","vtcode-default"]`
- `POST /mcp/call {"tool":"agent.runtimes"}` → `{"default":"default","runtimes":["codex-default","default","open-interpreter-default","vtcode-default"]}`
- `GET /mcp/catalog` → `agent.invoke` carries the `runtime` arg with `x-lb:{widget:"runtime"}`.

So hypothesis 1 (feature not registering) and hypothesis 2 (serve-wiring gap) were both DISPROVED —
the gateway is created with `node.clone()` (`main.rs:171`), the same `Node` `install()` populated, so
`node.runtimes()` returns the installed registry. The `TODO(serve-wiring)` note is stale/misleading.

The bug was in `ui/src/features/channel/palette/CommandPalette.tsx`. The arg rail (`activeArg`) only
ever targeted **required, unfilled** args (a required chip, then a required inline widget). The agent
command requires only `goal`; `runtime` is **optional**. So once the goal was committed, `activeArg`
went `undefined` and the `RuntimeArg` dropdown — which only renders when it is the single active arg —
**never rendered at all**. The user genuinely had no control to pick a runtime.

This reproduced as a failing real-gateway test: `CommandPalette.agent.gateway.test.tsx` types a goal,
commits it, then `findByLabelText("runtime")` — which timed out before the fix.

## FIX — TWO passes (the FIRST pass was WRONG; recorded here as a lesson)

### WRONG first pass (do not repeat)
`activeArg` was given a third tier: the first shown, optional, INLINE arg. This made the runtime picker
the "next active arg" AFTER `goal`. Two problems, both invisible to the tests as first written:
1. The palette renders exactly ONE widget (the single `activeArg`), so the picker only appeared once
   `goal` was **committed to a chip** — which requires pressing ⏎ mid-goal, a step no user takes. A user
   who types a goal and clicks send never saw a runtime control at all.
2. Even after the hidden ⏎, the picker **replaced** the goal field (single active slot).
The first e2e "passed" only because it scripted the ⏎ — it tested a path the user never uses. This is
why the fix looked green while the real screen stayed broken.

### CORRECT fix
Optional inline widgets render **PERSISTENTLY, alongside** the active arg — not through the single
active slot. `activeArg` is back to required-only (so `goal` stays the field you type into). A new
`optionalInlineArgs = args.filter(isInline && !required && shown && !== activeArg)` is mapped to
`ActiveArgWidget` in its own render block below the active-arg block, each writing into the same
`inlineVals` map `buildArgs` folds at submit. Result: pick `/agent` → the **Goal field AND the Runtime
dropdown are both visible immediately**; typing the goal (no ⏎) leaves the picker shown; send folds the
in-progress goal text + the picked runtime into the call. Optional CHIP args (plain `text` filters like
`reminder.list`'s `status`/`limit`) are unaffected (not inline). Per-kind `showIf` fields still only
render once their condition matches (`shown` gates the map).

Regression tests (rewritten for the persistent behavior — the old ones asserted the WRONG gated-by-⏎
flow): `CommandPalette.dispatch.test.tsx` "renders an optional inline widget PERSISTENTLY beside the
required arg (not gated behind ⏎)…" (types the goal without ⏎, asserts the select stays, sends both);
the real-gateway `CommandPalette.agent.gateway.test.tsx` asserts goal + runtime are both present the
instant the command is picked. Full palette suite (unit 16 + gateway 19) green.

### The REAL guard: a Playwright e2e (the jsdom suites were NOT enough)

The vitest "gateway" suite runs in **jsdom**, not a real browser — it passed against the buggy build
once and would have again, because the failure the user actually hit is a real-browser palette render.
The durable guard is `ui/e2e/channel-agent-runtime-picker.spec.ts`: it drives the BUILT shell (:4173)
against the real node (:8080, `EXTAGENT=1`), accepts `/agent`, and asserts the runtime `<select>` is
visible **immediately, alongside the goal field, WITHOUT committing the goal** (it types the goal and
re-asserts the picker is still shown — the exact thing the WRONG first fix failed), (b) lists the
node's external runtimes, and (c) lets you pick `open-interpreter-default`. NOTE: the first version of
this spec pressed ⏎ to commit the goal — that masked the bug; the spec was corrected to NOT press ⏎, so
it now fails against the wrong fix and passes against the right one. Run with `make ui-preview` (built
shell) + `make dev EXTAGENT=1` (node),
then `cd ui && pnpm exec playwright test channel-agent-runtime-picker`. Screenshot proof at
`ui/e2e/__screenshots__/channel-agent-runtime-picker.png` (Runtime dropdown = `open-interpreter-default`,
"Ready — press send to run"). Lesson: a palette-render bug needs a real-browser e2e — jsdom "gateway"
tests exercise the DATA path, not the rendered arg rail.

---

## Original handover (kept for the trail — the sandbox fix below still stands)

Do NOT re-ask the user to "click /agent and read the dropdown".

## Symptom (from the user, with screenshots)

- User starts the node exactly as:
  ```
  ZAI_API_KEY=e171bd6cf46844eeb651afa886af5d61.m3VLPilTkFJpYMms make dev EXTAGENT=1
  ```
- In a channel, a plain message goes to the **in-house `default`** agent and returns:
  `no in-house model is configured on this node; select an external runtime (e.g.
  open-interpreter-default) or wire a model provider`
- Typing `/` shows the palette. In the latest screenshot it DOES list an agent command:
  **"Ask the in-channel agent to pursue a goal (pick a runtime)"** (tool `agent`).
- The user states firmly they have **NO option to pick `open-interpreter-default`** — i.e. either
  the runtime dropdown never shows a non-`default` id, or selecting the command doesn't surface a
  usable runtime picker for them. This is the unresolved part.

## What is CONFIRMED working (verified this session, not assumed)

1. `interpreter 0.0.17` is on PATH.
2. `ZAI_API_KEY` is valid — a direct Z.AI chat completion returns fine.
3. The **exact** `interpreter exec` command the codex wrapper builds runs and produces output —
   BUT only after adding a sandbox flag (see fix below). Default `exec` runs `--sandbox read-only`,
   so the agent refuses to write files ("the environment is read-only"), sometimes ending with an
   empty `agent_message` (looks like "did nothing"). `wire_api=chat` is mandatory (`/responses`
   404s on Z.AI).

## Fix already applied this session (SEPARATE issue — the sandbox bug, NOT the picker)

File: `rust/crates/external-agent/src/wrappers/codex.rs` — `CodexWrapper::command_args` now emits
`--sandbox workspace-write -c approval_policy=never` before `-C <workspace>`. Two argv tests in the
same file updated. `cargo test -p lb-external-agent` = green (11+ tests). This makes a real run
actually write+run files. It does NOT address the "no picker option" symptom.

Verified real Rust run through the fixed args: agent wrote `hello.rs`, ran `rustc`, executed
`./hello`, reported `hi`. (Note: scratch shell has `python3` not `python`, irrelevant to the bug.)

## The picker symptom — where it likely lives (NOT yet confirmed)

The channel runtime dropdown is `ui/src/lib/widgets/inputs/RuntimeArg.tsx`. It renders every id from
`useRuntimes()` → the `agent.runtimes` MCP verb. Options = `runtimes` or `[defaultId]` fallback. So
if the user sees only `default`, the node returned `{default:"default", runtimes:["default"]}`.

`agent.runtimes` (`rust/crates/host/src/agent/runtimes.rs`) reads `node.runtimes()` — the registry
`install()` populates. Boot path IS wired: `rust/node/src/main.rs:57` calls
`external_agent::install(&node)`, which (feature-on) builds a `RuntimeRegistry::with_default(
UnconfiguredModel)`, calls `register_external_runtimes`, `node.install_runtimes(registry)`, and
prints:
```
external-agent: runtimes installed = ["codex-default", "default", "open-interpreter-default", "vtcode-default"]
```

### Leading hypotheses to check IN ORDER (each is a concrete next step, no user input needed)

1. **Is the boot line actually printed with `EXTAGENT=1`?** Start the node the user's way, grep its
   stdout for `external-agent: runtimes installed`. If absent or `["default"]` only → the feature
   build isn't registering (check `make dev EXTAGENT=1` truly passes `--features external-agent`;
   Makefile line ~60 maps `EXTAGENT=1 → NODE_FEATURES=external-agent`). Confirm the running binary
   is the feature build (a stale feature-off `cargo run` node still bound to :8080 would explain it).
2. **Does `agent.runtimes` over the gateway return the external ids?** Needs a bearer token
   (`missing bearer credential` on a bare curl). Get the dev token the UI uses and
   `POST /mcp/call {"tool":"agent.runtimes","args":{}}`. If it returns only `default` while the boot
   line listed all four → the served registry differs from the installed one (serve-wiring gap; see
   the `TODO(serve-wiring)` note in `rust/node/src/external_agent.rs` — the registry the agent
   *serve* path reads may not be the one `install()` populated).
3. **Catalog staleness:** `ui/src/features/channel/palette/useCatalog.ts` fetches `tools.catalog`
   ONCE on mount and caches. If the user's browser tab predates the feature-on node, the palette /
   arg schema may be stale — hard-reload the UI.
4. **The runtime dropdown may only render after the goal field is filled**, or the `x-lb:widget:
   "runtime"` arg isn't attached to the `agent.invoke` descriptor the catalog served. Check the
   descriptor `agent.invoke` returns from `tools.catalog` actually carries the runtime widget arg.

### Key files
- `rust/node/src/external_agent.rs` — install()/register; note the `TODO(serve-wiring)`.
- `rust/node/src/main.rs:57` — install() call site.
- `rust/crates/host/src/agent/runtimes.rs` — `agent.runtimes` reads `node.runtimes()`.
- `rust/crates/host/src/agent/serve.rs` / `dispatch.rs` — the serve/dispatch registry path.
- `ui/src/lib/widgets/inputs/RuntimeArg.tsx` + `useRuntimes.ts` — the dropdown.
- `ui/src/features/channel/palette/CommandPalette.tsx:214` — `agent.invoke` → `onSendAgent`.
- `ui/src/features/channel/palette/useCatalog.ts` — cached `tools.catalog`.

## Workaround if the picker stays broken

Set the workspace default runtime so a bare `/agent` (no runtime) resolves to Open Interpreter — the
resolution seam is `explicit arg → workspace default → registry default`
(`rust/crates/host/src/agent/resolve_default.rs`):
```
lb call agent.config.set '{"patch":{"default_runtime":"open-interpreter-default",
  "model_endpoint":{"provider":"zaicoding","model":"glm-4.6","api_key_env":"ZAI_API_KEY",
  "base_url":"https://api.z.ai/api/coding/paas/v4"}}}'
```
This still requires the node to OFFER `open-interpreter-default` (hypothesis 1/2) — `agent.config.set`
rejects a `default_runtime` the node doesn't offer with `BadInput`. So it's a workaround for the
*picker UI*, not for a node that isn't registering the runtime at all.

## What the next session should do FIRST
Run the user's exact command, capture the node boot log, grep for the `runtimes installed` line, and
hit `agent.runtimes` with a real token. That single pair of facts decides between hypothesis 1
(feature not registering) and hypothesis 2 (serve-wiring gap). Do not ask the user to interact with
the UI to gather this — get it from the node directly.
