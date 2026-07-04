# App scope — mobile extension model (federated app remotes)

Status: scope (the ask). Promotes to `public/app/` once shipped.

Let an extension ship a **mobile UI** the same way it ships a web UI today: an additive
`[app]` block in `extension.toml`, a JS-only Module Federation remote built with
Re.Pack, published through the unchanged signed-`Artifact` path, discovered via
`ext.list`, and mounted by the RN shell behind the same host-mediated bridge. Two
reference extensions prove it end to end: **`proof-panel-app`** (the mobile companion
of the existing wasm `proof-panel` backend extension) and **`channel-chat`** (a
pure-app extension over channels + the in-channel agent).

## Goals

- One manifest per extension stays the single source of truth: `[app]` sits beside
  `[ui]` and `[[widget]]` in `extension.toml`; the host and `ext.list` treat it as
  opaque data (rule 10 — no core branch on any extension id).
- An app remote is **JS/TS only** — it may not require native modules the shell
  doesn't already ship. Enforced at build (devkit) and stated at publish.
- The mount contract is **component-based** (RN has no DOM): the remote exposes React
  components; the shell passes the same `ctx`/`bridge` the web contract carries.
- Publishing reuses the signed registry unchanged: same `Artifact` shape, same
  digest/signature verification, app bundle served by the gateway under
  `/extensions/{ext}/app/`.
- The two examples ship as real, running extensions — no placeholder screens.

## Non-goals

- No native-code extension tier on mobile (no per-extension `.so`/frameworks). The
  sanctioned escape hatch for native needs stays the backend Tier-2 extension on a
  node, reached over MCP.
- No app-side extension *backends* — an app remote is UI only; logic lives in the
  extension's wasm/native component on the node (stateless-extension rule unchanged).
- No independent theming system — the app inherits the shell theme tokens (the
  `extensions/ui/theme-inheritance-scope.md` doctrine, RN-flavored).

## Intent / approach

Mirror the web federation seam one-for-one, changing only what a phone forces:

| Seam | Web (shipped) | App (this scope) |
|---|---|---|
| Build | Vite lib → `remoteEntry.js` (ESM) | Re.Pack 5 (Rspack) + MF2 → `remote.container.js.bundle` |
| Load | `import(url)` + import-map React shims | MF2 runtime container fetch; shared singletons `react`, `react-native`, `@nube/app-sdk` |
| Serve | gateway `/extensions/{ext}/ui/` | gateway `/extensions/{ext}/app/` |
| Contract | `mount(el, ctx, bridge)` / `mountWidget(el, ctx, bridge, widgetId)` | `Page({ctx, bridge})` / `Widget({ctx, bridge, widgetId})` React components |
| Bridge | `{ call, watch }`, scope-filtered, token never crosses | identical — same type, same semantics |
| Trust | trusted publisher in-process; untrusted → iframe sandbox | trusted publisher in-process; **untrusted app remotes deferred** (no iframe equivalent — see risks) |

The **manifest** gains one additive block (host treats every field as data):

```toml
[app]
entry = "app/dist/remote.container.js.bundle"   # served under /extensions/{ext}/app/
scope = ["proof-panel.*", "series.latest"]      # bridge tool allow-list, same as [ui].scope
```

`[[widget]]` rows gain nothing: a widget is app-capable when the extension's app remote
exposes `Widget` and the shell finds an `[app]` block — the widget id vocabulary is
shared with the web (`ext.list` already carries `widgets?`).

**Contract shape** (authored in `@nube/app-sdk`, the third mirror alongside
`ui/src/features/dashboard/builder/federationWidget.ts` and the devkit template — the
`ctx` types are byte-identical to the web v3 frames-in contract; only the mount
mechanics differ):

```ts
export interface AppRemote {
  Page?: React.ComponentType<{ ctx: MountCtx; bridge: Bridge }>;
  Widget?: React.ComponentType<{ ctx: WidgetCtx; bridge: WidgetBridge; widgetId: string }>;
}
```

**Rejected alternative — WebView-hosted web remotes.** Loading each extension's
existing web `remoteEntry.js` in a WebView would need zero new ext-side build, but
gives non-native look/feel/perf, a heavy bridge boundary per widget, and an awkward
path to the shared panel SDK. Kept as the *fallback* posture for an extension that
ships `[ui]` but no `[app]` (the shell may offer "open web view"), not the model.

### The two reference extensions

1. **`proof-panel-app`** (`app/extensions/proof-panel-app/`) — the mobile companion of
   the existing Tier-1 wasm `rust/extensions/proof-panel`. Proves the **backend-ext ↔
   app-ext pairing**: `Page` calls `proof-panel.proof.ping` / `proof.derive` through
   the scoped bridge; `Widget` renders the `proof.derived` series with `bridge.watch`
   (SSE). Its `[app]` block is added to the **existing** `proof-panel/extension.toml`;
   this folder is the RN bundle source that `[app].entry` points at (the app sibling
   of `proof-panel/ui/`).
2. **`channel-chat`** (`app/extensions/channel-chat/`) — a **pure-app extension** (no
   backend component; the manifest lives in its own folder with no `[runtime]`/tools):
   channel list → history → live stream → post, plus "ask the agent" via
   `channel.post kind:"agent"` rendering the RunEvent stream. Proves an extension can
   be *only* a phone surface while every byte still flows through granted verbs.

Between them the user's four requirements are exercised for real: multi-workspace
(shell), server connection (gateway REST+SSE), AI agent (`channel-chat`), channels
(`channel-chat`), and the widget SDK seam (`proof-panel-app`'s `Widget`).

## How it fits the core

- **Tenancy / isolation:** `ctx.workspace` comes from the session; the bridge rides
  the shell's token; a remote never holds the token. Same as web.
- **Capabilities:** `[app].scope` narrows the bridge client-side (defense in depth);
  the host re-checks every call at the one chokepoint. Deny inside a remote renders as
  a typed error, not a blank tile. `granted = requested ∩ admin_approved` unchanged —
  the reference extensions get no special treatment (rule 10).
- **Placement:** N/A — app remotes are client artifacts; the extension's placement
  rules concern its backend component only.
- **MCP surface:** consumed through the bridge (`call` → `POST /mcp/call`, `watch` →
  the gateway SSE routes). **No new verbs.** `ext.list` already returns the manifest
  surface; it must simply pass `[app]` through as data.
- **Data / Bus:** none directly; state and motion reach a remote only as bridge
  results and watch events.
- **Sync / authority / Secrets:** N/A beyond the shell's token handling.
- **Stateless extensions:** an app remote is stateless by construction (UI instance);
  anything durable goes through verbs to SurrealDB.
- **SDK/WIT impact:** **none** — the WIT ABI is untouched. The impact is on the
  *federation contract* mirrors: `@nube/app-sdk` becomes a contract mirror that must
  move in lockstep with `federationWidget.ts` and the devkit template. Flagged loudly:
  contract changes now touch **four** mirrors (host web, ext web copy, devkit
  template, app-sdk).

## Example flow

1. Developer runs the devkit generator with the app template → `app/extensions/<name>/`
   scaffold (Re.Pack config, contract imports from `@nube/app-sdk`, `build.sh`).
2. `build.sh` → `remote.container.js.bundle`; the build **fails** if the dependency
   graph pulls a native module outside the shell's shared set (the JS-only gate).
3. Publish through the unchanged signed path: bundle + manifest → `Artifact`
   (digest, publisher key, signature) → host verifies → stores → serves under
   `/extensions/{ext}/app/`.
4. Phone shell's `ext.list` shows the ext with `[app]`; nav entry appears (cap-gated).
5. User taps it; shell fetches + verifies the container, mounts `Page` with
   `{ ctx, bridge }`; widgets appear in any app dashboard surface via `Widget`.
6. Admin disables the extension → `ext.list` drops it → nav entry and mounted
   surfaces unmount on next refresh.

## Testing plan

Real infra per `scope/testing/testing-scope.md`; the two reference extensions are the
test vehicles (the proof-panel doctrine: no placeholders):

- **Mandatory capability-deny:** `channel-chat` with a token lacking
  `mcp:channel.post:call` — post denied through the bridge, typed error rendered;
  `proof-panel-app` bridge call outside `[app].scope` rejected client-side AND a
  scope-bypassing direct call rejected by the host.
- **Mandatory workspace-isolation:** the same extension mounted under two workspace
  tokens sees disjoint channels/series (real seeded records, real gateway).
- **Publish/verify:** tampered bundle (digest mismatch) refuses to mount; unsigned
  publisher refused.
- **Contract:** an app remote built from the devkit template mounts against the shell;
  `Widget` receives v3 frames (`ctx.data`) resolved by the shell — the same fixtures
  as the web `federationWidget` tests, proving the mirrors agree.
- **JS-only gate:** a fixture extension importing a native module fails `build.sh`
  with the named module in the error.
- **Hot-swap:** publish v2 of `channel-chat` while mounted → next mount serves v2; no
  durable state lost (there is none to lose — stateless rule).

## Risks & hard problems

- **Contract mirror count.** Four copies that must move together is the top drift
  risk. Mitigation: `@nube/app-sdk` types become the authored source and the web
  mirrors are checked against it in CI (a later slice; note it in the session doc).
- **Untrusted remotes.** Web sandboxes untrusted publishers in an iframe; RN has no
  equivalent cheap sandbox. v1: only **trusted-publisher** extensions mount in the
  app; untrusted `[app]` blocks are listed but not mountable. A JS-VM sandbox tier is
  a future scope — do not improvise one.
- **Shared-singleton version skew.** A remote built against a different RN/React minor
  than the shell can fail at runtime. The manifest's `[app]` should carry the sdk
  version it was built against, and the shell refuses mismatches loudly.
- **JS-only enforcement is a build-time promise**, not a runtime guarantee — the
  signed-publish + trusted-publisher gate is what actually holds the line in v1.

## Open questions

- Does `[app].entry` versioning ride the existing artifact `version` alone, or do we
  add an `sdk` compat field (`app.sdk = "0.1"`)? (Lean: add the compat field now;
  refusing skew needs it.)
- Widget surface in the app v1: mount extension `Widget`s inside `channel-chat`-style
  rich responses only, or ship a phone dashboard surface too? (Lean: rich responses
  first; the dashboard surface arrives with the shared SDK, `app-sdk-scope.md`.)
- Devkit: extend `lb devkit` generate/build/publish with an `--app` template in this
  slice or the next? (Lean: same slice — the examples need `build.sh` anyway; folding
  it into devkit avoids a bespoke script surviving forever.)

## Skill doc

N/A for new verbs (none added). The devkit `--app` template extends an existing
drivable surface — the implementing session updates the existing devkit skill doc
rather than creating a new one.

## Related

- `app-shell-scope.md` (the host), `app-sdk-scope.md` (the contract package)
- `../extensions/extensions-scope.md`, `../extensions/ui-federation-scope.md`,
  `../extensions/proof-panel-scope.md`, `../extensions/ext-sdk-scope.md`
- `../extensions/ui/theme-inheritance-scope.md`, `../extensions/ui/css-isolation-scope.md`
  (the web contracts whose intent carries over)
- `rust/extensions/proof-panel/` (the backend half of example 1),
  `rust/extensions/echarts-panel/` (the web widget reference)
- README §3 rule 10 (core knows no extension), §6.4 (signed registry)
