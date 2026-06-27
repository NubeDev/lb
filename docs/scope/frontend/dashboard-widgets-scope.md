# Frontend scope — dashboard widgets as extensions (the federated widget contract)

Status: scope (the ask). Promotes to `public/frontend/dashboard.md` once shipped. Target stage: **S9+**,
**after** `dashboard-scope.md` Phase 1 ships (the first-party grid + binding contract). This is that
scope's **Phase 2**, promoted to its own build-ready scope because it is the security-load-bearing piece:
a widget is the first thing that crosses the **extension trust boundary**, so "how a third-party widget
gets its data" must be specified as carefully as a WIT boundary. It is the **narrowing of
`scope/extensions/ui-federation-scope.md` to the widget-in-a-cell case** — same host-mediated bridge,
but the smallest possible surface (one cell, read-only series, nothing else).

> **Superseded to v2 by [`dashboard/widget-builder-scope.md`](dashboard/widget-builder-scope.md).** This
> doc froze a `v:1` widget contract — read-only, four series verbs, no writes, no DB. The shipped page
> bridge (`proof-panel`) already forwards *writes* under the grant, so v2 generalizes the widget to "any
> view bound to any MCP tool the install grant allows (read **or** write)," leashed by `cell.tools ∩
> grant` and re-checked at the host. The token-less / workspace-from-token / iframe-untrusted invariants
> below are **unchanged**; only the forwardable tool set widens. Read v2 for the current direction.

We want a **widget to be an installable extension**: a signed artifact, installed per workspace with an
admin-approved scope, that declares a `[widget]` block and **renders inside one dashboard grid cell**,
reading the series it is bound to and **nothing else**. A trusted (first-party, allow-listed publisher)
widget runs in-process via module federation; an untrusted (third-party) widget runs in a sandboxed
iframe. **Neither ever holds the session token, calls `invoke` directly, or touches SurrealDB** — it asks
the host through a narrow, audited bridge, and the host re-checks the workspace and the capability on
every call, exactly as it does for any other caller.

---

## The headline answer: how a widget extension accesses the database

**It does not. Ever. Directly.** A widget is the *weakest principal in the system*, not a privileged one.
There is no path from widget code to a SurrealDB handle, a store connection, or the session token. Data
reaches a widget through one chain, and that chain re-checks authorization at the host on every hop:

```
  widget code (in-process remote  OR  sandboxed iframe)
     │  postMessage { id, tool: "series.read", args: { series, range } }   ← NO token, NO db handle
     ▼
  WidgetBridge  (runs in the trusted shell — this is the security seam)
     │  1. is `tool` in THIS widget's granted read-only scope?  (series.read|latest|find|watch)  ──no──► reject locally
     │  2. does `args.series` fall within the widget's bound series scope?                        ──no──► reject locally
     │  3. forward to the ONE thing that holds the token:
     ▼
  invoke(tool, args)  ──►  gateway route / Tauri command   (the shell holds the token; the widget never does)
     │
     ▼
  HOST  — the same three gates every caller runs:
     │  Gate 1  workspace  ← from the SESSION TOKEN, never from the widget's message   (the hard wall, rule 6)
     │  Gate 2  capability ← `mcp:series.read:call` ∩ the install's admin-approved scope (rule 5/7)
     │  Gate 3  (n/a for series reads; present for membership-scoped resources)
     ▼
  SurrealDB read, in the workspace namespace  ──►  result
     │  postMessage { id, ok: true, result }  back to the widget
     ▼
  widget renders
```

Four invariants make this safe, and each is a test (below):

1. **No token at the widget.** The token lives only in the shell. A widget posts a `{tool, args}` request;
   the shell calls `invoke`. Assert the token appears in **no** postMessage payload, in or out.
2. **Workspace comes from the token, not the message.** The host derives the workspace from the verified
   session token (§7). A widget cannot name a foreign workspace — there is no field for it to set. A ws-B
   widget's reads hit only ws-B.
3. **The reachable tool set is read-only and bounded.** The bridge forwards **only** the series read verbs
   (`series.read` / `series.latest` / `series.find` / `series.watch`) that are in this widget's
   install-granted scope (`requested ∩ admin_approved`, the shipped S4 intersection). A widget that posts
   `series.write`, `dashboard.delete`, or anything else is rejected at the bridge **and** would be denied
   at the host regardless (defense in depth — the bridge filter is convenience, the host is the boundary).
4. **The binding is the leash.** A widget can only read series within its **bound scope** (the cell's
   `binding` + the install's series-prefix/tag constraint). It cannot read an arbitrary series id even via
   a granted `series.read` — the host re-checks the series against the grant's optional prefix narrowing
   (`mcp:series.read:call?series=cooler.*`), and the bridge pre-filters.

That is the whole DB-access story: **host-mediated, read-only, capability- and workspace-re-checked,
token-less.** It is identical in shape to how the UI, agents, and other extensions reach the store — MCP
is the universal contract (rule 7) — just narrowed to four read verbs in one cell.

---

## Goals

- **A widget is an installed extension artifact.** Reuses the **shipped** registry + lifecycle path
  (`scope/extensions/lifecycle-management-scope.md`, `registry-scope.md`): signed, verified before cache,
  installed **per workspace** with `requested ∩ admin_approved` scope. No new install mechanism.
- **A `[widget]` manifest declaration** — the extension states it provides a widget: `entry` (the ESM
  remote ref for trusted / the iframe entry URL for untrusted), `label` + `icon` (for the palette), and
  `scope` (the **read-only** series verbs it may call, bounded by the install grant; optionally a series
  prefix or required tags it may bind). The host reads it on install; the dashboard reads it to build the
  palette. No `[widget]` → the extension provides no widget (lifecycle/console unchanged).
- **Two trust tiers, one bridge** (README §6.13, inherited from `ui-federation-scope.md`):
  - **Trusted (admin-allow-listed publisher key — `lb-registry::TrustedKeys`) → module federation.** The
    widget ships an ESM remote exposing a `mount(el, bridge, tokens)` contract; the cell host loads it
    lazily and renders it **in-process**. Fast, shares design tokens, looks native.
  - **Untrusted (any other key) → sandboxed iframe.** `sandbox="allow-scripts"` (no same-origin, no top
    navigation); the **only** channel is `postMessage`. It gets design tokens as CSS variables but cannot
    read the parent DOM or the token. **The trust tier is the key's allow-list status, not the manifest's
    say-so** — a non-allow-listed widget can never load in-process even if its manifest asks to.
- **A read-only host-mediated bridge** (`WidgetBridge`) — the security seam above: forward a series read
  call within the widget's granted scope; host re-checks ws + cap per call. Nothing else is forwardable.
- **A widget palette in the dashboard editor** (cap-gated) — installed widget extensions appear as
  draggable tiles; dragging one creates a cell with `widget_type: "ext:<extId>"`, then the user binds it
  to a series (the same `{series} | {find:{tags}}` binding from Phase 1, constrained to the widget's
  declared scope).
- **The cell host renders by tier** — `features/dashboard/WidgetHost` learns to mount a cell **first-party**
  (the Phase-1 built-ins) **or federated** (`ext:<id>` → trusted in-process / untrusted iframe), through
  the bridge. One contract, two renderers.
- **A reference federated widget** — port the Phase-1 `chart` widget to a federated extension
  (`chart-widget`), proving the path end to end: install → declare `[widget]` → appears in palette → drag
  → bind → bridged `series.read`/`series.watch` → workspace-scoped, cap-checked render → uninstall evicts
  it with nothing to clean up (stateless).

## Non-goals

- **No widget writes, ever.** A widget reads series; it cannot mutate anything. There is no write verb in
  the bridge's forwardable set. An "action" surface (a button that does something) is a **full extension
  page** via `ui-federation-scope.md`, not a cell widget — a different, larger trust conversation.
- **No token, no `invoke`, no DOM across the boundary, no store handle** at the widget. (The #1 risk;
  see the invariants above.)
- **No arbitrary tools.** Only the read-only series verbs in the granted scope. The bridge is not a
  general RPC channel; resist adding widget-specific host APIs (surface creep is attack surface).
- **No in-process untrusted code.** Module federation is trusted-publisher-only; untrusted always
  sandboxes. Non-negotiable (it is arbitrary code execution in the shell otherwise).
- **No nav page / multi-cell widget / cross-node page routing.** A widget is one cell on the node the
  browser is already talking to. Pages and routing are `ui-federation-scope.md`.
- **No new install/registry/datastore.** Reuses the shipped registry + lifecycle + `Install` record (a
  serde-defaulted `[widget]` field on it, exactly as `tier`/`enabled` were added in lifecycle-management).
- **No `*.fake.ts`.** Tests drive a real in-process gateway + a real installed reference widget.

## Intent / approach

**Swap the renderer, keep the contract.** Phase 1 proved the binding contract (`{widget_type, binding,
options}` + four read verbs) with first-party React components. Phase 2 keeps that contract byte-for-byte
and changes only **who renders the cell** and **how its reads are mediated**:

- `widget_type: "chart"` → a built-in component, reads series via the shell's normal `http.ts` (Phase 1).
- `widget_type: "ext:chart-widget"` → a federated renderer (in-process or iframe by trust), reads series
  via the **`WidgetBridge`** (host-mediated). Same binding, same four verbs, same data on screen.

Because the contract didn't change, a dashboard built in Phase 1 keeps working, and the only new trust
surface is "a remote renders the cell and asks the bridge for its bound series." That surface is minimal
by construction — read-only, one cell, scoped — which is the entire reason widgets are the right first
consumer of federation (vs. arbitrary pages).

**The bridge is specified like a WIT boundary.** A thin, explicit, auditable protocol — every field
reviewable, because it is a trust boundary, not ergonomics:

```
  request  (widget → shell):  { id: string, tool: SeriesReadVerb, args: object }
  reply    (shell → widget):  { id: string, ok: true, result: any } | { id, ok: false, error: string }
  SeriesReadVerb = "series.read" | "series.latest" | "series.find" | "series.watch"
```

`series.watch` returns a subscription the shell backs with the **Phase-1 series SSE stream**; the shell
relays `sample` events to the widget as `{ id, event: "sample", sample }` messages (still no token, still
ws-scoped at the host). The shell tears the subscription down when the cell unmounts or the widget is
uninstalled (stateless eviction).

**Rejected alternatives** (inherited from `ui-federation-scope.md`, applied to widgets):

- *Let the widget call `invoke` / hold the token.* Rejected outright — hands the weakest principal the
  strongest credential. The widget is a gated caller through the bridge, full stop.
- *Module-federate untrusted widgets too.* Rejected — arbitrary in-process code from an untrusted
  publisher. Trusted-key-only for in-process; untrusted always iframes.
- *iframe trusted widgets too.* Rejected — first-party widgets should feel native (shared tokens,
  in-process, no per-frame latency on every sample). Two tiers, by trust.
- *A general tool bridge (any MCP tool).* Rejected for widgets — a widget needs four read verbs, not the
  tool namespace. The narrow bridge is the safety. (The general page bridge is `ui-federation-scope.md`.)
- *Give widgets write access for "interactive" widgets.* Rejected — a writing widget is an action surface;
  that is a page extension with its own grant conversation, not a cell.

## How it fits the core

- **Capabilities (rule 5/7):** a widget reaches data **only** through host-mediated MCP, re-checked per
  call. Its reachable set is `{series.read, series.latest, series.find, series.watch} ∩ install-granted
  scope`, optionally narrowed by a series-prefix grant. The deny path is real and tested (a widget calling
  outside its scope is denied **server-side**, not just bridge-filtered).
- **Tenancy / isolation (rule 6):** the workspace is the session token's, held by the shell, never the
  widget's. A ws-B widget can neither read nor enumerate ws-A series; the two-session isolation test
  extends to the widget bridge.
- **Stateless extensions (rule 4):** a widget holds no durable state — all truth is in SurrealDB or on the
  bus, reached through the bridge. Uninstall evicts the cell renderer + tears down its SSE subscription
  with nothing to clean up. This is what makes hot-reload/uninstall safe for widgets.
- **MCP is the universal contract (rule 7):** the bridge speaks MCP tool calls — the same contract the UI,
  agents, and other extensions use. No new RPC surface; just a narrowed, audited forwarder.
- **Placement / symmetric nodes (rule 1):** the widget is served by whichever node the browser is on (hub
  over SSE/HTTP, Tauri-local on the workstation). Same app, two transports; no role branch.
- **Data (SurrealDB):** the `[widget]` declaration rides the existing `Install` record (a new
  serde-defaulted field). No new table. The series it reads are the shipped S8 tables.
- **SDK/WIT impact — RESOLVED (both contracts frozen below).** The **`[widget]` manifest block** and the
  **bridge protocol** are long-lived contracts (a v2 is expensive), so they are specified as **final** in
  "The two forever-contracts" section below — the stop-and-confirm gate is *discharged in this scope*, not
  deferred to the coding session. Both carry an explicit version field so a future v2 is additive, never a
  breaking reinterpretation. The bridge is a browser `postMessage` protocol (not WIT), but it is a trust
  boundary and is frozen with the same care as a WIT boundary.

## Access & authorization — who sees and uses a widget

A widget surfaces through the **same three gates** as everything else (README §6.6; the S4 model):

1. **Gate 1 — workspace.** The widget extension is installed **per workspace**. A ws-B admin installing
   `chart-widget` does not make it appear in ws-A. The install record is workspace-namespaced.
2. **Gate 2 — capability.** A user sees the **widget palette** only if they hold the dashboard-edit cap
   (`mcp:dashboard.save:call` — you must be able to edit a dashboard to add a widget). A user sees a
   widget *render* on a dashboard they can view if they hold the series read cap the widget needs; lacking
   it, the cell renders an honest **denied** state (not a blank, not a fake value).
3. **Gate 3 — the install grant.** The widget itself can only call the verbs in `requested ∩
   admin_approved` for its install — an admin who approved only `series.read`/`series.latest` for
   `chart-widget` means the widget cannot call `series.find` even if its manifest requested it.

So: **install** is an admin act (Gate 1 + the install cap), **adding** a widget to a dashboard needs the
dashboard-edit cap, **rendering** needs the viewer's series read cap, and **what the widget can fetch** is
its install grant. Four independent checks, none bypassable by the widget. This composes with the
dashboard's own sharing model (`dashboard-scope.md` → Access & authorization): a viewer of a shared
dashboard sees its widgets render only for series they're granted.

## The two forever-contracts (RESOLVED — frozen, code against these)

Both contracts are **final**. They carry a version field so future growth is additive. Nothing below is
an open question; the coding session implements these byte-for-byte.

### Contract 1 — the `[widget]` manifest block

Rides the existing extension manifest (TOML, like `[native]`/`[ui]`); a serde-defaulted block, so an
extension without it provides no widget.

> **Refinement (2026-06-27, shipped):** an extension may declare **several** widgets, so the block is a
> TOML **array-of-tables `[[widget]]`** (`widgets: Vec<Widget>` end to end — manifest → `Install` →
> `ExtRow` → `ext.api`), serde-defaulted to empty. The v1 **fields below are unchanged**; only the
> cardinality (one → many) changed. This is strictly additive (empty default = the old single-widget
> behavior). See `sessions/extensions/fleet-monitor-federation-session.md`.

**Frozen v1 fields** (per `[[widget]]` tile):

```toml
[widget]
v          = 1                                   # block version (forever-evolution; required)
entry      = "./dist/chart-widget.mjs"           # trusted: ESM remote exposing mount(); untrusted: iframe entry URL
label      = "Temperature Chart"                 # palette label
icon       = "line-chart"                         # lucide icon name (palette)
scope      = ["series.read", "series.latest", "series.watch"]   # read-only series verbs it may call (subset of the 4)
bind       = { kind = "prefix", value = "cooler.*" }            # OPTIONAL bind leash — "prefix" | "tags"
# bind     = { kind = "tags", value = ["kind:temperature"] }   #   (the alternative form)
```

- **`scope`** must be a subset of `{series.read, series.latest, series.find, series.watch}` — the host
  **rejects an install** whose `[widget].scope` names any other tool (a write verb, a non-series tool).
  The effective scope at runtime is `scope ∩ admin_approved` (the shipped S4 intersection).
- **`bind`** is the optional leash: a widget may only bind/read series matching it. Omitted ⇒ the widget
  may bind any series the *viewer* is granted (no extra narrowing beyond the cap). `prefix` maps to the
  ingest grant narrowing (`mcp:series.read:call?series=cooler.*`); `tags` constrains `series.find`.
- **The trust tier is NOT in the manifest** — it is the publisher key's allow-list status
  (`lb-registry::TrustedKeys`). A manifest cannot ask to be trusted.
- **Not in v1 (additive later under a higher `v`):** multi-cell widgets, configurable options schema,
  write/action verbs (those are page extensions, not widgets), default sizing hints.

### Contract 2 — the widget bridge protocol

**One logical surface, two transports** (mirroring "one app, two deliveries"). A widget always sees the
same `bridge` API; the shell implements it in-process for trusted widgets and over `postMessage` for
sandboxed ones. The widget **never** sees the token in either transport.

**The `mount` entrypoint (both tiers).** The shell calls (trusted: directly; untrusted: the iframe's
bootstrap calls its local copy after the init handshake):

```
mount(el, bridge, ctx)
  el     : the host element / iframe body the widget renders into
  bridge : { call(req) → Promise<Reply>,  watch(req, onEvent) → unsubscribe() }
  ctx    : { binding, options, tokens }   # the cell's binding + widget options + design tokens (CSS-var map)
                                          #   NEVER the session token, NEVER the workspace id
```

**The wire messages (the iframe transport; the in-process `bridge` carries the same shapes as JS calls):**

```jsonc
// request  (widget → shell)
{ "v": 1, "id": "<uuid>", "tool": "series.read", "args": { "series": "cooler.temp", "from_seq": 0, "to_seq": null } }
// tool ∈ "series.read" | "series.latest" | "series.find" | "series.watch" | "series.unwatch"
// args = the existing host verb's args, verbatim (read: series+from/to_seq; latest/watch: series; find: tags)

// reply    (shell → widget) — one per request id
{ "v": 1, "id": "<uuid>", "ok": true,  "result": <verb result> }
{ "v": 1, "id": "<uuid>", "ok": false, "error": "denied" }   // error ∈ denied | out_of_scope | bad_request | unavailable

// watch stream (shell → widget) — after a series.watch, keyed by the watch's id
{ "v": 1, "id": "<watch-id>", "event": "sample", "sample": { /* Sample */ } }
{ "v": 1, "id": "<watch-id>", "event": "closed" }            // on teardown / uninstall / unmount

// init handshake (shell → iframe, once on load — iframe transport only)
{ "v": 1, "event": "init", "binding": { /* … */ }, "options": { /* … */ }, "tokens": { "--lb-bg": "#…", /* … */ } }
```

**Frozen rules:**
- `series.unwatch` and `series.watch` are **bridge controls** the shell satisfies with the Phase-1 series
  SSE subscription — they are not new host MCP verbs. The shell also force-`closed`s every watch on cell
  unmount / widget uninstall (stateless eviction; no leaked stream).
- **The shell verifies `event.origin`** on every inbound iframe message and posts only to the iframe's
  expected origin. The iframe is `sandbox="allow-scripts"` (no `allow-same-origin`, no top navigation).
- **The shell is the filter; the host is the boundary.** The shell drops any `tool` not in the widget's
  effective scope (→ `out_of_scope`) and any `args.series` outside the `bind` leash; the host re-checks
  cap + workspace + the series narrowing regardless. A bypassed shell filter still hits a host deny.
- **No field carries the token or the workspace id** — ever, in any message, either direction. (Tested.)
- **Versioning:** every message and the manifest block carry `v`. A receiver rejects an unknown major `v`
  rather than guessing. v1 is what this scope freezes.

## Example flow

1. A first-party **`chart-widget`** is published (signed, allow-listed key) and an admin installs it into
   `kfc` (the shipped upload/console path), approving scope `["series.read","series.watch"]`.
2. Alice (holds `mcp:dashboard.save`) opens a `kfc` dashboard in edit mode; the **widget palette** shows
   `chart-widget` (Gate 1 install + Gate 2 edit cap). She drags it onto the grid → a cell
   `{widget_type:"ext:chart-widget", binding:{series:"cooler.temp"}}`.
3. The publisher key is allow-listed → the cell host **module-federates** the remote in-process and calls
   `mount(el, bridge, tokens)`. It looks native (shared tokens).
4. The widget posts `{id, tool:"series.read", args:{series:"cooler.temp", range:"1h"}}`. The bridge
   confirms `series.read` ∈ its scope and `cooler.temp` ∈ its bound scope, calls `invoke`; the host
   re-checks `mcp:series.read:call` + the `kfc` workspace (from Alice's token) and returns kfc's data. The
   chart backfills, then `series.watch` streams live samples — all token-less, ws-scoped.
5. The widget posts `{tool:"dashboard.delete", …}` (not in its scope). The bridge rejects it; the host
   would deny it anyway. **Denied.** Nothing happens.
6. A **third-party** `fancy-gauge` (publisher key NOT allow-listed) is installed. Its cell opens into a
   **sandboxed iframe**; it talks to the host only via `postMessage`, gets kfc-scoped cap-checked series
   data, and can touch neither the parent DOM nor the token.
7. Carol (a `mcdonalds` session) has `chart-widget` installed in her workspace; every bridged call is
   **mcdonalds**. A `kfc` series id from her widget is denied/empty. The wall holds across the widget
   bridge.
8. An admin uninstalls `chart-widget` → its cells stop rendering, the bridge tears down their SSE
   subscriptions, the dashboard record still lists the cells (they render an "extension not installed"
   state) — stateless eviction, no orphaned state.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`) — real infra, a **real installed reference
widget**, no mock backend:

- **Capability deny** — a widget calling a tool **outside its granted scope** is denied **server-side**
  (assert the host denies even if the bridge filter were bypassed); a widget calling a series read without
  the viewer holding the cap is denied. **Per-verb** deny across the four read verbs.
- **Workspace isolation** — two real sessions: a ws-B widget's bridged calls hit only ws-B; a ws-A series
  id is denied/empty. The two-principal test, extended to the widget bridge.
- **Trust-tier routing** — an allow-listed-key widget renders **in-process** (module federation); a
  non-allow-listed one renders **sandboxed** (iframe); a non-allow-listed key **cannot** load in-process
  even if its manifest asks (the tier is the key's status, not the manifest's claim).

Plus this slice's cases:

- **Token never crosses the boundary** — assert the session token is in **no** postMessage payload (request
  or reply); a widget cannot forge a wider call (it has no token to widen with).
- **Bridge contract** — `{id, tool, args}` in → `invoke` out → `{id, ok|err, result}` back; malformed
  messages rejected; an unknown/non-read tool rejected; the `series.watch` subscription tears down on
  unmount/uninstall (no leaked stream).
- **Binding leash** — a widget bound to `cooler.*` cannot read `fryer.state` even with `series.read`
  granted (the host's series-prefix narrowing + the bridge pre-filter).
- **Manifest `[widget]` round-trip** — an extension declaring a widget appears in the palette (cap-gated);
  one not declaring it does not; the decl survives install/uninstall (uninstall evicts the renderer).
- **Reference widget e2e** — `chart-widget`: install → palette → drag → bind → bridged `series.read` +
  `series.watch` → ws-scoped render → uninstall evicts cleanly.
- **Vitest** — the `WidgetHost` renderer (both tiers) + the `WidgetBridge` on the real in-process gateway,
  including deny + isolation + trust-tier-routing + token-never-crosses.

## Risks & hard problems

- **The widget is the weakest principal; treating it as trusted is the catastrophic failure.** Any path
  that hands it the token, lets it `invoke` directly, or trusts a workspace/series from a message is a
  tenancy/capability hole. The host-mediated bridge + per-call re-check is the entire safety story; it is
  specified and tested like a WIT boundary. **Load-bearing.**
- **Module federation = arbitrary in-process code.** Acceptable ONLY for an admin-trusted (allow-listed)
  publisher key. If the allow-list check is wrong or bypassable, an untrusted publisher gets code
  execution in the shell. The tier derives from the registry's verified key status, never the manifest.
- **iframe sandbox escapes.** `sandbox` flags, `postMessage` **origin** checks, and CSP must be correct or
  an "untrusted" widget isn't sandboxed. Pin the attributes; verify origin on every message.
- **Bridge surface creep.** Every forwardable verb is attack surface. Keep it to the four read verbs;
  adding "just one write" turns a leash into a hole.
- **Binding-scope enforcement is at the host, not the widget.** The bridge pre-filter is convenience; the
  host's series-prefix narrowing on the grant is the real leash. Don't ship the prefix grant as a TODO.
- **Manifest/protocol forever-cost.** `[widget]` + the bridge protocol are long-lived. Get them right once
  (the stop-and-confirm gate); a v2 is expensive.
- **Federation tooling.** Module federation needs build support (lean: dynamic `import()` of a published
  ESM remote with the `mount(el, bridge, tokens)` contract — least build magic, works with Vite). The
  untrusted iframe path must work even if federation tooling is absent (graceful fallback to iframe).

## Open questions

**None blocking — the two forever-contracts are frozen above; this scope codes with no open question.**
The decisions, recorded so they aren't re-litigated:

- **`[widget]` manifest + bridge protocol** — FROZEN (see "The two forever-contracts"). The stop-and-confirm
  gate is discharged here, not deferred.
- **Federation mechanism** — DECIDED: dynamic `import()` of a published ESM remote exposing
  `mount(el, bridge, ctx)` (least build magic, works with Vite). The untrusted iframe path works even if
  federation tooling is absent (graceful fallback).
- **Where the tier is decided** — DECIDED: the publisher-key allow-list makes a widget *eligible* for
  in-process; an admin per-widget "force sandbox" toggle is the defense-in-depth override. The tier is
  never the manifest's claim.
- **Token exposure** — DECIDED: design tokens as CSS variables to both tiers + the `tokens` map in `ctx`
  for trusted widgets that want values in JS; the iframe gets CSS vars only and cannot read the parent.
- **Invariants (not open, restated):** widgets are **read-only**; **never** hold the token / `invoke` / a
  store handle / the workspace id; the reachable set is the four series read verbs ∩ the install grant;
  the binding is leashed by the `bind` narrowing re-checked at the host.

The only thing the coding session must still do is the normal HOW-TO-CODE work (build it, test it, the
mandatory categories) — there is no design decision left to make.

## Related

- `scope/frontend/dashboard-scope.md` — Phase 1 (the grid + binding contract this swaps the renderer
  behind) and its Access & authorization model this composes with.
- `scope/extensions/ui-federation-scope.md` — the general page bridge this **narrows to a widget**; shares
  the trust tiers, the `mount` contract, the token-less bridge principle, and the `TrustedKeys` allow-list.
- `scope/extensions/lifecycle-management-scope.md` + `scope/registry/registry-scope.md` — the shipped
  install/verify/per-workspace-grant path a widget extension rides; the publisher-key allow-list the tier
  reuses.
- `scope/ingest/ingest-scope.md` — the `series.read`/`series.latest`/`series.find` verbs (and the
  `series.watch` SSE the dashboard scope adds) the bridge forwards, with the series-prefix grant narrowing.
- `scope/auth-caps/authz-grants-scope.md` — the "gated callers, never trusted deciders" rule the widget
  bridge inherits; the `requested ∩ admin_approved` install intersection.
- `scope/tenancy/tenancy-scope.md` — the workspace wall the bridge holds per session.
- README **§6.13** (extension UIs — module federation vs iframe by trust), **§6.6** (identity/caps/3 gates),
  **§7** (tenancy).
