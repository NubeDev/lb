# Agent-authored extensions — S1–S4 session

**Scope:** [`scope/external-agent/agent-ext-authoring-scope.md`](../../scope/external-agent/agent-ext-authoring-scope.md)
**Slices built:** S1 (shim + bridge), S2 (persona unlock + Ask-over-bridge), S3 (themed UI template), S4 (skills + Studio card). S5 (live E2E demo + docs promotion) gated on user confirmation per the scope's handoff notes.

## Follow-up session — the MCP-only E2E gap (`devkit.write_file` + live energy-dashboard)

The S1–S4 work left the E2E **unproven**: the previous session hand-wrote `App.tsx`/`bridge.ts`/
`mount.tsx` into the scaffolded `test-panel` dir, then rebuilt — which proves nothing about whether
an agent driving ONLY the MCP API can author an extension. This session closed that gap.

### What shipped

**`devkit.write_file` — the MCP file-write verb (Option A).** The agent's scaffold→build loop had
no way to put real source between scaffold and build through MCP alone — THE blocker. Added:

- `lb_devkit::write_file` (`rust/crates/devkit/src/write_file.rs`): resolves `path` under the
  devkit root via the SAME `resolve_under_root` gate `scaffold`/`build`/`inspect` use (traversal +
  symlink-escape guards apply), writes UTF-8 `content`, creates parent dirs, overwrites if present.
  New `WriteFileReport { path, bytes }` model.
- Host gate `devkit_write_file` (`rust/crates/host/src/devkit/write_file.rs`): runs
  `mcp:devkit.write_file:call`, then delegates.
- Wired into `call_devkit_tool`, `system/catalog.rs`, the `extension-builder` persona's
  `granted_tools`, the dev-admin `member_caps` (`session/credentials.rs`), and the persona-coding
  fluid-loop test.
- 5 unit tests (`rust/crates/devkit/tests/write_file_test.rs`): new file, overwrite, traversal
  reject, absolute-outside-root reject, round-trip with `resolve_under_root`.

**Scaffold `[ui] scope` own-tool name fix.** `devkit/src/scaffold.rs::tool_name` was doing
`id.replace('-', ".")`, so id `energy-dashboard` produced scope entry `energy.dashboard.ping`
while the runtime resolves `<ext>.<tool>` by `split_once('.')` with the id **verbatim** — the
scope entry never matched for any hyphenated id. Now uses the id verbatim.

**`Claims` test fix.** `agent_persona_coding_test.rs` was missing the `constraint`/`run_id` fields
the S1 session added to `Claims` — the test hadn't compiled since S1. Added the two `None` fields
to the test's `principal()` helper. (Two stale S2 tests remain red — they assert external runtimes
*fail*, but S2 *unlocked* them; inverted from the new reality. Out of scope here.)

### The live E2E (every step a real curl, no hand-edits, no restart after publish)

Against `make cloud` + `make seed-demo-sqlite` (the `demo-buildings` sqlite source), all via
`POST /mcp/call` + `POST /extensions`:

1. `POST /login` → admin token.
2. `devkit.scaffold {id:"energy-dashboard", tier:"wasm", features:["ui","datasources"]}` → 16 files.
3. **`devkit.write_file`** replaced `ui/src/App.tsx` with a real page (recharts `BarChart` + table,
   data via `bridge.call("federation.query", {source:"demo-buildings", …})`) — the step that was
   done by hand before.
4. `devkit.build` → fresh wasm (81818 B) + `remoteEntry.js` (558429 B).
5. `POST /extensions` → **204**, live, no restart.
6. The mounted page's data path answers: `federation.query` → 8 real rows
   (`Riverside Data Center` 49022.9 kWh, …).
7. `GET /extensions/energy-dashboard/ui/remoteEntry.js` → **200** (558429 B, `text/javascript`).
8. `GET /extensions` → `energy-dashboard` is `enabled + running`, UI label set, scope tools
   include `federation.query`.

The shell that mounts it: `make ui-preview` (`pnpm exec vite build && vite preview`, port 4173) —
the vite dev server's FSWatcher segfaults in this env, but the built shell is stable and is the
recommended preview path for federated pages anyway.

### Gap found (NOT fixed — recorded)

Publishing does **not** grant the new extension's tool call caps to any caller, and the dev-login
token carries a static cap list that doesn't fold durable grants — so `energy-dashboard.ping`
(like the boot-loaded `hello.echo`) returns `denied` over `POST /mcp/call`. The UI/data-dashboard
E2E (G5a) is unaffected (the page drives `federation.query`, whose cap IS held); the WASM-tool E2E
(G5b) is gated on resolving it. Full write-up + two viable fixes:
`docs/debugging/extensions/publish-does-not-grant-tool-call-caps.md`.

### Docs updated

- `docs/skills/extension-authoring/SKILL.md` — new §3.3 (`devkit.write_file`), the cap-table row,
  and §9 (the exact working agent-driven E2E curl sequence, grounded in this live run).
- `docs/debugging/extensions/publish-does-not-grant-tool-call-caps.md` + README row.
- This session section.

### Files touched this session

**New:** `rust/crates/devkit/src/write_file.rs`, `rust/crates/devkit/tests/write_file_test.rs`,
`rust/crates/host/src/devkit/write_file.rs`,
`docs/debugging/extensions/publish-does-not-grant-tool-call-caps.md`.

**Modified:** `rust/crates/devkit/src/{lib,model,scaffold}.rs`,
`rust/crates/host/src/devkit/{mod,tool}.rs`, `rust/crates/host/src/system/catalog.rs`,
`rust/crates/host/src/agent/personas/personas.toml`, `rust/crates/host/tests/agent_persona_coding_test.rs`,
`rust/role/gateway/src/session/credentials.rs`, `docs/skills/extension-authoring/SKILL.md`,
`docs/debugging/README.md`.

---

## What shipped (S1–S4)


### S1 — the MCP shim bridge

**New crate `lb-mcp-shim`** (`rust/crates/mcp-shim/`): a tiny stdio MCP ⇄ `POST /mcp/call` binary.
Speaks JSON-RPC 2.0 over stdio (one message per line) on three methods: `initialize`, `tools/list`,
`tools/call`. The menu is a pre-baked JSON file the role crate writes into the scratch dir
(advertisement only — `caps::check` on every `tools/call` is the wall). The token refreshes lazily:
proactively at 60% TTL (D2) + a one-shot 401 self-heal (D2). File layout (one verb per file):
`serve.rs` (the stdio loop), `forward.rs` (HTTP forward), `refresh.rs` (token lifecycle), `menu.rs`
(read the menu), `config.rs` (env wiring).

**Run-token mint** (`rust/role/external-agent/src/token.rs`): mints a short-TTL (5 min, D1),
run-scoped Ed25519-signed token carrying the derived principal's caps + the caller's `constraint`
+ `run_id`. The constraint is load-bearing: a verified token produces a `Principal` via
`lb_auth::verify` which does NOT re-introduce the caller bound (gate 2b only fires when
`Principal.constraint` is `Some`), so the caller's caps MUST ride IN the token as the `constraint`
claim. Without this, a run token would widen to the agent's own caps.

**Gateway run-status gate (D3)** (`role/gateway/src/session/authenticate.rs`): `verify_token` now
consults the job record's status when the token carries a `run_id`. A terminal run (Cancelled/
Failed/Done) → token refused as opaque `401` (no oracle). Hard cancel is instant — the TTL is the
belt, run-status is the braces. Ordinary session tokens (no `run_id`) skip this read entirely —
zero overhead on the hot path.

**Token refresh route** `POST /agent/runs/{id}/token/refresh` (`role/gateway/src/routes/
run_token_refresh.rs`): re-mints under the node key with a fresh TTL. The path-vs-claim id mismatch
is `400`; a non-run token on the route is `400`; a terminal run's refresh is refused (D3).

**Claims extension** (`rust/crates/auth/src/claims.rs`): `Claims` gained `constraint: Option<Vec<
String>>` + `run_id: Option<String>` (both `#[serde(default)]` — legacy tokens deserialize
unchanged). `Principal::from_token_claims` (pub(crate)) populates both from the signed claims so
the gateway's two-gate + run-status checks fire on the verified principal exactly as they would on
a derived one.

**Per-wrapper MCP config** (`rust/crates/external-agent/src/wrapper.rs`): `AgentWrapper` gained
`fn mcp_config(&self, shim_bin, scratch) -> Option<McpConfig>` (default `None`). `CodexWrapper`
implements it — writes a `config.toml` under `<scratch>/codex-home/` with `[mcp_servers.lb]`
pointing at `lb-mcp-shim`. VT Code returns `None` (pre-bridge). `drive()` gained `extra_env: &[(String,
String)]` for the bridge env vars (the shim inherits them from the agent child).

**Bridge wiring** (`rust/role/external-agent/src/bridge.rs` + `lib.rs`): `AcpRuntime::run` now
(1) creates the durable job record at start (so D3 works), storing the persona's Ask floor in the
payload; (2) mints the run token; (3) writes the menu JSON + the wrapper config into the scratch
dir (mode 0600); (4) marks the job Done/Failed at end. The token + gateway URL NEVER appear in the
goal, a transcript, or a log — only in the per-child env map + the per-run config file.

### S2 — persona unlock + Ask-over-bridge

**Persona unlock** (`rust/crates/host/src/agent/personas/personas.toml`): `extension-builder`
`runtimes` gained the external ids (`open-interpreter-default`, `vtcode-default`, `codex-default`).
The in-house loop stays the default; the external runtimes are additive choices.

**Ask-over-bridge gate** (`rust/role/gateway/src/routes/mcp.rs`): `/mcp/call` now checks the run's
persona Ask floor BEFORE dispatching when the caller is a run-scoped token. If the tool is in the
ask list, the call returns `{ ok: false, awaiting_approval: true, message: "..." }` as the tool
result — the agent reports it and ends its turn, the human approves in the dock/Studio (same
`agent.decide` path). This is the same durable suspension concept as the in-house loop, produced at
the dispatch chokepoint (the bridge has no host-side loop to intercept at).

### S3 — themed UI template

The devkit's `ui` template (`rust/crates/devkit/templates/ui/`) was upgraded from bare inline-styled
React to a correct-by-construction themed template:
- **Tailwind v4** via `@tailwindcss/vite` (the vite plugin). `package.json.tmpl` carries
  `tailwindcss`, `@tailwindcss/vite`, `recharts`.
- **Scoped tokens** (`src_styles_tokens.css.tmpl`): `.lbx-<id>` root class aliases the host shadcn
  vars with standalone fallbacks (`--lbx-bg: var(--background, 210 20% 98.5%)`). NEVER `:root` (the
  federated-CSS leak). A `[data-theme="dark"]` standalone fallback.
- **No preflight** (`src_styles_main.css.tmpl`): `@import "tailwindcss/theme"` +
  `@import "tailwindcss/utilities"` — NOT `@import "tailwindcss"` (that includes preflight, which
  resets the host's base styles when federated).
- **Sample chart** (`src_widgets_SampleChart.tsx.tmpl`): a recharts `LineChart` with stroke
  `hsl(var(--lbx-chart-1))` — follows the workspace theme (light + dark) automatically.
- **Root wrapper** (`src_mount.tsx.tmpl`): `<div className="lbx-<id>">` anchors the scoped tokens.
- `scaffold.rs` emits the new files; `scaffold_test.rs` asserts the themed shape.

### S4 — skills + Studio card

**Skill docs** updated:
- `docs/skills/extension-authoring/SKILL.md` — new `## 8. The themed UI template` section (the
  theming/chart contract an agent must follow).
- `docs/skills/external-agent/SKILL.md` — new `## 7. The MCP bridge` section (the bridge/shim/token
  lifecycle + the honest "not a sandbox" statement).

**Studio "Generate with agent" card** (`ui/src/features/studio/GenerateWithAgent.tsx`): a card on
the Build tab that pre-fills an `agent.invoke` with `persona:"builtin.extension-builder"` + the
chosen runtime, then deep-links to the run feed. Hides when no external runtimes are registered
(the feature is off). Sugar over the shipped invoke — no new verb. `InvokeRequest` + the `agent_invoke`
ipc command + `invokeAgent` API client gained an optional `runtime` field (additive — absent ⇒
workspace default, the existing genui/dashboard path).

## Tests (real gateway + real store + real caps, rule 9)

- **`mcp_bridge_test.rs`** (gateway integration, 8 green): the shim lib (`serve_on`) driven against
  a real gateway over a duplex pair — no spawned binary, no fake backend. Covers: `tools/list`
  returns the narrowed menu; `tools/call` round-trips `host.time.now`; **capability deny**
  (mandatory — member-caps run denied on `host.fs.list`); **run-status gate D3** (mandatory —
  cancelled run's next call fails closed); **workspace isolation** (mandatory — ws-B token denied
  on ws-A); **token never in stdout** (byte-asserted); **Ask-over-bridge** (`ext.publish` returns
  "awaiting approval"); non-ask tool dispatches normally.
- **`run_token_refresh_route_test.rs`** (6 green): the refresh route happy path + D3 terminal-run
  refusal + path-vs-claim mismatch + non-run token + D3 at `/mcp/call` + ws-isolation.
- **`lb-mcp-shim` lib tests** (10 green): JSON-RPC dispatch, menu round-trips, refresh guard.
- **`lb-devkit` scaffold tests** (6 green): the themed template files are scaffolded with the right
  shape (scoped tokens, no preflight, recharts chart-1 stroke, tailwindcss deps).
- **`lb-role-external-agent` tests** (6 green): the bridge writes menu + config + env; the token
  carries run_id + constraint; refresh lead is 60% TTL.
- **`contract-mirrors.guard.test.ts`** (5 green): the frozen widget mount contract (v4) is
  untouched by the template upgrade.
- `pnpm exec tsc --noEmit` clean (0 errors).

## Pre-existing reds (NOT this slice)

- `SystemView.gateway`, `sqlSource.gateway`, `agent_routed_test` — red on clean master per the
  scope's testing plan. Not touched.
- Another session's in-flight flows work briefly broke `lb-host` during this session (a syntax error
  in `run_debug.rs`); it was fixed by that session before this session's final build.

## Open items (S5 — gated on user confirmation)

- The two live E2E runs (UI energy-dashboard + WASM tag-normalizer) need a real external agent
  binary (Open Interpreter) + `make docker-build-image` + `make dev EXTAGENT=1 DEVKIT_BUILDER=
  container`. Not runnable in this coding session. The deterministic skeleton (scaffold → build →
  publish → call) is proven by the bridge integration test; the live run is the model-in-the-loop
  confirmation.
- Docs promotion to `docs/public/external-agent/` + `docs/STATUS.md` update — do after the user
  confirms both E2E runs.

## Files touched

**New crates:**
- `rust/crates/mcp-shim/` (src/{lib,main,config,menu,forward,refresh,serve}.rs + Cargo.toml)

**New files:**
- `rust/role/external-agent/src/{token,bridge}.rs`
- `rust/role/gateway/src/routes/run_token_refresh.rs`
- `rust/role/gateway/tests/{run_token_refresh_route_test,mcp_bridge_test}.rs`
- `rust/crates/devkit/templates/ui/src_styles_{main,tokens}.css.tmpl`
- `rust/crates/devkit/templates/ui/src_widgets_SampleChart.tsx.tmpl`
- `ui/src/features/studio/GenerateWithAgent.tsx`

**Modified:**
- `rust/crates/auth/src/{claims,principal,verify}.rs` — `run_id` + `constraint` claims
- `rust/crates/external-agent/src/{wrapper,driver,wrappers/codex}.rs` — MCP config + `extra_env`
- `rust/role/external-agent/src/{lib,Cargo.toml}` + `tests/channel_smoke_test.rs` — bridge wiring
- `rust/role/gateway/src/{session/authenticate,server,routes/mcp,Cargo.toml}.rs` +
  `tests/common/mod.rs` + `tests/{native_callback,native_call_routes,gateway}_test.rs` +
  `src/signing_key.rs` — run-status gate + refresh route + Ask gate
- `rust/crates/host/src/agent/personas/personas.toml` — persona runtimes unlock
- `rust/crates/host/src/native/spec.rs` — Claims fields
- `rust/crates/devkit/src/scaffold.rs` + `tests/scaffold_test.rs` — themed template
- `rust/crates/devkit/templates/ui/{package.json,vite.config.ts,src_App,src_mount}.tmpl` — themed
- `rust/node/src/{main,control_engine,federation}.rs` — Claims fields
- `rust/role/acp/src/main.rs` — Claims fields
- `rust/role/gateway/src/routes/agent_invoke.rs` — `runtime` field
- `ui/src/lib/ipc/http.ts` + `ui/src/lib/agent/agent.api.ts` + `ui/src/features/studio/StudioView.tsx`
  — Studio card + runtime arg
- `docs/skills/extension-authoring/SKILL.md` + `docs/skills/external-agent/SKILL.md`
- `rust/Cargo.toml` — workspace registration for `lb-mcp-shim`
