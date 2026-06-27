# Extensions scope — proof-panel (the basics, proven end-to-end by one Tier-1 extension)

Status: scope (the ask). Promotes to `public/extensions/` once shipped. Target stage: **S10 (extension
UX)**. Sibling of `ui-federation-scope.md` (the page-host + bridge), `lifecycle-management-scope.md`
(install/enable/disable), and `native-tier-scope.md` (the Tier-2 reference, `fleet-monitor`). Where
`fleet-monitor` is the **native** reference (its own PID, stdio-supervised), `proof-panel` is the
**WASM (Tier-1)** reference — the lighter, in-process tier the runtime (`lb-runtime`, wasmtime
Component Model) already supports but no shipped extension yet exercises *with a UI*.

One paragraph: the platform's basics — the MCP contract, the capability gates, publish-then-install,
the WASM runtime, UI federation, and the host-mediated read-only bridge — are each built and tested in
isolation, but **no single artifact proves them composed end-to-end on the Tier-1 path**. `proof-panel`
is that artifact: one self-contained first-party extension that ships (a) a real WASM-served MCP tool
and (b) a federated page that reads **real workspace series data** through the bridge — so installing it
and opening its page exercises install → grant-intersection → mount → cap-checked call → workspace-scoped
result in one motion. It is a *proof*, not a product: its value is that it goes green against a real
node, not that it does fleet analytics.

## Goals

- **One Tier-1 WASM extension, both halves.** A `[runtime] tier = "wasm"` extension that ships **both** a
  capability (an MCP tool served from the wasm guest) **and** a federated UI page — co-located in one
  `rust/extensions/proof-panel/` folder, the same one-folder shape as `fleet-monitor` but on the
  in-process tier.
- **A real MCP tool from the wasm guest.** One tool, `proof.ping` — stateless, returns a workspace-tagged
  JSON snapshot (the wasm analogue of `fleet.summary`). Proves a Tier-1 guest serves a reachable,
  cap-gated MCP tool through `lb-runtime`'s `tool.call` WIT export, with the host enforcing the gate
  before dispatch.
- **A federated page that reads real data through the bridge.** The page lists the workspace's series
  (`series.find`) and shows the latest value of a selected one (`series.latest`) — **only** through the
  host-mediated bridge (`bridge.call`), never a token, DB handle, or raw `fetch`. Empty workspace → an
  honest empty state, never fabricated rows (the `fleet-monitor` widget-placeholder gap is the
  anti-pattern this scope closes).
- **Grant intersection visibly enforced.** The manifest *requests* `series.find|latest|read`; the page's
  effective scope is `requested ∩ admin_approved` (the S4 intersection). A test installs it with a
  **narrower** approval and proves the page is denied the un-approved verb at the bridge — the gate is
  real, not cosmetic.
- **Proven against a real node, no placeholders.** Unlike `fleet-monitor`'s two placeholder widgets,
  every surface `proof-panel` ships is wired to a real verb and covered by a test that drives the real
  path. "Done" means `cargo test` (the wasm tool + host install/deny) and `pnpm test:gateway` (the page
  over a real gateway) are green.

## Non-goals

- **No new host verbs, no SDK/WIT change.** `proof-panel` consumes only **existing** series verbs
  (`series.find`/`series.latest`/`series.read`) and the **existing** `tool.call` WIT world. If building
  it needs a new core verb or a WIT bump, that is a finding to surface — the scope's premise is that the
  basics are *already* sufficient to build a real extension.
- **No native tier.** That is `fleet-monitor`/`native-tier-scope.md`. `proof-panel` is Tier-1 WASM on
  purpose — to exercise the *other* runtime path.
- **No write path.** The bridge is read-only by frozen contract (`ui-federation-scope.md`). `proof.ping`
  writes nothing durable (stateless guest, rule 4). No `series.write` from the page.
- **No analytics / product surface.** No charts beyond a latest-value readout, no dashboards (that is
  `dashboard-scope.md`). The page is the thinnest real reader that still proves the path.
- **No untrusted/iframe tier.** `proof-panel` is a **trusted first-party** federated remote (the
  in-process path). The sandbox tier is covered by `ui-federation-scope.md`.

## Intent / approach

**Mirror `fleet-monitor`'s one-folder shape, drop to Tier-1, and remove every placeholder.** The folder
ships a wasm guest (`src/lib.rs` serving `proof.ping` via the generated WIT bindings — no stdio loop,
no PID, the host instantiates it in-process) and a co-located federated `ui/` (the same `mount(el, ctx,
bridge)` handshake, the same frozen `Bridge`/`MountCtx` contract). The page reaches data only through
`bridge.call("series.find"|"series.latest", …)`; the host re-checks the cap + workspace on every call.

Why this over the alternatives:
- **Why not just wire `fleet-monitor`'s widgets?** That completes one native extension but never
  exercises the **WASM-with-UI** path, which is the gap. `proof-panel` proves the tier nothing else
  proves, and it does so as a clean reference others can copy (a "blessed starting point" the
  `ui-federation` scope's hypothetical `hello-ui` only sketched).
- **Why a tool *and* a page (not just a page)?** Shipping both in one folder is the whole "an extension
  is one thing; its backend and frontend are optional parts" claim (`extension.toml` preamble). Proving
  it on Tier-1 means a wasm guest's tool and its federated page install and run as a single unit.
- **Why series verbs specifically?** They already exist, they return real workspace-scoped data, and the
  bridge contract is *defined* in terms of them — so `proof-panel` reuses the exact seam
  `ui-federation` froze, with zero new surface.

## How it fits the core

- **Tenancy / isolation:** The page receives `ctx.workspace`; every `bridge.call` is re-scoped to the
  caller's workspace by the host (the page cannot name another workspace). `series.find` returns only the
  caller's series. A two-workspace test proves ws-B's page sees none of ws-A's series.
- **Capabilities:** The wasm tool dispatch is gated by `mcp:proof.ping:call` (host-side, before dispatch
  — the grant lives on the caller per the manifest preamble convention). The page's reads are gated by
  the existing `mcp:series.find:call` / `mcp:series.latest:call`. Effective UI scope = `requested ∩
  admin_approved`. **Deny path:** install with an approval missing `series.latest` → the page's
  latest-value call returns a host-side denial (403 at the bridge), surfaced as an honest error, not a
  blank. A separate deny-test: call `proof.ping` without the grant → denied, nothing leaked.
- **Placement:** **either** — a Tier-1 wasm component runs in-process on whatever node hosts it
  (workstation Tauri-local or cloud hub); no `if cloud`. Symmetric by construction.
- **MCP surface:** **Consumes** existing reads — `series.find` (list), `series.latest` (get). **Exposes**
  one tool — `proof.ping` (a parameterless get returning a workspace-tagged snapshot). API shape: read
  verbs only (get + list). **No CRUD** (read-only reference, stated per §6.1). **No live feed** this
  slice (a `series.watch` upgrade is an open question, not a goal). **No batch.**
- **Data (SurrealDB):** Touches **no new tables.** Reads the existing `series` data via the host verbs;
  the wasm guest holds nothing durable (rule 4 — stateless, hot-reload safe). State stays in SurrealDB;
  the extension is pure motion-of-reads.
- **Bus (Zenoh):** N/A this slice — all reads are request/response through the host bridge, not a
  stream. (A future `watch` upgrade would use the bus; see open questions.)
- **Sync / authority:** N/A — read-only, node-local reads of already-synced series. No new authority.
- **Secrets:** N/A — the page never holds the token (the whole point of the bridge); the wasm guest
  never sees a credential.

## Example flow

1. A workspace admin uploads the signed `proof-panel` artifact; `ext.publish` verifies the publisher key,
   caches the bytes, and (publish-then-install) installs + loads the wasm component in-process.
2. Install computes the effective grant: manifest `request = [series.find, series.latest, series.read]`
   ∩ admin approval. Say the admin approves `series.find` + `series.latest` only.
3. The shell reads `ext.list`, sees the `[ui]` block, and adds a cap-gated "Proof Panel" nav slot.
4. The user opens it. The shell loads the federated remote and calls `mount(el, {workspace}, bridge)`.
5. The page calls `bridge.call("series.find", {})`. The host re-checks `mcp:series.find:call` + the
   workspace, runs the query, returns the caller's series list. The page renders them (or an honest
   empty state).
6. The user selects a series; the page calls `bridge.call("series.latest", {series})`. Host re-checks,
   returns the latest sample. The page shows the value.
7. Separately, an agent (or the test) calls the `proof.ping` MCP tool with the grant → the wasm guest
   returns `{"ok":true,"ws":"<workspace>","node":"proof-panel","tier":"wasm"}`, proving the backend half
   is a reachable Tier-1 tool. Without the grant, the call is denied and nothing is leaked.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md` that apply:

- **Capability deny-tests (required):** (a) `proof.ping` without `mcp:proof.ping:call` → denied, opaque.
  (b) The page's `series.latest` call when the install approval omitted it → bridge denies (the
  `requested ∩ admin_approved` narrowing is enforced at call time, not just displayed).
- **Workspace-isolation (required):** Two real workspaces; ws-B's page (and `proof.ping`) sees none of
  ws-A's series. Driven through the real gateway.
- **No-mocks (CLAUDE §9 / testing §0):** The wasm tool test instantiates the **real** component through
  `lb-runtime`; the UI test runs against a **real spawned gateway** (`pnpm test:gateway`), seeding real
  series via the real ingest/write path. No `*.fake.ts`, no fabricated rows.
- **Hot-reload (applies, Tier-1):** Re-install/replace the wasm instance and prove the page's next call
  still resolves (the guest is stateless, so swap loses nothing — rule 4). At least asserted via the
  existing publish-then-install + `load_enabled` restart path used by `ext_publish_test.rs`.

Key cases: wasm `proof.ping` unit tests (ok / unknown-tool-is-error / bad-params-is-error, mirroring
`fleet-monitor/src/call.rs`); a host install+grant-intersection test (mirroring `ext_ui_test.rs`); a
real-gateway UI test (`ProofPanel.gateway.test.tsx`: empty-state → seed a series → find lists it →
latest shows it → ungranted verb denied).

## Risks & hard problems

- **The "proves nothing new" trap.** If `proof-panel` only re-treads what `fleet-monitor` + `hello`
  already cover, it is dead weight. The mitigation is the **Tier-1-wasm-with-a-UI** combination and the
  **grant-intersection deny-test** — neither is currently proven by a single shipped artifact. Keep the
  scope honest: if every assertion already has a green test elsewhere, cut it.
- **Wasm guest serving a tool whose name has a dot.** `proof.ping` must round-trip through the
  qualify/unqualify split (`<ext>.<tool>`) the MCP dispatch does. Verify the guest sees the unqualified
  `ping` (or `proof.ping`) consistently with how `hello`'s `echo` is dispatched — a naming mismatch is a
  silent NotFound.
- **Federation build reproducibility.** The `ui/` remote must build to `assets/remoteEntry.js` with the
  shared-React federation config exactly as `fleet-monitor`'s does, or the shell mount fails at runtime
  with an opaque error. Copy the proven `vite.config.ts`/`build.sh`, don't re-derive.
- **Placeholder regression.** The entire point is *no placeholders*. A surface that renders "wires to X
  next" is a scope failure here, by definition.

## Open questions — RESOLVED (shipped 2026-06-27, see `../../sessions/extensions/proof-panel-session.md`)

- **Live feed upgrade?** → **Stayed request/response.** No `series.watch` this slice; tracked as a
  follow-up for when the verb ships.
- **Does `proof.ping` need a host-side cap?** → **No.** It follows the hello convention: the manifest
  requests no host-side cap for its own tool; `mcp:proof-panel.proof.ping:call` lives on the caller's
  token (verified against `ext_publish_test.rs`; asserted by the deny test).
- **Retire `hello`/`hello-v2`?** → **Kept as-is.** They remain the absolute-minimum spine probe;
  `proof-panel` is the full-stack Tier-1 reference beside them, not a replacement.
- **Widget tiles?** → **Deferred to the dashboard scope.** No `[[widget]]` shipped; the one page is
  enough proof here.

### Findings surfaced while building (premise check)

The scope's premise — "the basics are *already* sufficient to build a real extension, no new core verb
or WIT bump" — held for the **contract** but exposed one **plumbing** gap and one **adjacent** gap, both
fixed/worked-around without a new verb or WIT change:

- **The bridge couldn't dispatch host-native `series.*`.** `POST /mcp/call` → `call_tool` resolved only
  the runtime registry, so the series verbs the bridge contract is defined in terms of `NotFound`-ed.
  Fixed in `crates/host/src/tool_call.rs` (authorize then delegate to the existing `call_ingest_tool`).
  No new verb, no WIT change. → `../../debugging/extensions/bridge-cannot-dispatch-host-native-series.md`.
- **`series.find` discovery needs tag edges the ingest path doesn't create from `labels`.** Worked
  around in tests by seeding the tag edge through the real tag path; root fix tracked. →
  `../../debugging/extensions/series-find-needs-tag-edges-not-labels.md`.

## Related

- README **§6.3** (extension tiers — Tier-1 wasm vs Tier-2 native), **§6.13** (extension UIs / module
  federation / design tokens), **§3** rules 4 (stateless extensions), 5 (capability-first), 7 (MCP is
  the contract).
- `scope/extensions/ui-federation-scope.md` (the page-host + bridge this consumes),
  `scope/extensions/native-tier-scope.md` + `rust/extensions/fleet-monitor/` (the native sibling this
  mirrors), `scope/extensions/lifecycle-management-scope.md` (publish-then-install),
  `scope/registry/registry-scope.md` (signed artifact path).
- `scope/ingest/` (the `series.*` verbs consumed), `scope/frontend/dashboard-widgets-scope.md` (the
  widget path this deliberately defers to).
- `public/extensions/extensions.md` (promotes here on ship).
