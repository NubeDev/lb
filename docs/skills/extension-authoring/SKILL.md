---
name: extension-authoring
description: >-
  Author, build, test, and publish a Lazybones extension against the devkit — the DEVELOPER
  workflow: pick a template, scaffold into the devkit root, code within FILE-LAYOUT, build, inspect,
  then publish (a proposal) and drive the new tool against a live node to prove it. Use when a task
  says "build/write/author a new extension", "add a devkit tool or widget", "scaffold and test an
  extension", "publish what I built", or when grounding an agent that CODES extensions. This is the
  authoring companion to `core.extensions` (the OPERATOR view: enable/disable/uninstall) — here the
  job is to *make* one, not to *run* one. Covers `devkit.templates/scaffold/build/inspect/root`, the
  `extension.toml` manifest, the WIT world boundary, the two shapes (WASM guest tool vs federation UI
  widget), and the Ask-gated `ext.publish` / `native.install` posture.
---

# Authoring an extension against the devkit

> **STOP — read this before you run any shell command.** You author an extension **only** through the
> devkit MCP verbs against the **already-running** node (`devkit.templates → devkit.scaffold →
> devkit.write_file → devkit.build → devkit.inspect → ext.publish`). You do **NOT** build or run the
> platform yourself. Specifically, **never**:
> - `make dev`, `make build*`, `cargo build`, `cargo run`, or any build of *the repository* — the node
>   is already up; `devkit.build` compiles your extension hermetically (container builder) for you.
> - `git clone`/`cd` into the host repo, `chmod` its tree, or edit anything outside your scaffolded
>   extension folder — the repo is read-only to you and unrelated to your task.
> - hand-write `Cargo.toml`/`extension.toml`/`build.sh` from scratch — `devkit.scaffold` emits a
>   correct-by-construction template; you only fill in the source with `devkit.write_file`.
>
> Work **only** inside your run's scratch cwd and the scaffold path `devkit.scaffold` returns. If you
> find yourself shelling `make`, `cargo`, or `git` against the repo, you are off the rails — stop and
> call `devkit.templates` instead. (If you are an **external agent** run: your host-tool reach is the
> MCP bridge; your own shell is NOT the way in, and a run that only shells and never calls a `devkit.*`
> verb is reaped as stalled.)

In Lazybones **everything is an extension** — a chat app, a coding workplace, a flow node, a data
panel. The devkit is the **sanctioned way your code enters the platform**: it never lands as a core
crate or a special-cased branch (rule 10). An extension is *one folder* — a backend (a WASM component
or a native sidecar) plus an optional federated UI — described by an `extension.toml` manifest, that
the host verifies, installs, and hot-loads.

This skill is the **developer** view: `templates → scaffold → code → build → inspect → publish →
prove it`. The **operator** view (list / enable / disable / uninstall / native reset) lives in
`core.extensions` — go there for lifecycle management; this doc points at it once and stays on
authoring. The reference companion for the live-node verb shapes is `core.extensions`; the
"drive-what-you-built" discipline is `core.e2e-backend`.

Drive every verb over the node gateway (default `http://127.0.0.1:8080`; override with
`LB_GATEWAY_URL` / `VITE_GATEWAY_URL`). Two equivalent surfaces:

1. **The universal MCP bridge** — `POST /mcp/call {tool, args}` for any host verb by dotted name
   (`devkit.scaffold`, `devkit.build`, `ext.publish`). Rule 7: capabilities are MCP tools.
2. **Dedicated REST routes** — `POST /extensions` for publish (the gateway supplies the clock).

Workspace + principal come from the **bearer token**, never the body (the hard wall, §6/§7). Every
verb is capability-gated server-side; a denial is **opaque**.

## 1. What an extension IS (the boundary you code against)

- **Two tiers.** `wasm` (Tier-1, the default): a portable, in-process, capability-sandboxed
  WebAssembly component — a pure transform, no ambient authority. `native` (Tier-2): an OS child
  process the host supervises over framed stdio — for long-lived daemons or work the sandbox can't do
  (a DB driver, a broker client). Pick wasm unless you *need* the process.
- **The WIT world is the forever boundary.** `world = "lazybones:ext/extension@0.2.0"` in the
  manifest names the stable ABI. The host rejects a manifest whose world **major** differs from its
  SDK major at load time (bump minor freely: 0.1→0.2 is compatible). The WIT `.wit` files are a
  **stop-and-confirm** — do not edit them to make your extension fit; conform to them.
- **The manifest (`extension.toml`) is the contract**, read before anything is instantiated. It
  declares identity, tier, world, the caps it **requests**, the tools it exposes, and visibility.
  The host grants `granted = requested ∩ admin_approved` at install — the manifest **asks**, the
  workspace **decides**. A "built-in" extension gets the exact same path as a third-party one; nothing
  is pre-approved (rule 10).
- **Stateless (rule 4).** An instance holds nothing durable — state lives in SurrealDB or on the bus.
  That is what makes publish-a-new-version a lossless hot-reload (`hello` → `hello-v2`, same id).

**Reference targets to copy from:** `rust/extensions/hello` (minimal wasm guest, in the Makefile's
`WASM_EXTS`), `rust/extensions/hello-v2` (the version-swap), `rust/extensions/echo-sidecar` (native),
`rust/extensions/proof-panel` (UI page + widget). The devkit's own templates live in
`rust/crates/devkit/templates/{wasm,native,ui,common}`.

## 2. Authenticate

```bash
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login -H 'content-type: application/json' \
  -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)   # empty ws bootstraps a ws-admin
```

Send `Authorization: Bearer $TOKEN` on every call. Caps you need for the inner loop:

| Phase | Capability |
|---|---|
| Templates | `mcp:devkit.templates:call` |
| Root (path anchor) | `mcp:devkit.root:call` |
| Scaffold | `mcp:devkit.scaffold:call` |
| **Customize source** | `mcp:devkit.write_file:call` *(the agent authoring verb — see §3.3)* |
| Build | `mcp:devkit.build:call` + `mcp:bus.watch:call` (to stream the log) |
| Inspect | `mcp:devkit.inspect:call` |
| Publish | `mcp:ext.publish:call` *(Ask-gated for you — see §5)* |
| Native install | `mcp:native.install:call` *(Ask-gated — the sharpest tool)* |
| Call your tool | `mcp:<id>.<tool>:call` |

## 3. The authoring inner loop (Allow-fluid)

The scaffold → edit → build → inspect loop is **fluid** — these are your own working directory under
the devkit root; iterate freely. Publish is the boundary that changes posture (§5).

### 3.1 Pick a template — `devkit.templates`

```bash
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{"tool":"devkit.templates","args":{}}'
# → [{ "tier":"wasm",   "label":"Tier-1 WASM component",
#      "features":["ui","series-read","ingest","kv"], "world":"lazybones:ext/extension@0.2.0" },
#     { "tier":"native", "label":"Tier-2 native sidecar", "features":[…], "world":"…@0.2.0" }]
```

`features` are presets that expand into `[capabilities] request` in the scaffolded manifest:
`ui` (a federated UI page + widget bridge), `series-read` (`series.read/latest/find`), `ingest`
(`ingest.write`), `kv` (`template.get/save`), `datasources` (`federation.query/schema` +
`datasource.list` — the caps an extension needs to query an external datasource like postgres,
sqlite, or the `demo-buildings` source). **Pass `features` as a real JSON array**, not a stringified
one: `["ui","datasources"]` not `"[\"ui\",\"datasources\"]"`. (The host tolerates the stringified
form, but the real array is the correct shape — use it.)

### 3.2 Scaffold — `devkit.scaffold` (into the devkit root)

`args` is a `ScaffoldRequest`: `{ id, tier, features }`.

```bash
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{
  "tool":"devkit.scaffold",
  "args":{ "id":"weather-panel", "tier":"wasm", "features":["ui","series-read"] }}'
# → { "path":"/abs/repo/rust/extensions/weather-panel", "files":[ … ] }   (ScaffoldReport)
```

`id` is kebab-case (`[a-z0-9-]`, no leading/trailing dash, not the reserved `devkit-build*`). **Keep
the returned absolute `path`** — every later verb takes it. It is written under the devkit root
(`$LB_DEVKIT_ROOT`, default `rust/extensions/`); `devkit.root` returns that anchor. Anything outside
the root — or with a `..` — is rejected by `resolve_under_root` ("path escapes LB_DEVKIT_ROOT").

The folder scaffold writes:

```
rust/extensions/weather-panel/
├── extension.toml        # the §13 manifest contract
├── Cargo.toml            # wasm: [lib] crate-type=["cdylib"]  ·  native: [[bin]]
├── build.sh              # the tier's build recipe
├── src/lib.rs            # wasm entry (wit-bindgen)  ·  src/main.rs for native (lb-supervisor wire)
└── ui/                   # only with the `ui` feature — a module-federation remote (./mount)
```

### 3.3 Customize the scaffolded source — `devkit.write_file`

This is the verb that lets an **MCP-only agent** (no shell, no `Write` tool, no filesystem access
— the in-house runtime, or an external agent behind the bridge) author a real page/tool between
scaffold and build. Without it the scaffold → build loop can only ship the sample template; with it
the agent writes its own `App.tsx` / `src/lib.rs` / `extension.toml` over the one MCP contract.

`args` is `{ path, content }`. `path` is resolved under the devkit root by the SAME
`resolve_under_root` gate `scaffold`/`build`/`inspect` use, so the traversal / symlink-escape guards
all apply — the agent can only write inside its own scaffolded dir. `content` is the UTF-8 source
body (text only; no binary). It creates parent dirs as needed and **overwrites** if the file exists,
so it is both "create" and "edit".

```bash
# Author the real page: replace the sample App.tsx with one that queries demo-buildings through
# the federated bridge (the page's ONLY data path — never a token, DB, or fetch).
jq -n --rawfile body ./App.tsx '{tool:"devkit.write_file",args:{path:"weather-panel/ui/src/App.tsx",content:$body}}' \
  > /tmp/write_call.json
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' --data @/tmp/write_call.json
# → { "path":"/abs/repo/rust/extensions/weather-panel/ui/src/App.tsx", "bytes":5930 }   (WriteFileReport)
```

The `path` is **relative to the devkit root** (`<id>/ui/src/App.tsx`, not the absolute path) OR an
absolute path that lives under the root. A `..` segment or a path outside the root is rejected
("path escapes LB_DEVKIT_ROOT" / "traversal") — write_file can't escape the devkit root, same as
scaffold can't. Use the id-relative form; it's portable.

**What to write for each shape:**
- **UI page** (`features:["ui"]`) — replace `ui/src/App.tsx` with your real page. Keep the
  `.lbx-<id>` root wrapper in `mount.tsx` (it anchors the theme tokens — §8) and the `bridge` import;
  call data ONLY through `bridge.call(tool, args)` (e.g. `bridge.call("federation.query", …)`). Do
  NOT rewrite `src_contract.ts` (frozen mirror) or `tokens.css` unless you mean to.
- **WASM guest tool** (`tier:"wasm"`, no `ui`) — replace `src/lib.rs` with your tool dispatch
  against the WIT world; declare each tool in `extension.toml`'s `[[tools]]`.
- **Native sidecar** (`tier:"native"`) — replace `src/main.rs` against the supervisor wire.

**Gotchas:**
- **Write before build.** `devkit.build` compiles what's on disk; `write_file` is how the bytes get
  there over MCP. The order is always scaffold → write_file → build.
- **The runtime call name keeps the id verbatim.** A tool `[[tools]] name="ping"` on extension
  `energy-dashboard` is called as `energy-dashboard.ping` (the resolver splits on the FIRST dot).
  Do NOT convert `-` → `.` in tool names you reference from the page or the `[ui] scope`;
  `energy-dashboard.ping` is right, `energy.dashboard.ping` is wrong.
- **`content` is a JSON string.** For a multi-line source body, build the request body with
  `jq -n --rawfile` (or `--arg`) so newlines/quotes are escaped correctly — don't hand-quote it in
  the shell or the JSON will break on the first `"`.

### 3.4 Build — `devkit.build`

`args` is `{ path, ts? }`. `ts` (millis) is optional but seeds the `job_id` / `log_subject` — pass a
unique value so concurrent builds don't collide.

```bash
STARTED=$(curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{
  "tool":"devkit.build","args":{ "path":"/abs/repo/rust/extensions/weather-panel", "ts":1719829200000 }}')
# → { "job_id":"devkit-build-<ts>", "log_subject":"devkit/build/devkit-build-<ts>" }   (BuildStarted)
SUBJECT=$(echo "$STARTED" | jq -r .log_subject)
```

Build is async and runs the folder's `build.sh` (`cargo build --target wasm32-wasip2 --release` for
wasm, `cargo build --release` for native, then `pnpm install && vite build` if `ui/` exists). Stream
the log over SSE using the **returned `log_subject` verbatim** — don't hand-build the subject:

```bash
curl -sN "http://127.0.0.1:8080/bus/stream?subject=$(jq -rn --arg s "$SUBJECT" '$s|@uri')&token=$TOKEN"
# data:"    Finished release [optimized] target(s)"
# data:"devkit build: done"          ← terminal success line
# data:"devkit build: failed"        ← terminal failure line
```

### 3.5 Inspect — `devkit.inspect` (your pre-flight and proof)

`args` is `{ path }`. Call it **before** building (to check the toolchain) and **after** (to prove
an artifact was written):

```bash
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{
  "tool":"devkit.inspect","args":{ "path":"/abs/repo/rust/extensions/weather-panel" }}'
# → { "id":"weather-panel", "tier":"wasm", "tools":["ping"],
#     "caps":["mcp:series.read:call", …], "built":true,
#     "toolchain":{ "cargo":true, "pnpm":true, "wasm32_wasip2":true },
#     "artifacts":[{ "kind":"wasm", "path":"…", "size":123456, "mtime":"2026-07-05T…Z" }] }
```

`built` says an artifact exists; `toolchain` says whether a build *can* succeed (wasm needs `cargo` +
`wasm32-wasip2`; a UI needs `pnpm`); `artifacts[].size/mtime` let you diff before/after so "built"
means *this build wrote a fresh artifact*, not "a release dir exists". A doomed build (missing target)
is otherwise a slow way to find out — inspect first.

## 4. Code within FILE-LAYOUT (the persona builds inside these rules)

`docs/FILE-LAYOUT.md` is the project's most important rule for AI work — hold the line while coding:

- **≤400 lines/file hard** (PR-blocked above), ~100 typical. If a `src/lib.rs` dispatch grows, split.
- **One responsibility per file. One verb per file.** A tool bundle is a folder of verbs, not one
  `lib.rs` with a giant `match`: `src/tools/mod.rs` (the dispatch barrel) + `src/tools/ping.rs` +
  `src/tools/read_series.rs`, one file per tool. The manifest's `[[tools]] name` maps 1:1 to a file.
- **Folder-of-verbs over file-of-nouns.** Never `utils.rs` / `helpers.ts` / `common` / `misc` — name
  the concept. On the UI side: one component per `.tsx`, one hook per `use<X>.ts`, `index.ts` is a
  barrel only.
- An unknown tool name must **error, never silently succeed** (`ToolError::Failed("unknown tool: …")`).
- **The WIT world is stable — treat it as stop-and-confirm.** Don't edit the SDK `.wit` to fit your
  extension; conform. Changing a core mediation seam to reach your extension is a leak (rule 10).

## 5. Publish is a proposal, not an assumption

Building is your working loop; **publishing puts code into the running workspace** — so for the
extension-builder persona `ext.publish` is **Ask-gated**. Do not publish as a side effect of "it
built". Surface the `devkit.inspect` substance — the exact **caps requested**, the **tools** it adds,
the artifact **size** — as the proposal, and let the human approve the install.

The gateway route is `POST /extensions`. The devkit shortcut publishes a **built local folder**: the
gateway reads the built binary + manifest, signs with the node's `dev-publisher` key, **verifies the
signature before storing anything**, installs (grant = the manifest's requested ∩ admin-approved),
and hot-loads it — callable immediately, no restart.

```bash
curl -s -o /dev/null -w "%{http_code}\n" -X POST http://127.0.0.1:8080/extensions \
  -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{ "path":"/abs/repo/rust/extensions/weather-panel" }'
# 204 = published + installed + live   ·   403 = capability deny   ·   422 = verification/pack failure
```

A **native install** (`native.install` / `mcp:native.install:call`) spawns a supervised OS child —
the sharpest tool in the kit, so it is **Ask-gated too**. Same posture: propose with the inspected
substance, don't assume. (Publish-by-path handles the native sign+install for you; direct
`native.install` is the low-level seam.)

> Build before publish-by-path: the shortcut reads the *built* binary
> (`target/wasm32-wasip2/release/<id>_ext.wasm` or `target/release/<id>`) — publishing an unbuilt
> folder fails at read time.

## 6. Built ≠ done — drive what you built (pair with `core.e2e-backend`)

A green build is not a working extension. After publish, **prove the tool answers on the live node**,
then confirm it's registered and observable — the `core.e2e-backend` discipline (scaffold → build →
publish → drive the new tool → assert via the catalog/telemetry):

```bash
# 1. Call your new tool directly over its own MCP verb <id>.<tool>:
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{"tool":"weather-panel.ping","args":{}}'   # real answer, not 404

# 2. Assert it's registered + reachable in the tool catalog:
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d '{"tool":"tools.catalog","args":{}}' | jq '.[] | select(.name=="weather-panel.ping")'

# 3. Read telemetry to see the call landed (the observable proof it ran):
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{"tool":"telemetry.read","args":{}}'
```

Then the **two mandatory negatives** (per `docs/scope/testing/testing-scope.md` — a happy path alone
is not a pass): a caller **missing** `mcp:<id>.<tool>:call` gets an **opaque** denial, and a token for
**another workspace** can neither call nor see the extension (the hard wall, checked before the cap).
The reference in-process test is `rust/crates/host/tests/devkit_e2e_test.rs`; the real spawned-node
test is `ui/src/features/studio/StudioView.gateway.test.tsx`. No mocks (rule 9) — real node, real
SurrealDB (`mem://`), real cap gates.

## 7. The two authoring shapes

**WASM guest tool** (`tier="wasm"`, `src/lib.rs`) — a bundle of MCP tools behind one dispatch entry.
Generate bindings against the host's WIT world, implement `tool::Guest::call(name, input_json)`, match
the tool name, JSON in → JSON out. No ambient identity: it gets only the JSON the host passes, and
touches the platform **only** through the host-mediated callback `host.call-tool(tool, json)` —
authorized host-side on every call against `caller ∩ install-grant`. See `rust/extensions/hello`.

**Federation UI widget** (the `ui` feature, `ui/src/mount.tsx`) — a module-federation **remote** the
shell loads. It exposes one module, `./mount`, and the shell calls `mount(el, ctx, bridge)`; return an
unmount cleanup. The page reaches data **only** through `bridge.call(tool, args)` — the same
`POST /mcp/call` bridge, narrowed to the read-only tools in the manifest's `[ui] scope` (then to the
install grant server-side). It never sees a token, a DB, or a raw `fetch`. A dashboard tile is the
same shape via `[[widget]]`. See `rust/extensions/proof-panel`.

The manifest wires which shape you shipped:

```toml
[ui]                                         # only with the `ui` feature
entry = "remoteEntry.js"   label = "Weather"   icon = "cloud"
scope = ["weather-panel.ping", "series.read", "series.latest"]   # read-only tools the page may call
```

## 8. The themed UI template (Tailwind v4 + recharts + shadcn tokens)

When you scaffold with `features:["ui"]`, the template is **already themed — correct by
construction**. You do not write CSS from scratch; you wire tokens and replace sample data. The
shape below is what `devkit.scaffold` writes into `ui/` — understand it before rewriting.

- **Tailwind v4 via `@tailwindcss/vite`, with NO preflight.** A federated widget must not reset the
  host's base styles, so the template disables Tailwind's preflight. Do not re-enable it. The build
  is `pnpm install && vite build` (already in `build.sh`).
- **Theme tokens live in `src/styles/tokens.css`, scoped under `.lbx-<id>` — NEVER `:root`.** Each
  token aliases a host shadcn var with a standalone fallback, so the same artifact renders under both
  the host shell and standalone dev:
  ```css
  .lbx-weather-panel {
    --lbx-bg:      var(--background, 210 20% 98.5%);
    --lbx-chart-1: var(--chart-1, 200 80% 50%);
    /* … */
  }
  ```
  When federated into the LB shell, the host's `--background` / `--chart-1` / etc cascade through
  (the workspace theme — light or dark — wins). When developed standalone, the fallback palette
  renders. Either way the widget looks native.
- **The chart ramp: `--lbx-chart-1..8` alias `--chart-1..8`.** A recharts series reads
  `hsl(var(--lbx-chart-1))` for its stroke — it follows the workspace theme automatically, light and
  dark. Do not hardcode hex; read the token.
- **The sample chart in `src/widgets/SampleChart.tsx` is a recharts `LineChart`.** Replace its data,
  keep its stroke token (`stroke="hsl(var(--lbx-chart-1))"`). That token binding is the whole point.
- **The root wrapper in `src/mount.tsx` (`<div className="lbx-<id>">`) is load-bearing** — it anchors
  the scoped tokens from `tokens.css`. Keep it even when rewriting the page; a widget whose root
  loses the class renders unstyled (no token resolves).
- **`src_contract.ts` is a frozen mirror of the host's widget mount contract (v4). Do NOT edit it.**
  It is regenerated from the host when the world moves; treat edits as a build break, not a shortcut.

## Gotchas

- **Paths must be under the devkit root.** Pass the absolute `path` `devkit.scaffold` returned; a
  path outside `$LB_DEVKIT_ROOT` or containing `..` is rejected. `$LB_DEVKIT_ROOT` (default
  `rust/extensions`) is a *different* directory from `$LB_DIR` (default `.lazybones`, which holds the
  `dev-publisher` key).
- **`ts` on the MCP form, not on REST.** `devkit.build` over `/mcp/call` takes an optional `ts`;
  streaming needs `mcp:bus.watch:call`. The `/extensions` REST route fills its clock.
- **World major must match the host.** A manifest world whose major differs from the host SDK major is
  refused at load. Don't casually bump the major; don't edit the WIT to fit.
- **Publish status codes:** `204` ok · `403` cap deny · `422` verification/pack failure (nothing
  stored). Deny is opaque — if a call "vanishes", check the token's caps and workspace.
- **Don't publish as a reflex.** `ext.publish` and `native.install` are Ask-gated for this persona —
  they mutate the running workspace; propose with `devkit.inspect` substance and let the human decide.

## 9. The agent-driven E2E (MCP-only, no shell, no hand-edits)

The proof that an agent driving **only** the MCP API can build and ship a working extension. Every
step below is a real HTTP call to the running node — no `cp`, no `Write` tool, no backend restart
after publish. This is the exact sequence the energy-dashboard live run used (grounded in a real
`make cloud` node + `make seed-demo-sqlite` → the `demo-buildings` sqlite source).

```bash
GW=http://127.0.0.1:8080
# 1. Login (dev-login bootstraps a workspace-admin member of `acme`).
TOKEN=$(curl -s -X POST $GW/login -H 'content-type: application/json' \
  -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)

# 2. Scaffold a UI + datasources extension over MCP.
EXT=$(curl -s -X POST $GW/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d '{"tool":"devkit.scaffold","args":{"id":"energy-dashboard","tier":"wasm","features":["ui","datasources"]}}' \
  | jq -r .path)                       # /abs/repo/rust/extensions/energy-dashboard

# 3. Author the real page — replace the sample App.tsx through devkit.write_file.
#    (App.tsx calls bridge.call("federation.query", {source:"demo-buildings", sql:…}) — the ONLY
#    data path; the page never sees a token/DB/fetch.)
jq -n --rawfile body ./App.tsx \
  '{tool:"devkit.write_file",args:{path:"energy-dashboard/ui/src/App.tsx",content:$body}}' > /tmp/w.json
curl -s -X POST $GW/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' --data @/tmp/w.json
# → {"path":"…/energy-dashboard/ui/src/App.tsx","bytes":5930}

# 4. Build (wasm32-wasip2 + the federated UI bundle). Stream the log if you want progress.
TS=$(date +%s%3N)
curl -s -X POST $GW/mcp/call -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d "{\"tool\":\"devkit.build\",\"args\":{\"path\":\"$EXT\",\"ts\":$TS}}"   # → {job_id, log_subject}
# Poll devkit.inspect until built:true with a fresh wasm + remote-entry artifact.

# 5. Publish — POST /extensions signs with the node's dev-publisher key, verifies, installs, and
#    hot-loads. 204 = live, NO restart.
curl -s -o /dev/null -w "%{http_code}\n" -X POST $GW/extensions \
  -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d "{\"path\":\"$EXT\"}"               # → 204

# 6. Verify the page's data path answers (the bridge call the mounted page fires on load):
curl -s -X POST $GW/mcp/call -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{"tool":"federation.query","args":{"source":"demo-buildings","sql":"SELECT s.name AS site, ROUND(SUM(pr.value),1) AS kwh FROM point_reading pr JOIN point p ON p.id=pr.point_id JOIN meter m ON m.id=p.meter_id JOIN site s ON s.id=m.site_id WHERE p.name LIKE '"'"'%Energy kWh%'"'"' GROUP BY s.name ORDER BY kwh DESC LIMIT 8"}}'
# → {"columns":["site","kwh"],"rows":[["Riverside Data Center",49022.9],…]}

# 7. Verify the federated bundle the shell dynamic-imports is served:
curl -s -o /dev/null -w "%{http_code} %{size_download}B\n" \
  $GW/extensions/energy-dashboard/ui/remoteEntry.js          # → 200 558429B

# 8. Verify it's installed + enabled + running (REST — ext.* is not an MCP-dispatched family):
curl -s -H "authorization: Bearer $TOKEN" $GW/extensions \
  | jq '.[] | select(.ext=="energy-dashboard") | {ext,enabled,running,ui:.ui.label}'
```

Open the shell (`make ui-preview` for the built shell, or `make dev` for vite) → click
**energy-dashboard** in the sidebar → the page renders the real kWh-by-site bar chart + table,
themed from the workspace tokens. No file was touched by hand between scaffold and build; no
restart was needed after publish.

> **Calling the scaffold's own `<id>.ping` tool directly** (`POST /mcp/call` with
> `energy-dashboard.ping`) needs `mcp:<id>.<tool>:call` in the caller's cap set. The dev-login token
> carries a static cap list and the publish flow does not (yet) auto-grant a freshly published
> extension's tool call caps — see
> `docs/debugging/extensions/publish-does-not-grant-tool-call-caps.md`. For a **UI/data-dashboard**
> extension the page's `federation.query` bridge call IS the tool driving (the data path the user
> sees); the scaffold's `ping` is boilerplate. The WASM-tool E2E (G5b), where the tool itself is the
> deliverable, is the run that surfaces this cap-grant gap and is gated on resolving it.

## Related

- Operator lifecycle (enable/disable/uninstall/reset): `core.extensions` skill /
  `docs/skills/extensions/SKILL.md` — **the companion; don't duplicate it here**.
- Drive-what-you-built runbook: `core.e2e-backend` / `docs/testing/e2e-backend.md`.
- Manifest + tiers + WIT boundary (source of truth): `docs/scope/extensions/extensions-scope.md`
  (§13 manifest), `doc-site/content/public/extensions/extensions.mdx`.
- File layout the persona codes within: `docs/FILE-LAYOUT.md`.
- Devkit implementation: `rust/crates/devkit/` (scaffold/build/inspect/template/feature/signing) +
  host wiring `rust/crates/host/src/devkit/`; gateway route `rust/role/gateway/src/routes/ext.rs`.
- Reference extensions: `rust/extensions/{hello,hello-v2,echo-sidecar,proof-panel}`.
- Testing policy + the two mandatory negatives: `docs/scope/testing/testing-scope.md`; the reference
  tests `rust/crates/host/tests/devkit_e2e_test.rs`, `ui/src/features/studio/StudioView.gateway.test.tsx`.
