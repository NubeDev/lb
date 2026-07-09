# External-agent scope — agent-authored extensions ("build me an energy dashboard")

Status: scope (the ask). Promotes to `public/external-agent/` once shipped.

A user in the running app says to the agent: *"build me an energy dashboard page"* (or *"a tag
normalizer tool"*) — and an **external agent runtime** (Open Interpreter / codex-family, per
`agent-runtimes-scope.md`) authors a **whole extension** with the devkit — **any tier**: a Rust
**WASM** guest tool, a **native** (Tier-2) sidecar, and/or a federated **UI** page — builds it
hermetically (`DEVKIT_BUILDER=container`), and — after the Ask-gated human approval — publishes
it live into the workspace, then drives the new tool to prove it. For the UI case the generated
page must be **shadcn/ui + recharts on the shell theme**. There is **no per-domain persona**
("energy-dashboard persona" is explicitly what we are NOT building): the generic
`builtin.extension-builder` persona + the granted skills catalog carry all the knowledge, and
the *domain* ("energy", "tags") arrives only in the user's goal text. This scope closes the
four gaps that stop that today: (1) the external agent has **no way to call node MCP tools**,
(2) the `extension-builder` persona is **pinned to the in-house runtime**, (3) the devkit **UI
template is unthemed** bare React (no Tailwind/shadcn tokens, no recharts) — the wasm/native
Rust templates are already adequate for backend authoring — and (4) the grounding **skills
don't teach the theming/chart contract**.

Dev posture for everything here: `make dev EXTAGENT=1 DEVKIT_BUILDER=container` (the
`external-agent` cargo feature is **off by default** — `rust/node/src/external_agent.rs`; the
container builder needs `make docker-build-image` once, `Makefile` lines 132–136).

## Goals

- **G1 — MCP bridge (the wall, minimal slice).** A spawned external agent can call the node's
  MCP tools — and *only* those — under the derived principal `persona ∩ agent ∩ caller`, with
  every call re-checked server-side by `caps::check`. This is the shippable subset of
  `capability-wall-scope.md` #3; the OS sandbox (namespaces/seccomp) stays in that scope.
- **G2 — persona unlock.** `builtin.extension-builder` runs on external runtimes once (and only
  where) the bridge is active; its Ask floor (`ext.publish`, `native.install`, …) still produces
  a durable human decision even when the call arrives over the bridge.
- **G3 — themed scaffold.** `devkit.scaffold` with the `ui` feature emits a
  **correct-by-construction themed template**: Tailwind v4, shadcn-token aliasing per the
  library CSS rules, **recharts** as the default chart lib wired to the shell's
  `--chart-1..8` / `ctx.theme` (contract v4), CSS scoped under the extension root class, no
  preflight. An agent that only fills in the blanks produces an on-theme page.
- **G4 — skills teach it.** `core.extension-authoring` (grounding skill of the persona) gains
  the UI-theming/recharts section so the folded goal carries the contract; the catalog line and
  the SKILL.md stay grounded in a live run.
- **G5 — E2E proof, both shapes.** Two real end-to-end runs: (a) **UI** — admin asks for an
  "energy dashboard" page → external agent scaffolds/builds (container) → Ask decision →
  publish → the page renders in the shell with the workspace theme (light + dark), charts
  colored from `--chart-N`; (b) **WASM tool** — admin asks for a small backend tool (e.g. a
  string/tag transform) → agent scaffolds `tier:wasm` (no `ui` feature), writes the Rust guest,
  container-builds `--target wasm32-wasip2`, publishes after the Ask decision, then **drives
  the new `<id>.<tool>` via its own MCP call** and confirms it in `tools.catalog` — the
  persona's built-≠-done discipline, exercised over the bridge.

## Non-goals

- **No per-domain personas.** No "energy" anything in core, personas.toml, templates, or
  skills. Domain enters via the goal string only (rule 10: core knows no extension).
- **Not the full capability wall.** OS sandboxing, ACP fs/terminal-decline, and the
  built-ins-off fail-closed assertion remain `capability-wall-scope.md`. We ship the *tool
  bridge* + the existing scratch-dir seal (`rust/role/external-agent/src/scratch.rs`); we do
  NOT claim the subprocess can't use its own shell. That is why publish stays Ask-gated.
- **Not the ACP SDK driver.** The bridge works over the shipped `exec --json` transport's
  config surface (external agents consume an MCP server config); `acp-driver-scope.md` #2
  replaces the transport later without moving the bridge.
- **No new chart framework in the shell.** The shell keeps recharts (`ui/package.json`,
  `ui/src/features/charts/`); this scope standardizes the *extension template* on it too.
- **Not the Studio wizard rewrite.** The Studio manual flow (Create → Build → Publish) is
  untouched; we add one entry point (see flow step 1), not a fourth step.

## Intent / approach

**The bridge is a stdio MCP shim, not a second server.** We add a tiny binary
`lb-mcp-shim` (new leaf crate `rust/crates/mcp-shim/`, one responsibility: stdio MCP ⇄
`POST /mcp/call`). When `dispatch.rs::invoke_via_runtime` selects an external runtime, the role
crate (`rust/role/external-agent/`) writes the runtime's MCP-server config (every supported
wrapper — Open Interpreter, codex-family — accepts an MCP servers config file/flag) pointing at
the shim, and hands the shim two env vars: the gateway URL and a **run-scoped token**. The shim
forwards `tools/list` → the run's narrowed tool menu and `tools/call` → `POST /mcp/call`; the
node re-checks caps on every call exactly as it does for the UI. Nothing new is reachable: the
gateway stays the only server surface (scope/README rule), and the shim holds no state.

**The run token follows the locked key-lifecycle decisions** (`agent-key-lifecycle-scope.md`
D1–D5): minted in `rust/role/external-agent/` at run start with the derived principal's caps,
5-minute TTL, shim refreshes at 60% TTL + self-heals on 401 via a refresh endpoint that the
gateway refuses once the run is terminal. The token never appears in the goal text or logs.

**Ask-gating over the bridge reuses the shipped policy engine.** A bridged `ext.publish` hits
the same host dispatch as an in-house call, so the persona's `policy_preset.ask` produces the
same durable suspension (`agent.decide`). The shim surfaces the "pending human decision"
response to the agent as a normal tool result ("awaiting approval — poll/decide happens in the
app"); the run stays inside `RUN_WALL_CEILING` or ends politely telling the user to approve and
re-invoke. (Decision: **do not** block the subprocess waiting on a human — a 15-min wall clock
and a human coffee break don't compose. The agent reports "publish proposed", the human
approves in the dock/Studio, the effect fires from the suspension — same as in-house.)

**The template upgrade is the default `ui` template, not a new feature flag.** One template,
correct by construction (`css-isolation-scope.md`'s direction): Tailwind v4 via
`@tailwindcss/vite`, an `src/styles/tokens.css` that aliases host shadcn vars with standalone
fallbacks (`--lbx-bg: var(--background, …)` — the `nav-rail`/`panel` pattern in `packages/*`),
utilities scoped under `.lbx-<id>` root class, **no preflight**, and a sample
`src/widgets/SampleChart.tsx` rendering a recharts `LineChart` colored from
`hsl(var(--chart-1))` with the v4 `ctx.theme.chart[]` fallback for standalone dev. The
`src_contract.ts.tmpl` mirror stays byte-identical to `federationWidget.ts` (guarded by
`contract-mirrors.guard.test.ts` — extend the guard to the new files if they touch the
contract). *Alternative rejected — a separate `ui-themed` feature*: two templates means the
agent (and every human) must pick, and the unthemed one becomes the permanent wrong default.
*Alternative rejected — echarts as the template default*: echarts is the JS-canvas escape hatch
(`echarts-panel` reference), can't read CSS vars, and doubles the taught surface; recharts is
what the shell itself uses (`chartTheme.ts`) and consumes tokens natively. The user's ask is
explicit: shadcn/ui + recharts + shell theme.

**Entry point in the UI.** The dock/channel `agent.invoke` path already works (composer
`CommandPalette`, `runtime` dropdown via `agent.runtimes`, persona precedence explicit →
workspace default). We add one affordance: a **"Generate with agent"** card on the Studio
Build tab (`ui/src/features/studio/`) that pre-fills an `agent.invoke` with
`persona: "builtin.extension-builder"` and the chosen runtime, then deep-links to the run feed.
No new verb — it is sugar over the shipped invoke.

## How it fits the core

- **Tenancy / isolation:** the run token carries the caller's workspace; every bridged call is
  workspace-checked first (wall order unchanged). Scratch dir stays per-ws/per-run
  (`scratch.rs`). Published extension lands in the caller's workspace registry only.
- **Capabilities:** invoke gate `mcp:agent.invoke:call` (member); the useful surface
  (`mcp:devkit.templates|root|scaffold|inspect|build:call`, `mcp:ext.publish|list:call`,
  `mcp:host.fs.list:call`, `mcp:tools.catalog:call`, …) is admin-tier — a member run gets the
  honest opaque deny on the first devkit call (mandatory deny test). The tool menu the shim
  advertises is `narrow_tools(persona) ∩ agent ∩ caller`; advertisement is UX, `caps::check`
  is the wall.
- **Placement:** local-only in practice — `devkit.*` is a local-developer surface and container
  builds need a runtime on the node; the bridge itself is placement-agnostic (config, no
  `if cloud`).
- **MCP surface:** **no new host verbs.** The shim is a client of the existing bridge
  (`POST /mcp/call`) + one new gateway route `POST /agent/runs/{id}/token/refresh`
  (run-status-gated, per key-lifecycle D3). API shape: the refresh is a single `get`-style
  verb; no CRUD/list/batch applies (N/A — the run lifecycle owns creation/expiry).
- **Data (SurrealDB):** no new tables. Run transcript/events, the `devkit-build` job record,
  and the durable Ask suspension are all existing records. Token material is never persisted.
- **Bus (Zenoh):** unchanged — build log on `devkit/build/<job_id>` (fire-and-forget), run
  events via the existing `publish_run_event` path.
- **Sync / authority:** node-local feature; no offline story beyond the run failing honestly.
- **Secrets:** the run token is minted in-memory in `role/external-agent` (D4) and passed via
  env to the shim only; the container build's git token stays on its shipped `lb-secrets`
  path (`devkit/build-git-token`) — the agent never sees it.
- **Symmetric nodes / one datastore / stateless extensions / state-vs-motion:** all unchanged;
  the generated extension is stateless like any other.
- **No mocks:** all tests run the real gateway/store/bus; the external agent binary and Docker
  are the sanctioned externals (same status as `cargo`/`pnpm` behind `Toolchain`). No
  `*.fake.ts`; the shim is tested against the real gateway.
- **FILE-LAYOUT:** shim crate = folder-of-verbs (`serve.rs`, `forward.rs`, `refresh.rs`,
  `menu.rs`); template additions are one file per widget/style concern.
- **SDK/WIT impact:** none — the WIT world and manifest are untouched. **Widget mount
  contract:** unchanged (still v4); only template *content* changes. If any contract file is
  touched, all three mirrors + the guard test move together — flagging loudly per the rule.
- **Skill doc:** YES — this is an agent-drivable surface. The implementing session updates
  `docs/skills/extension-authoring/SKILL.md` (theming/recharts section + external-runtime
  notes) and `docs/skills/external-agent/SKILL.md` (bridge + shim + token lifecycle), both
  grounded in the live E2E run. Core skills are build-embedded (`corpus.rs`) — re-seed on
  rebuild is automatic.

## Slices (build order)

1. **S1 — shim + bridge.** `rust/crates/mcp-shim/` binary; role-crate config writer per
   wrapper profile (`profiles.rs`); run-token mint + refresh route (key-lifecycle D1–D5);
   tool-menu advertisement from the narrowed menu. Exit: an external run lists and calls
   `tools.catalog` for real; a call outside the menu/caps is denied server-side.
2. **S2 — persona unlock + Ask over the bridge.** `personas.toml` `extension-builder`
   `runtimes` gains the external ids; `apply.rs::check_runtime` comment updated; prove the
   Ask suspension fires on a bridged `ext.publish` and the approval executes it.
3. **S3 — themed `ui` template.** Template files per Intent; `devkit.build` (container) green
   on a fresh scaffold; standalone `pnpm build` green too.
4. **S4 — skills + Studio entry card.** Skill sections; `StudioShell` "Generate with agent"
   card; catalog descriptions current.
5. **S5 — the two E2E runs + docs promotion** (after the user confirms both work — see below).
   The WASM-tool run (G5b) needs S1+S2 only, so run it as the first live checkpoint once S2
   lands; the UI run (G5a) additionally needs S3+S4.

## Example flow

1. Node up via `make dev EXTAGENT=1 DEVKIT_BUILDER=container`. Admin (dev-login) opens Studio →
   Build → "Generate with agent", types *"build me an energy dashboard page — kWh by hour for
   the demo-buildings source"*, picks runtime `open-interpreter-default`. The card fires
   `agent.invoke {goal, runtime, persona:"builtin.extension-builder"}`.
2. `dispatch.rs` resolves the persona (identity + grounding skills folded into the goal,
   catalog + memory appended), derives the principal, mints the run token, writes the wrapper's
   MCP config → spawns the agent with `lb-mcp-shim` as its only configured tool server, cwd =
   the per-run scratch dir.
3. Agent calls `devkit.templates` → `devkit.root` → `devkit.scaffold {id:"energy-dashboard",
   tier:"wasm", features:["ui"]}` over the bridge; gets the themed template; writes its page
   (shadcn card + recharts `LineChart` bound via the v3 `sources[]`/frames-in contract taught
   by the skill).
4. `devkit.build {path}` → container job; agent tails the result; `devkit.inspect` for the
   publish substance (caps, tools, size).
5. `ext.publish` → **Ask suspension** (persona floor). The agent reports "publish proposed —
   approve in the app" and ends the turn. The admin approves (`agent.decide` in the dock);
   the publish executes: server-side sign → verify → install → live, no restart.
6. User opens the new page; it renders in the workspace theme, dark-mode toggle recolors it
   (CSS vars cascade; charts read `--chart-N`). Deny path: a member trying step 3 gets an
   opaque deny on `devkit.scaffold` and the run reports it honestly. Expired-token path: shim
   refreshes at 60% TTL; after run cancel, refresh is refused and the next call fails closed.
7. **WASM-tool variant (G5b):** same steps 1–5 with a backend goal (*"a tool that normalizes
   tag strings"*) — the agent scaffolds `tier:"wasm"` **without** `features:["ui"]`, writes
   `src/lib.rs` against the WIT world (guest tool, declared in `extension.toml`),
   container-builds `--target wasm32-wasip2`, and after the approved publish **calls the new
   tool by its dotted name over the bridge** (`<id>.<tool>` — id is opaque data, rule 10) and
   checks `tools.catalog` before reporting done. A native (Tier-2) goal follows the same path
   with `tier:"native"`; `native.install` sits under the same Ask floor.

## Testing plan

Per `scope/testing/testing-scope.md` — real store (`mem://`), real bus, real gateway, real
binaries. Sanctioned externals: the agent binary, Docker.

- **Capability deny (mandatory):** (a) member-caps run → first `devkit.scaffold` over the
  bridge is `Denied`, nothing scaffolds; (b) a bridged call to a tool outside
  `narrow_tools` → denied even though caps would allow (menu ∩); (c) refresh after terminal
  run → refused.
- **Workspace isolation (mandatory):** ws-A's run token cannot read/publish into ws-B
  (bridged `ext.list`/`ext.publish` cross-ws → deny; reuse the registry isolation fixture).
- **Bridge integration (Rust, `#[cfg(feature = "external-agent")]`):** spawn a real shim
  against a live test gateway; `tools/list` equals the narrowed menu; `tools/call` round-trips
  `tools.catalog`; token never appears in transcript/logs (assert on bytes, same discipline as
  the git-token test).
- **Ask-over-bridge (integration):** bridged `ext.publish` creates the durable suspension; a
  subsequent `agent.decide` approval executes the publish; a deny leaves nothing installed.
- **WASM authoring loop (integration):** scripted over the bridge (no model): scaffold
  `tier:wasm` → write a minimal guest tool → container `devkit.build` → approved publish →
  call `<id>.<tool>` via `POST /mcp/call` and assert the result + `tools.catalog` entry —
  the deterministic skeleton of E2E run G5b (prereq: wasm exts built, `make build-wasm`).
- **Template (UI unit + gateway):** fresh scaffold builds standalone (`pnpm build`) and via
  `devkit.build` container mode; a gateway test mounts the built widget through `ExtHost` and
  asserts (jsdom) the root class scoping, no-preflight (host `h2` styles unchanged), and that
  the sample chart's series color resolves from the host `--chart-1` in light AND `.dark`.
  Contract mirror guard stays green.
- **E2E (S5, semi-manual):** the Example flow run on the live dev node, recorded in the
  session doc with the run id, build job id, and screenshots light/dark. Regression tests for
  anything that breaks land per `debugging-scope.md`.
- Pre-existing failures to ignore as noise: `SystemView.gateway`, `sqlSource.gateway`,
  `agent_routed_test` (red on clean master).

## Risks & hard problems

- **The honest-boundary gap.** Until the OS sandbox ships, the external agent can still use
  its own shell beside the bridge. Mitigations already in posture: scratch-dir cwd seal,
  Ask-gated publish/install (nothing enters the node unreviewed), admin-only surface. Say this
  plainly in the SKILL.md; do not market the bridge as a sandbox.
- **Token in the wrong place.** The token must live only in the shim's env and the MCP config
  file inside the per-run scratch dir (mode 0600, deleted with the run) — never in the goal,
  transcript, or build log. Byte-assert tests.
- **Ask + wall-clock interplay.** A run that proposes publish and *waits* would burn the
  15-min ceiling; the decided contract (report-and-end, approval executes from the suspension)
  must be taught in the skill or agents will poll.
- **Wrapper config drift.** Each wrapper names its MCP config differently; keep it one file
  per wrapper in `role/external-agent/src/wrappers-config/` and cover each with the bridge
  integration test, or the second runtime silently ships broken.
- **Template rot.** The themed template duplicates token names from `globals.css`; a renamed
  shell token breaks extensions silently. The gateway template test above is the tripwire —
  keep it asserting on *resolved color equality*, not just class presence.
- **jsdom quirks:** recharts renders zero-size in jsdom (rect-cull quirk, cf. FlexLayout note)
  — assert on stroke/fill attributes and resolved CSS vars, not on pixel layout.

## Open questions

None — decisions taken above (stdio shim over ACP-SDK bridge; report-and-end Ask contract;
upgrade the default `ui` template rather than fork it; recharts over echarts as template
default; no Studio wizard step). If implementation invalidates one, record the reversal in the
session doc and update this scope.

## Handoff notes for the implementing session (read first)

- Start the node with `make dev EXTAGENT=1 DEVKIT_BUILDER=container` (run
  `make docker-build-image` once; `make build-wasm` before `make test-be`).
- Dev-login is member-shaped but carries the admin-tier caps; for a *true* member in deny
  tests use `signInWithCaps`.
- The one selection point for runtimes is `dispatch.rs::invoke_via_runtime`; do not add a
  second. The goal-fold order (identity → grounding skills → catalog → memory) is load-bearing.
- Sequence S1→S5; each slice lands with its tests green before the next. Session doc at
  `docs/sessions/external-agent/agent-ext-authoring-session.md` from the ABOUT-DOCS template.
- Run the WASM E2E (G5b) as soon as S2 is green — it validates the bridge/persona/Ask core
  without waiting on the template work.
- S5 gate: demo **both** E2E runs to the user; **only after they confirm they work** promote to
  `doc-site/content/public/external-agent/` + `doc-site/content/public/extensions/dev-flow.mdx` (template section),
  update both SKILL.md files and `docs/STATUS.md`.

## Related

- `external-agent-scope.md` (umbrella), `capability-wall-scope.md` (#3 — this ships its bridge
  subset), `agent-key-lifecycle-scope.md` (D1–D5 govern the run token),
  `acp-driver-scope.md` (#2 — future transport under the same bridge),
  `agent-runtimes-scope.md`, `run-lifecycle-scope.md`.
- `../agent-personas/persona-coding-scope.md` (#4 — the `extension-builder` posture this
  unlocks), `rust/crates/host/src/agent/personas/personas.toml`.
- `../extensions/ext-sdk-scope.md` (devkit flow), `../extensions/devkit-container-build-scope.md`
  (shipped container builder), `../extensions/ui/css-isolation-scope.md` +
  `../extensions/ui/theme-inheritance-scope.md` (the template's CSS/theme rules),
  `../extensions/ui-federation-scope.md`.
- Shell theme/chart truth: `ui/src/styles/globals.css` (`--chart-1..8`),
  `ui/src/features/charts/chartTheme.ts`,
  `ui/src/features/dashboard/builder/federationWidget.ts` (frozen v4 contract),
  `ui/src/lib/theme/resolve-theme-tokens.ts`; reference consumer
  `rust/extensions/echarts-panel/`.
- Skills: `docs/skills/extension-authoring/SKILL.md`, `docs/skills/external-agent/SKILL.md`.
- README §3 (rules 5, 7, 10), §6.10 (jobs).
