# Extensions scope — UI federation (mount an extension's own pages inside the shell)

Status: scope (the ask). Promotes to `public/extensions/` once shipped. Target stage: **S10 (extension
UX)**. The UI counterpart to the extension *runtime* + *lifecycle* + *registry* scopes
(`lifecycle-management-scope.md`, `native-tier-scope.md`, `registry-scope.md`): those let a node
**install, supervise, and manage** an extension; this scope lets an installed extension **render its own
UI** inside the shared shell. It is the piece the admin-console scope explicitly deferred
(`frontend/admin-console-scope.md` Non-goals: "Mounting an extension's own UI pages is the deferred
`scope/extensions/` UI-federation scope. This console **manages** extensions; it does not host their
pages."). README **§6.13**: *"Extension UIs: module federation for trusted first-party extensions; Web
Components or iframes for untrusted ones. Design tokens exposed so extension UIs look native."*

The platform's whole premise is **"everything else is an extension"** (CLAUDE.md): chat, coding-agent
workplace, flow tool, document store. Today an extension can ship **capabilities** (MCP tools) and
**lifecycle**, but its **pages** have nowhere to live — the shell only renders the core surfaces
(channels · members · inbox · outbox · admin · extensions). This scope gives an extension a **slot in
the nav and a host for its page**, so a first-party extension's React UI mounts natively and an
untrusted one runs sandboxed — without either being trusted with the session token or the host's caps.

## Goals

- **A page-host surface in the shell** — the shell gains a generic `features/ext-host/` surface that,
  given an installed extension that *declares a UI*, renders that extension's page in a nav slot
  (cap-gated like every other surface). The core surfaces are unchanged; extension pages are additive.
- **Two trust tiers, one contract** (README §6.13):
  - **Trusted (first-party) → module federation.** The extension ships a federated remote (an ESM
    bundle exposing a mount component); the shell loads it at runtime and renders it in-process. Fast,
    native-feeling, shares the design tokens. Allowed only for an **admin-trusted publisher key**
    (the registry allow-list already exists — `lb-registry::TrustedKeys`).
  - **Untrusted (third-party) → iframe / Web Component sandbox.** The page runs in an isolated browsing
    context; it talks to the host **only** through a narrow, audited `postMessage` bridge — never the
    DOM, the token, or `invoke` directly.
- **A host-mediated capability bridge for extension pages** — an extension page calls platform verbs
  the SAME way everything else does: **MCP tools, host-mediated, capability-checked** (Non-negotiable
  rule 7: "MCP is the universal contract"). The page never holds the session token; it asks the host
  (`postMessage` → shell → `invoke`), and the host re-checks the cap and the workspace on every call.
  A page can reach **only** the tools its extension's install record was granted (`requested ∩
  admin_approved`, the S4 intersection) — the page is a *gated caller, never a trusted decider*
  (inherited from `authz-grants`).
- **Design tokens exposed** so a trusted federated page looks native (the shell's Tailwind/shadcn
  tokens are published as CSS variables the remote consumes); an untrusted iframe gets the tokens too
  but cannot reach beyond its frame.
- **A UI declaration in the extension manifest** — an extension states `[ui]` (entry, tier-of-trust,
  nav label/icon, requested tool scope). The host reads it on install; the shell reads it to build the
  nav slot. No UI declared → no page surface (the lifecycle/console story is unchanged).
- **A reference UI extension** — a minimal first-party extension that ships a page (e.g. a "hello-ui"
  that lists the workspace's channels via `channel.list` through the bridge), proving the whole path:
  install → declare UI → mount → call a capability → get a workspace-scoped, cap-checked result.

## Non-goals

- **No new core surfaces.** This hosts *extension* pages; it does not redesign channels/inbox/etc.
- **No bypass of the capability or workspace gates.** An extension page is the weakest principal in the
  system, not a privileged one. It gets a `postMessage` bridge to host-mediated MCP, nothing more. No
  direct `invoke`, no token, no DOM access across the trust boundary. (This is the #1 risk — see below.)
- **No arbitrary remote code from untrusted publishers in-process.** Module federation (in-process) is
  **trusted-publisher-only**; untrusted always sandboxes (iframe/Web Component). The trust tier is the
  publisher key's allow-list status, not the extension's say-so.
- **No server-side rendering / page federation across nodes.** A page is delivered by the node the
  browser is already talking to (the hub for `browser`/`mobile`, Tauri-local for the workstation). Same
  app, two transports — the symmetric-node story, unchanged. Cross-node page routing is out.
- **No extension *theming* override of the shell.** Tokens are exposed read-only; a page styles within
  them. It cannot restyle the shell chrome.
- **No marketplace / discovery UI.** Installing/uploading is the lifecycle+console story (already
  shipped). This scope mounts what is installed; a public catalog browser is a later scope.

## Intent / approach

**One page-host, two renderers, one host-mediated bridge.** The shell gets a `features/ext-host/`
surface. When an installed extension declares a UI, the shell adds a cap-gated nav slot; selecting it
renders the page through the renderer its trust tier dictates:

```
  manifest [ui] ──► host reads on install ──► Install record carries the UI decl (+ granted tool scope)
        │
        ▼
  shell nav slot (cap-gated)  ──select──►  features/ext-host/ExtHost
        │                                        │
        │  trusted (allow-listed key)            │  untrusted (any other)
        ▼                                        ▼
  module-federated remote                  sandboxed <iframe>/Web Component
  (in-process ESM, shares tokens)          (isolated context, tokens via CSS only)
        │                                        │
        └──────────► postMessage bridge ◄────────┘
                            │  { tool, args }  (NEVER the token)
                            ▼
                     shell → invoke(tool) → gateway/Tauri → HOST
                            │  re-checks workspace (token) + cap, per call
                            ▼
                     result ──► postMessage back ──► page
```

- **The bridge is the security seam.** The page posts `{tool, args}`; the shell (which *does* hold the
  token) calls `invoke`, the host re-checks the cap + workspace, and the result is posted back. The page
  is denied exactly as a forged call is denied — the gateway is the boundary, the bridge is plumbing.
  The shell **filters** the tool to the extension's granted scope before forwarding (defense in depth;
  the host re-checks regardless).
- **Trusted = module federation.** A first-party remote exposes a `mount(el, bridge, tokens)` contract.
  The shell loads it lazily (Vite/webpack module federation) only when its nav slot is opened. The
  publisher-key allow-list (`TrustedKeys`, already used by the registry verify) decides "trusted".
- **Untrusted = iframe sandbox.** `sandbox="allow-scripts"` (no same-origin, no top navigation); the
  only channel is `postMessage`. The page gets the design tokens as CSS variables on the frame document
  but cannot read the parent DOM or the token.
- **Manifest `[ui]`** declares: `entry` (the remote URL / bundle ref), `label` + `icon` (the nav slot),
  and `scope` (the MCP tools the page may call — bounded by the install's `requested ∩ admin_approved`).
- **Tokens exposed** via a small `lib/design-tokens/` export (CSS variables) the shell injects into
  both renderers, so a federated page looks native and an iframe page at least matches palette/spacing.

**Rejected alternatives:**
- *Let extension pages call `invoke` / hold the token directly.* Rejected outright — that hands the
  weakest principal the strongest credential. The page must be a gated caller through a host-mediated
  bridge (rule 5/7; the `authz-grants` "gated callers, never trusted deciders" rule). This is the whole
  point of the scope.
- *Module-federate everything (incl. untrusted).* Rejected — in-process remote code from an untrusted
  publisher is arbitrary code execution in the shell. Trusted-publisher-only for in-process; untrusted
  always sandboxes. Matches README §6.13 exactly.
- *iframe everything (incl. trusted).* Rejected — first-party pages should feel native (shared tokens,
  in-process, no postMessage latency for every interaction). Two tiers, by trust.
- *A bespoke per-extension nav wiring.* Rejected — one generic `features/ext-host/` driven by the
  manifest `[ui]` decl, like the lifecycle console is one generic surface over `ext.*`.

## How it fits the core

- **Capabilities (rule 5/7):** an extension page reaches platform functionality **only** through
  host-mediated MCP tools, re-checked per call. The page holds no token and no caps; its reachable tool
  set is its install's granted scope. The deny path is real and tested (a page calling an ungranted
  tool is denied server-side).
- **Tenancy / isolation (rule 6):** the workspace comes from the **session token the shell holds**,
  never from the page or its messages. A ws-B page can neither see nor mutate ws-A; the two-principal
  isolation test extends to extension pages.
- **Stateless extensions (rule 4):** a page holds no durable state — all truth is in SurrealDB or on
  the bus, reached through the bridge. This is what keeps hot-reload/uninstall safe for UI extensions
  too: uninstall evicts the page with nothing to clean up.
- **MCP is the universal contract (rule 7):** the bridge speaks MCP tool calls — the same contract the
  UI, agents, and other extensions use. No new RPC surface for pages.
- **Placement / symmetric nodes (rule 1):** the page is served by whichever node the browser is already
  on (hub over SSE/HTTP, Tauri-local on the workstation). Same app, two transports; no role branch.
- **Data (SurrealDB):** the manifest `[ui]` decl rides the existing `Install` record (a new
  serde-defaulted field, like `tier`/`enabled` did in lifecycle-management). No new table.
- **SDK/WIT impact:** **the manifest `[ui]` block is a manifest-schema addition** (a forever-ish
  contract — confirm before landing). The host↔page bridge is a **new audited message contract** (not
  WIT — it's a browser `postMessage` protocol), but it must be specified as carefully as a WIT boundary
  because it is a trust boundary. **Stop-and-confirm gate** before implementing either.

## Example flow

1. A first-party **`hello-ui`** extension is published (signed, allow-listed key) and installed into
   `acme` (the upload/console path, already shipped). Its manifest declares
   `[ui] entry=… label="Hello" icon=… scope=["channel.list"]`.
2. Alice (admin) opens **acme**; the shell reads the install's UI decl and shows a **Hello** nav slot
   (cap-gated). Bob, lacking the extension's grant, never sees it.
3. Alice opens **Hello**. The publisher key is allow-listed → the shell **module-federates** the remote
   in-process and calls its `mount(el, bridge, tokens)`. The page looks native (shared tokens).
4. The page asks the bridge for `channel.list`; the shell forwards through `invoke`; the host re-checks
   `mcp:channel.list:call` + the workspace and returns **acme's** channels. The page renders them.
5. The page tries `channel.delete` (not in its scope) — the shell filters it out **and** the host would
   deny it anyway: **denied**. The page shows the deny; nothing happens.
6. A **third-party** `widgets` extension (publisher key NOT allow-listed) declares a UI. Its slot opens
   into a **sandboxed iframe**; it talks to the host only via `postMessage`, gets `acme`-scoped,
   cap-checked results, and can touch neither the parent DOM nor the token.
7. Carol (ws-B admin) opens her shell: the same `hello-ui` page, but every bridged call is **ws-B**.
   The wall holds across the extension-page surface.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md`:

- **Capability deny** — an extension page calling a tool **outside its granted scope** is denied
  server-side (the bridge filter is convenience; the host is the boundary). A page never receives the
  token; assert it cannot forge a wider call.
- **Workspace isolation** — two real sessions: a ws-B page's bridged calls hit only ws-B; a ws-A id is
  denied/empty. The two-principal test, extended to the page bridge.
- **Trust-tier routing** — a trusted (allow-listed key) extension renders **in-process** (module
  federation); an untrusted one renders **sandboxed** (iframe/Web Component). A non-allow-listed key
  **cannot** load in-process even if its manifest asks to (the trust tier is the key's status, not the
  manifest's claim).

Plus this slice's cases:

- **Bridge contract** — `{tool, args}` in → `invoke` out → result back; malformed messages are
  rejected; the token never crosses the boundary (assert it is not in any postMessage payload).
- **Tokens exposed** — a federated page receives the design tokens (CSS variables present); an iframe
  page receives them on its frame document but cannot read the parent.
- **Manifest `[ui]` round-trip** — an extension declaring a UI gets a nav slot (cap-gated); one not
  declaring a UI gets none; the decl survives install/uninstall (uninstall evicts the slot).
- **Reference extension** — the `hello-ui` end-to-end: install → mount → bridged `channel.list` →
  workspace-scoped result; uninstall removes the page.
- **Vitest** — the `features/ext-host/` renderer + bridge on the fake (mirror the console suites),
  including the deny + isolation + trust-tier-routing cases; fakes match the bridge contract 1:1.

## Risks & hard problems

- **The page is the weakest principal; treating it as trusted is the catastrophic failure.** Any path
  that hands a page the token, lets it call `invoke` directly, or trusts a workspace/cap from a message
  is a tenancy/capability hole. The host-mediated bridge + per-call re-check is the entire safety story;
  it must be specified and tested like a WIT boundary. **This is the load-bearing risk.**
- **Module federation = arbitrary in-process code.** It is acceptable ONLY for an admin-trusted
  publisher key. If the allow-list check is wrong or bypassable, an untrusted publisher gets code
  execution in the shell. The trust tier must derive from the registry's verified key status, not the
  manifest.
- **iframe sandbox escapes.** `sandbox` flags, `postMessage` origin checks, and CSP must be correct or
  an "untrusted" page isn't. Pin the sandbox attributes; verify origin on every message.
- **Bridge surface creep.** Every tool the bridge exposes is attack surface. Keep it to "forward an MCP
  tool call within the extension's granted scope"; resist adding page-specific host APIs.
- **Manifest/SDK forever-cost.** The `[ui]` block and the bridge protocol are long-lived contracts.
  Get them right once (the stop-and-confirm gate); a v2 protocol is expensive.
- **Build coupling.** Module federation needs build-tooling support (Vite/webpack). Pin the approach
  before depending on it; the untrusted iframe path must work even if federation tooling is absent
  (graceful: an extension with no federated remote falls back to iframe).

## First consumer — widget-in-a-cell (the tractable narrowing)

The **dashboard** (`scope/frontend/dashboard-scope.md`, Phase 2) is this bridge's **first and narrowest
consumer**, and it is the right one to build first. A federated *widget* is far smaller than a federated
*page*: it gets **one grid cell**, a **read-only** data binding (`series.read` / `series.latest` /
`series.watch` within its bound scope), and **nothing else** — no nav slot, no token, no arbitrary tool
set. That shrinks the load-bearing trust surface (the #1 risk below) to "render the data it was bound to,
call nothing else." So the recommended build order is: prove the bridge on the **widget** case (read-only
series in a cell), then generalize to full extension **pages**. The dashboard ships its widget binding
contract **first-party** in Phase 1 (`{widget_type, binding, options}` + the three read verbs) precisely
so Phase 2 is a *renderer* swap (first-party component → federated remote / iframe) behind this bridge,
not a contract redesign. The `[ui]` manifest block gains a narrowed `[widget]` sibling (entry, label,
the read-only series scope it may bind) for this case.

## Open questions

- **Federation tooling** — **RESOLVED → real Vite Module Federation** (`@originjs/vite-plugin-federation`).
  Shipped (2026-06-27, `fleet-monitor`, see `sessions/extensions/fleet-monitor-federation-session.md`):
  the shell is the federation **host** sharing `react`/`react-dom` as singletons; an extension ships a
  `remoteEntry.js` container exposing `./mount` and the shell loads it in-process via the federation
  runtime (`features/ext-host/federation.ts`). This *upgrades* the earlier lean ("dynamic `import()` of
  a published ESM remote"): a raw `import()` bundles a second React copy → hook-dispatcher mismatch, not
  native. The `mount(el, ctx, bridge)` contract is unchanged; only the loader is now true MF.
  **Native extensions also ship pages** — the install path now projects `[ui]`/`[[widget]]` onto the
  `Install` for BOTH tiers (`crates/host/src/ui_decl.rs`), so a native extension's page surfaces in
  `ext.list` (it did not before — the load-bearing fix this slice).
- **The bridge protocol shape** — a thin `{id, tool, args}` request/`{id, ok|err, result}` reply over
  `postMessage`, vs. a richer typed channel (Comlink-style). Lean: the thin explicit protocol (auditable;
  every field is reviewable) — this is a trust boundary, not ergonomics.
- **Where the trust tier is decided** — purely the publisher-key allow-list, vs. an admin per-extension
  "allow in-process" toggle on top. Lean: allow-listed key ⇒ eligible for in-process, with an admin
  opt-in toggle as defense in depth (an admin can force any extension to sandbox).
- **Nav placement** — extension pages as top-level nav slots vs. grouped under an "Extensions" section.
  Lean: top-level cap-gated slots (discoverable), the console stays the *management* surface.
- **Token exposure mechanism** — CSS custom properties injected into both renderers vs. a JS tokens
  object passed to `mount`. Lean: both (CSS vars for styling, a tokens object for a federated page that
  wants values in JS); the iframe gets CSS vars only.
- **Manifest `[ui]` fields** — minimal (`entry`, `label`, `icon`, `scope`) vs. richer (routes, multiple
  pages, default surface). Lean: minimal one-page-per-extension v1; multi-page is a follow-up.
- **SDK/WIT gate** — the `[ui]` manifest block + the bridge protocol are forever-ish contracts; confirm
  the schema before implementing (HOW-TO-CODE.md "SDK/WIT impact: stop and confirm").

## Related

- `scope/extensions/lifecycle-management-scope.md` — install/enable/disable/uninstall + upload the page
  host sits on top of (a UI extension is installed/managed exactly like any other).
- `scope/extensions/extensions-scope.md` — the runtime + capability model the page bridge re-checks into.
- `scope/extensions/native-tier-scope.md` + `scope/registry/registry-scope.md` — the publisher-key
  allow-list (`TrustedKeys`) the trust tier reuses.
- `scope/frontend/admin-console-scope.md` — the console that **manages** extensions (this scope **hosts
  their pages**); the deferral this scope resolves.
- `scope/frontend/frontend-scope.md` + `scope/frontend/ui-design-scope.md` — the shell + the design
  tokens exposed to extension pages.
- `scope/auth-caps/authz-grants-scope.md` — the "gated callers, never trusted deciders" rule the page
  bridge inherits.
- `scope/tenancy/tenancy-scope.md` — the workspace wall the bridge holds per session.
- README **§6.13** (frontend / extension UIs — the source line), **§6.6** (identity/caps), **§7**
  (tenancy).
