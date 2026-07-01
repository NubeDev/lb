---
name: extensions
description: >-
  Create, build, publish, and manage Lazybones extensions over the node gateway — scaffold a new
  extension (wasm or native tier), stream its build, publish the signed artifact into a workspace,
  then list / enable / disable / uninstall it and call its tools. Use when a task says "make/scaffold
  a new extension", "build and publish an extension", "add a devkit tool", "install/enable/disable an
  extension", "manage extensions over the API", or "call devkit.* / ext.* over MCP". Covers the
  `devkit.*` SDK verbs, the `/extensions` REST routes, the universal `POST /mcp/call` bridge, the
  `extension.toml` manifest contract, tiers/features/worlds, and the sign-and-publish lifecycle.
---

# Making & managing extensions

In Lazybones, **everything is an extension** — a chat app, a coding workplace, a flow node, a data
panel. An extension is *one folder* holding a backend (a WASM component or a native sidecar) plus an
optional federated UI, described by an `extension.toml` manifest. The node's built-in **Extension
Studio** (`ui/src/features/studio`) drives this whole flow with a UI; this skill is the same flow
over the API, for automation.

A node exposes the surface two equivalent ways over its HTTP gateway (default
`http://127.0.0.1:8080`, override with `VITE_GATEWAY_URL` / `LB_GATEWAY_URL`):

1. **The universal MCP bridge** — `POST /mcp/call {tool, args}` for ANY host verb by dotted name
   (`devkit.scaffold`, `devkit.build`, `ext.list`, …). This is rule 7: capabilities are MCP tools,
   and the UI, agents, and extensions all call them the same way.
2. **Dedicated REST routes** — `GET/POST/DELETE /extensions…` for the lifecycle verbs (ergonomic; the
   gateway supplies the clock so you omit `ts`).

Both derive the **workspace + principal from the bearer token** — never from the request body (the
hard wall, README §6/§7). Every verb is capability-gated server-side; a denial is **opaque** (you
cannot tell "forbidden" from "absent").

## The lifecycle

```
scaffold ──▶ author ──▶ build ──▶ (inspect) ──▶ publish ──▶ list / enable / disable / uninstall
 devkit      src/ +      devkit      devkit        POST         GET|POST|DELETE /extensions
.scaffold   manifest    .build     .inspect     /extensions     (or ext.* over /mcp/call)
```

Scaffold writes a folder under the devkit root (`$LB_DEVKIT_ROOT/<id>/`, default `rust/extensions/<id>/`). You
author the tool code + manifest in it. Build compiles it (streaming logs on the bus). Publish signs
the built artifact and **verify-before-stores + installs + hot-loads** it — it's callable
immediately, no restart. Then you manage it.

## 1. Authenticate

```bash
# dev login: who + which workspace. An empty workspace bootstraps the caller as workspace-admin.
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login \
  -H 'content-type: application/json' \
  -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)
```

Send `Authorization: Bearer $TOKEN` on every call. To act in another workspace, `login` into it —
the token *is* the workspace.

### Capabilities you need

| Phase | Capability string(s) |
|---|---|
| Templates | `mcp:devkit.templates:call` |
| Root (picker anchor) | `mcp:devkit.root:call` (+ `mcp:host.fs.list:call` to browse) |
| Scaffold | `mcp:devkit.scaffold:call` |
| Inspect | `mcp:devkit.inspect:call` |
| Build | `mcp:devkit.build:call` + `mcp:bus.watch:call` (to stream the log) |
| Publish | `mcp:ext.publish:call` |
| List | `mcp:ext.list:call` |
| Enable / Disable | `mcp:ext.disable:call` (one cap gates both) |
| Uninstall | `mcp:ext.uninstall:call` |
| Call a tool | `mcp:<ext_id>.<tool>:call` (e.g. `mcp:hello.echo:call`) |

## 2. Pick a template

```bash
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{"tool":"devkit.templates","args":{}}'
# → [{ "tier":"wasm",  "label":"…", "features":["ui","series-read","ingest","kv"],
#      "world":"lazybones:ext/extension@0.2.0" },
#     { "tier":"native", … }]
```

- **Tier** — `wasm` (default: a portable, in-process, capability-sandboxed WebAssembly component) or
  `native` (an OS child process the host supervises; platform-specific; for long-lived daemons /
  work the sandbox can't do).
- **World** — the WIT contract version the extension targets (`lazybones:ext/extension@0.2.0`). The
  host rejects a manifest whose world **major** doesn't match its SDK major at load time.
- **Features** — presets that expand into capability requests in the scaffolded manifest:

  | Feature | Grants (manifest `[capabilities] request`) |
  |---|---|
  | `ui` | a federated UI page + dashboard-widget bridge (no direct MCP cap) |
  | `series-read` | `mcp:series.read:call`, `mcp:series.latest:call`, `mcp:series.find:call` |
  | `ingest` | `mcp:ingest.write:call` |
  | `kv` | `mcp:template.get:call`, `mcp:template.save:call` |

## 3. Scaffold

```bash
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{
  "tool":"devkit.scaffold",
  "args":{ "id":"weather-panel", "tier":"wasm", "features":["ui","series-read"] }}'
# → { "path":"/abs/repo/rust/extensions/weather-panel", "files":[ … ] }   (absolute, under the devkit root)
```

`id` is kebab-case (`[a-z0-9-]`, no leading/trailing dash, not the reserved `devkit-build*`). **Keep
the returned `path`** — every later verb takes it. The folder it writes:

```
rust/extensions/weather-panel/
├── extension.toml        # the §13 manifest contract (identity, tier, caps, tools, visibility)
├── Cargo.toml            # wasm: [lib] crate-type=["cdylib"]  ·  native: [[bin]]
├── build.sh              # the tier's build recipe
├── src/lib.rs            # wasm entry (wit-bindgen)   ·   src/main.rs for native (lb-supervisor wire)
└── ui/                   # only if the `ui` feature was selected
    ├── package.json  vite.config.ts  tsconfig.json  index.html
    └── src/App.tsx  mount.tsx  remoteEntry.ts        # module-federation remote
```

### The manifest (`extension.toml`) — the forever contract

```toml
[extension]
id          = "weather-panel"
version     = "0.1.0"           # semver; rollback = publish/load a prior version
name        = "Weather Panel"
description = "Reads a series and draws it."

[runtime]
tier        = "wasm"            # "wasm" | "native"
world       = "lazybones:ext/extension@0.2.0"
placement   = "either"         # "local-only" | "cloud-only" | "either"  (config, NOT an `if cloud`)

[capabilities]
request     = ["mcp:series.read:call", "mcp:series.latest:call"]  # a REQUEST, not a grant

[[tools]]                       # one block per tool the extension exposes
name        = "ping"
description = "Health check."

[visibility]
class       = "private"        # "private" (this workspace) | "public" (global registry)

# native tier only:
# [native]
# exec = "weather-panel"   args = []   restart = "on-crash"   # target triple filled by build

# optional UI page (with the `ui` feature):
# [ui]
# entry = "remoteEntry.js"   label = "Weather"   icon = "cloud"
# scope = ["weather-panel.ping", "series.read"]   # read-only tools the page may call via the bridge

# optional dashboard tiles / flow nodes:
# [[widget]]  entry="widget.js"  label="Weather tile"  scope=["series.read"]
# [[node]]    …flow node type contribution…
```

The host grants `requested ∩ admin-approved` at install — the manifest asks, the workspace decides.

## 4. Author the tool code

Scaffold gives you working stubs; here's the shape each entry point takes. **The rule spanning all
three: an extension is stateless (rule 4)** — hold nothing durable in the instance; everything comes
in on the call, state lives in the store/bus. That's what makes a hot-reload swap
(`hello` → `hello-v2`, same id) lossless.

### wasm tier — `src/lib.rs`

A wasm extension is a bundle of MCP tools behind one dispatch entry point. Generate bindings against
the host's WIT world (the scaffold wires the path), implement `tool::Guest::call`, match on the tool
name, JSON in → JSON out:

```rust
wit_bindgen::generate!({ path: "../../sdk/wit", world: "extension" });
use serde::{Deserialize, Serialize};

#[derive(Deserialize)] struct PingIn { #[serde(default)] ws: String }
#[derive(Serialize)]   struct PingOut { ok: bool, ws: String }

struct WeatherPanel;
impl exports::lazybones::ext::tool::Guest for WeatherPanel {
    fn call(name: String, input_json: String)
        -> Result<String, exports::lazybones::ext::tool::ToolError>
    {
        use exports::lazybones::ext::tool::ToolError;
        lazybones::ext::host::log(&format!("weather-panel.{name} called"));  // fire-and-forget audit
        match name.as_str() {
            "ping" => {
                let in_: PingIn = serde_json::from_str(&input_json)
                    .map_err(|e| ToolError::BadInput(e.to_string()))?;
                serde_json::to_string(&PingOut { ok: true, ws: in_.ws })
                    .map_err(|e| ToolError::Failed(e.to_string()))
            }
            other => Err(ToolError::Failed(format!("unknown tool: {other}"))),  // never a silent ok
        }
    }
}
export!(WeatherPanel);
```

Cargo: `[lib] crate-type = ["cdylib"]`, deps `wit-bindgen`, `serde`, `serde_json`. The manifest's
`[[tools]] name` is what the host routes `<ext>.<name>` to; an unknown name must **error, never
silently succeed**. A wasm guest has **no ambient identity** — the WIT `call(name, input-json)` ABI
hands it only the JSON the host passes; if a tool needs the workspace, the caller passes it in an arg
(the real wall is the host's cap gate, not the arg).

### Calling a capability back into the host (`world @0.2.0`)

A guest touches the platform ONLY through the host-mediated callback — never a DB handle or token.
`host.call-tool(tool, json)` invokes any MCP tool the guest is granted (`series.*`, `ingest.write`,
another ext's `<ext>.<tool>`), authorized **host-side on every call** against the guest's *effective*
principal = `caller ∩ install-grant`. Declare `world = "…@0.2.0"` in the manifest to use it.

```rust
use lazybones::ext::host::{call_tool, ToolError as HostErr};
// read the latest sample of a series, then commit a derived one — both host-authorized:
let latest = call_tool("series.latest", &serde_json::json!({ "series": "proof.demo" }).to_string())
    .map_err(|e| match e { HostErr::Failed(m) => /* an honest failure */ m, HostErr::BadInput(m) => m })?;
let _ = call_tool("ingest.write", &serde_json::json!({ "samples": [/* … */] }).to_string())?;
```

If the install grant omits `ingest.write` (or the caller lacks it), that call is **denied at the
host** even though the guest asked — the denial surfaces as a `ToolError::Failed`, never swallowed.
A re-entrant guest→host→guest chain is bounded by a host depth guard (`"call depth exceeded"`).

### native tier — `src/main.rs`

A native extension is an OS child the host supervises over `Content-Length`-framed stdio, using the
`lb-supervisor` wire types (so the child↔host ABI can't drift — the native peer of the WIT world).
Read your injected identity from the env, then serve `init` / `health` / `call` / `shutdown`:

```rust
use lb_supervisor::{read_frame, write_frame, CallParams, Method, Reply, Request};
use tokio::io::{stdin, stdout};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let ws = std::env::var("LB_EXT_WS").unwrap_or_default();      // injected scoped identity
    let (mut i, mut o) = (stdin(), stdout());
    while let Ok(body) = read_frame(&mut i).await {
        let req: Request = match serde_json::from_slice(&body) { Ok(r) => r, Err(_) => continue };
        let reply = match req.method {
            Method::Init     => Reply::ok(req.id, r#"{"ready":true}"#.into()),
            Method::Health   => Reply::ok(req.id, "ok".into()),
            Method::Shutdown => break,                            // ack + exit cooperatively
            Method::Call     => {
                let p: CallParams = serde_json::from_str(&req.params).unwrap();
                match p.tool.as_str() {
                    "echo" => Reply::ok(req.id, format!(r#"{{"echo":{},"ws":"{ws}"}}"#, p.input)),
                    other  => Reply::err(req.id, format!("unknown tool: {other}")),
                }
            }
        };
        if write_frame(&mut o, &serde_json::to_vec(&reply).unwrap()).await.is_err() { break }
    }
}
```

Cargo: `[[bin]] name = "<id>"`, deps `lb-supervisor`, `serde`, `serde_json`, and its **own** `tokio`
with `["rt","macros","io-util","io-std"]` (the child owns its stdio; the host crates don't). Manifest
`[native] exec` must match the bin name; `restart = "on-crash"` gets it respawned. Still stateless —
a kill + respawn loses nothing; the host's record is the truth.

### the UI half — `ui/src/mount.tsx`

If you took the `ui` feature, the folder ships a **federated remote** the shell loads. It exposes one
module, `./mount`, and the shell calls `mount(el, ctx, bridge)`:

```tsx
export function mount(el: HTMLElement, ctx: MountCtx, bridge: Bridge): () => void {
  const root = createRoot(el);
  root.render(<App ctx={ctx} bridge={bridge} />);
  return () => root.unmount();       // return an unmount cleanup
}
```

The page reaches data **only** through `bridge.call(tool, args)` — the same host bridge (`POST
/mcp/call`) every surface uses; it never sees a token, a DB, or a raw `fetch`. It may call only the
read-only tools in the manifest's `[ui] scope` (narrowed to the install grant server-side):

```toml
[ui]
entry = "remoteEntry.js"   label = "Weather"   icon = "cloud"
scope = ["weather-panel.ping", "series.read", "series.latest"]
```

## 5. Build (async, streams logs)

```bash
STARTED=$(curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{
  "tool":"devkit.build","args":{ "path":"/abs/repo/rust/extensions/weather-panel", "ts":1719829200000 }}')
# → { "job_id":"devkit-build-<ts>", "log_subject":"devkit/build/devkit-build-<ts>" }
SUBJECT=$(echo "$STARTED" | jq -r .log_subject)
```

`ts` is optional (defaults to `0`) but it seeds the `job_id` / `log_subject` — pass a unique
millisecond value so concurrent or repeated builds don't collide on the same log stream.

Build runs `cargo build --target wasm32-wasip2 --release` (wasm) or `cargo build --release`
(native), then `pnpm install && pnpm exec vite build` if a `ui/` folder exists. Each output line is
published on the bus. **Stream it** over SSE using the `log_subject` from the response verbatim (don't
hand-build the subject):

```bash
curl -sN "http://127.0.0.1:8080/bus/stream?subject=$(python3 -c \
  "import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1]))" "$SUBJECT")&token=$TOKEN"
# data:"   Compiling weather-panel v0.1.0"
# data:"    Finished release [optimized] target(s)"
# data:"devkit build: done"          ← terminal success line
# data:"devkit build: failed"        ← terminal failure line
```

The two terminal lines are `devkit build: done` / `devkit build: failed`. (SSE needs the token as a
query param because `EventSource` can't set headers.)

### Confirm it built

```bash
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{
  "tool":"devkit.inspect","args":{ "path":"/abs/repo/rust/extensions/weather-panel" }}'
# → { "id":"weather-panel", "tier":"wasm", "tools":["ping"], "caps":["mcp:series.read:call", …],
#     "built":true,
#     "toolchain":{ "cargo":true, "pnpm":true, "wasm32_wasip2":true } }
```

`inspect` is your pre-flight: `built` tells you an artifact exists; `toolchain` tells you whether the
build *can* succeed. Check it **before** building — a doomed build (missing `wasm32-wasip2` target or
`pnpm`) is otherwise a slow way to find out.

- **wasm tier** needs `cargo` + the `wasm32-wasip2` target (`rustup target add wasm32-wasip2`).
- **native tier** needs `cargo`.
- **any UI** (`ui/` present) needs `pnpm`.

## 6. Publish (sign → verify-before-store → install → hot-load)

The gateway route is `POST /extensions`. It accepts **two body shapes**:

**(a) The devkit shortcut — publish a built local folder.** The gateway reads the built binary +
manifest, signs with the node-owned `dev-publisher` key (auto-created at `$LB_DIR/keys/dev-publisher.key`),
and publishes. This is what the Studio uses.

```bash
curl -s -o /dev/null -w "%{http_code}\n" -X POST http://127.0.0.1:8080/extensions \
  -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{ "path":"/abs/repo/rust/extensions/weather-panel" }'
# 204 = published & installed & live   ·   403 = capability deny   ·   422 = verification/pack failure
```

**(b) A full signed `Artifact`** — the registry wire shape, verified against the *workspace's* trusted
publisher keys (`gw.trusted`). Produced by external publisher tooling (`lb-pack`, the SDK's
`sign_artifact`); the console never mints it. Body fields: `ext_id, version, manifest_toml,
wasm(number[]), digest_hex, publisher_key_id, signature(number[])`.

Either way the host **verifies the signature before storing anything** — a tampered / unsigned /
foreign-key upload is rejected and nothing is persisted. On success the extension is installed
(grant = the manifest's requested caps) and loaded live.

## 7. Manage installed extensions

| Action | REST route | MCP bridge (`POST /mcp/call`) | Cap |
|---|---|---|---|
| List (live state) | `GET /extensions` | `{"tool":"ext.list","args":{}}` | `mcp:ext.list:call` |
| Enable (start) | `POST /extensions/{ext}/enable` | `{"tool":"ext.enable","args":{"ext":"…","ts":…}}` | `mcp:ext.disable:call` |
| Disable (stop) | `POST /extensions/{ext}/disable` | `{"tool":"ext.disable","args":{"ext":"…","ts":…}}` | `mcp:ext.disable:call` |
| Uninstall | `DELETE /extensions/{ext}` | `{"tool":"ext.uninstall","args":{"ext":"…","ts":…}}` | `mcp:ext.uninstall:call` |

```bash
curl -s http://127.0.0.1:8080/extensions -H "authorization: Bearer $TOKEN"
# → [{ "ext":"weather-panel", "version":"0.1.0", "tier":"wasm",
#      "enabled":true, "running":true, "health":"ok", "restart_count":0,
#      "ui":{ "entry":"remoteEntry.js", "label":"Weather", "icon":"cloud", "scope":[…] },
#      "widgets":[…] }]
```

`enabled` is the durable intent the boot reconciler honors; `running` is live (for native, joined
from the supervisor's PID map — wasm is `running = enabled`). **Enable/disable flips intent** (the
install stays); **uninstall tombstones the install** + stops any native child + evicts the binary.

## 8. Call the extension's tool

Once published, every declared tool is a first-class MCP verb `<ext_id>.<tool>` behind
`mcp:<ext_id>.<tool>:call`:

```bash
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{"tool":"weather-panel.ping","args":{}}'
```

## End-to-end (one script)

```bash
BASE=http://127.0.0.1:8080
TOKEN=$(curl -s -X POST $BASE/login -H 'content-type: application/json' \
  -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)
mcp(){ curl -s -X POST $BASE/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d "{\"tool\":\"$1\",\"args\":$2}"; }

PATH_=$(mcp devkit.scaffold '{"id":"weather-panel","tier":"wasm","features":["ui","series-read"]}' | jq -r .path)
SUB=$(mcp devkit.build "{\"path\":\"$PATH_\"}" | jq -r .log_subject)
curl -sN "$BASE/bus/stream?subject=$(jq -rn --arg s "$SUB" '$s|@uri')&token=$TOKEN" \
  | grep -m1 'devkit build: done'                       # block until built
[ "$(mcp devkit.inspect "{\"path\":\"$PATH_\"}" | jq -r .built)" = true ] || { echo "not built"; exit 1; }
curl -s -o /dev/null -w "publish: %{http_code}\n" -X POST $BASE/extensions \
  -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' -d "{\"path\":\"$PATH_\"}"
mcp weather-panel.ping '{}'
curl -s $BASE/extensions -H "authorization: Bearer $TOKEN" | jq '.[].ext'
```

## Test it (no mocks — the real chain)

Rule 9: **nothing that can run in-process gets mocked.** An extension is tested by exercising the
*real* chain — scaffold → build (real toolchain) → sign → publish (real host) → call — against a real
node with a real SurrealDB (`mem://`) and real capability gates. A `*.fake.ts` or a hand-stubbed host
is banned: it lets the work *look* done while the real path is unbuilt. Two levels:

- **Rust host test** (`Node::boot()`, in-process). The reference is
  `rust/crates/host/tests/devkit_e2e_test.rs`: it `scaffold_extension` → `build_extension` (real
  cargo) → `sign_artifact` → `ext_publish` → `call_tool` / `call_sidecar`, and asserts the generated
  tool actually answers (`"tier":"wasm"` / `"native"`). Copy its `publish_generated` helper for a new
  extension's happy-path test.
- **UI gateway test** (`cd ui && pnpm test:gateway`, a **real spawned node**). The reference is
  `ui/src/features/studio/StudioView.gateway.test.tsx`: `signInWithCaps(...)` mints a token with the
  exact caps, then drives `listDevkitTemplates → scaffold → build` (collecting real SSE log lines,
  asserting `devkit build: done`) `→ inspect → publish → call the generated tool`. No fake backend —
  `vitest.gateway.config.ts`, per rule 9.

### The two mandatory tests (every extension, every session)

Per `docs/scope/testing/testing-scope.md`, a happy path alone is not a pass. Both of these must be
green:

1. **Capability-deny.** A caller **missing** `mcp:<ext>.<tool>:call` gets an **opaque** denial — a
   published, working extension is still unreachable without the grant.

   ```bash
   DENIED=$(curl -s -X POST $BASE/login -H 'content-type: application/json' \
     -d '{"user":"user:mallory","workspace":"acme"}' | jq -r .token)   # same ws, NO tool cap
   curl -s -o /dev/null -w "%{http_code}\n" -X POST $BASE/mcp/call \
     -H "authorization: Bearer $DENIED" -H 'content-type: application/json' \
     -d '{"tool":"weather-panel.ping","args":{}}'                      # 403 — and indistinguishable
   ```                                                                 # from "no such tool"

2. **Workspace-isolation.** A token for **workspace B** can neither call nor see workspace A's
   extension — the hard wall (§7), checked before the capability.

   ```bash
   OTHER=$(curl -s -X POST $BASE/login -H 'content-type: application/json' \
     -d '{"user":"user:ada","workspace":"other-ws"}' | jq -r .token)   # different workspace
   curl -s $BASE/extensions -H "authorization: Bearer $OTHER" | jq 'length'   # 0 — A's ext invisible
   curl -s -o /dev/null -w "%{http_code}\n" -X POST $BASE/mcp/call \
     -H "authorization: Bearer $OTHER" -H 'content-type: application/json' \
     -d '{"tool":"weather-panel.ping","args":{}}'                      # denied
   ```

Encode both as assertions in the Rust or gateway test (deny → `ExtError::Denied` / a 403; isolation →
empty list + denied call), not just as a manual curl.

> **Toolchain caveat.** These tests run a **real build**, so the host needs `cargo` +
> `wasm32-wasip2` (and `pnpm` for a UI). Call `devkit.inspect` (or read `ToolchainReadiness`) first;
> a missing target is a skipped/red build, not a logic bug.

## Non-negotiable rules these encode

- **Stateless extensions** (rule 4). An instance holds no durable state — it lives in SurrealDB or on
  the bus. This is what makes publish-a-new-version a safe **hot-reload** (`hello` → `hello-v2`, same
  id, higher version, no state loss).
- **Capability-first** (rule 5). Nothing is reachable except through a host-mediated cap check;
  workspace isolation is checked first, then the cap. The manifest *requests*; the workspace *grants*.
- **Workspace is the hard wall** (rule 6). Every record is workspace-scoped; the workspace comes from
  the token, never the body. A workspace-B token can never see workspace-A extensions.
- **Signed registry** (S7). Publish is **verify-before-store**: a signature that doesn't verify
  against the workspace's trusted keys stores nothing.
- **Symmetric nodes** (rule 1). `placement` is metadata; there is no `if cloud` branch. The same
  binary runs edge or cloud, role by config.

## Gotchas

- **Paths must be under the devkit root.** `inspect` / `build` / publish-by-path resolve every path
  under `$LB_DEVKIT_ROOT` (default `rust/extensions`); anything outside it — or with a `..` — is
  rejected. Pass the **absolute** `path` that `devkit.scaffold` returned (or a path relative to the
  root). `devkit.root` returns that absolute root (the Studio's folder picker browses from it). Note
  this is a *different* directory from `$LB_DIR` (default `.lazybones`), which only holds the
  node's `dev-publisher` key.
- **`ts` on `/mcp/call`, not on REST.** The `ext.enable|disable|uninstall` MCP forms need a
  millisecond `ts` in `args` (determinism, README §3: no wall-clock inside a verb). The `/extensions`
  REST routes fill it from the gateway clock — omit it there.
- **Stream with the returned `log_subject` verbatim.** Don't reconstruct the bus subject by hand; the
  `devkit.build` response gives you the exact string. Streaming needs `mcp:bus.watch:call`.
- **Publish status codes:** `204` ok · `403` capability deny · `422` verification/pack failure. A
  `422` means *nothing* was stored.
- **One cap gates enable *and* disable.** Both `ext.enable` and `ext.disable` authorize on
  `mcp:ext.disable:call` — there is no separate `mcp:ext.enable:call`. Grant the disable cap to allow
  either.
- **Denials are opaque.** A missing cap and a missing/foreign-workspace record both read as
  forbidden/absent. If a call "vanishes", check your token's caps and workspace.
- **Build before publish-by-path.** The shortcut reads the *built* binary
  (`target/wasm32-wasip2/release/<id>_ext.wasm` or `target/release/<id>`); publishing an unbuilt
  folder fails at read time.
- **World major must match.** A manifest world whose major differs from the host SDK major is
  rejected at load — bump the minor freely (0.1→0.2 is compatible), not the major casually.
- **`id` rules:** kebab-case, no leading/trailing dash, not the reserved `devkit-build*` prefix.

## Related

- Extension model + tiers + manifest (source of truth): `docs/public/extensions/extensions.md`,
  `docs/scope/extensions/extensions-scope.md` (§13 manifest contract).
- Devkit implementation: `rust/crates/devkit/` (scaffold/build/inspect/template/feature/signing),
  host wiring `rust/crates/host/src/devkit/`, gateway routes `rust/role/gateway/src/routes/ext.rs`.
- The Studio UI that drives this flow: `ui/src/features/studio/` (and its
  `StudioView.gateway.test.tsx`, which exercises the real scaffold→build→publish→call chain).
- Real examples: `rust/extensions/hello` (minimal wasm), `rust/extensions/hello-v2` (version swap),
  `rust/extensions/echo-sidecar` (native), `rust/extensions/proof-panel` (UI page + widget).
- Capability / workspace rules: `README.md` §3, §5, §6, §7; `docs/scope/auth-caps/`.
