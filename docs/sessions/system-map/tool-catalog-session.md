# Session — system-map: tool catalog + MCP & ACP service pages

Status: shipped. Scope: `docs/scope/system-map/tool-catalog-scope.md`.

## The ask

From the System page (`/t/<ws>/system`), let an operator **see every MCP tool reachable for their
workspace — name + description + where it comes from**, plus **a dedicated page each for the MCP
runtime and the ACP adapter** with whatever status/facts we can honestly show.

## What shipped

Two new admin-gated, read-only verbs beside the existing three system-map verbs, plus two new shell
pages drilled from the System page's runtime cards.

### Backend (Rust)

- **`system.tools`** (`crates/host/src/system/tools.rs`) → `SystemTools { ws, role, tools: ToolInfo[] }`.
  Built by `collect::collect_tools`: the **static host-native catalog** (`system/catalog.rs`) +
  every extension's registry tools (`<ext>.<tool>`, name-only — the registry carries no description).
- **`system.acp`** (`crates/host/src/system/acp_verb.rs`) → `AcpInfo` from `system/acp.rs` — the
  adapter's protocol version, handled methods, advertised capabilities, JSON-RPC error codes, and
  auth/notes (mirrors `role/acp/src/session.rs`; the host owns the truth so the UI never imports the
  role binary).
- **Static host catalog** (`system/catalog.rs`): a `const` table of the built-in `host.*`/`system.*`/
  `agent.*`/`bus.*`/`store.*`/`inbox.*`/`outbox.*`/`dashboard.*`/`template.*`/`devkit.*`/`series.*`/
  `ingest.*` verbs with one-line descriptions + a group. A unit test asserts every `is_host_native`
  dispatch prefix has ≥1 entry (drift guard).
- **`ToolInfo` / `SystemTools` / `AcpInfo`** added to `system/model.rs`; exported from `system/mod.rs`
  + `host/src/lib.rs`.
- **Registry**: new `Registry::entries()` (`crates/mcp/src/registry.rs`) returns `(ext_id, tools)`
  pairs for the catalog to walk. The load-bearing `Hosted.tools: Vec<String>` was **left unchanged**
  (no ripple through `serve`/`dispatch`/remote routing/SDK) — descriptions are joined at read time,
  not stored.
- **An `acp` subsystem card** added to `collect_services` (Idle — a per-session adapter, not a polled
  resident) with an `acp → mcp` topology edge, so it appears on the grid + graph and drills to its page.
- **MCP bridge** (`system/tool.rs`) dispatches the two new verbs; **gateway** exposes
  `GET /system/tools` + `GET /system/acp` (`routes/system.rs`, `server.rs`) and grants
  `mcp:system.tools:call` + `mcp:system.acp:call` to the dev-admin role (`session/credentials.rs`).

### Frontend (TypeScript/React)

- **Types/API/transport**: `ToolInfo`/`SystemTools`/`AcpInfo` in `lib/system/system.types.ts`;
  `systemTools()`/`systemAcp()` verbs in `system.api.ts`; `system_tools`/`system_acp` → `/system/*`
  in `lib/ipc/http.ts`.
- **MCP service page** (`features/system-mcp/`): `McpServiceView` + `useMcpService` — a searchable,
  source-grouped tool table with the live runtime counts in the header.
- **ACP service page** (`features/system-acp/`): `AcpServiceView` + `useAcpService` — the adapter's
  static facts as labelled sections, honest that it is a per-session adapter (no live connection count).
- **Routing/nav**: two new `CoreSurface`s `system-mcp`/`system-acp` with routes `/system/mcp` +
  `/system/acp` (`createAppRouter.tsx`, `surface.ts`), cap-gated in `allowed.ts`. The System page's
  `mcp`/`acp` cards now drill to these pages (`features/system/navigate.ts`) instead of the detail
  sheet. The pages are NOT in the sidebar (reached by drilling) — like subsystem detail.

### Fake removed (CLAUDE §9)

While wiring caps I found `ADMIN_CAPS` in `ui/src/lib/session/admin-caps.ts` — a dead client-side
re-implementation of the gateway's `member_caps()` cap list ("the grant the fake hands back at
login"), unused except for a barrel re-export. The real caps come from the gateway's `POST /login`
reply (server-side `credentials.rs`). A parallel client copy of node behavior is exactly the banned
fake (it can silently drift), so I **hard-deleted** `ADMIN_CAPS` and its re-export. `CAP` (the cap
STRINGS the UI compares against server-issued caps) + `hasCap`/`isAdmin`/`ADMIN_SECTION_CAPS` stay —
they gate display only, never security.

## Testing (all green)

- **Host integration** (`crates/host/tests/system_map_test.rs`, real booted `Node`, no fakes):
  catalog lists host-native + a real registry-registered extension's tools, sorted, well-formed;
  capability-deny for both verbs (each needs its own cap; holding the others does not grant them);
  workspace-isolation (host portion identical node facts across A/B); ACP reports protocol v1 + its
  five methods. **13 integration + 3 unit (catalog drift guard, ACP shape) pass.**
- **UI unit**: `pnpm test` — 114 pass (no breakage; the deleted `ADMIN_CAPS` had no consumers).
- **UI real-gateway** (`pnpm test:gateway`, no fakes): `McpServiceView.gateway.test.tsx` (5) — real
  host-native tools + descriptions render, search filters, live counts, capability-deny;
  `AcpServiceView.gateway.test.tsx` (2) — real protocol/methods/error-codes render + capability-deny;
  `SystemView.gateway.test.tsx` updated (mcp/acp now drill; the phone-viewport sheet test uses the
  still-no-page `bus` card). **16 system-area gateway tests pass.**
  - Note: one **pre-existing, unrelated** flake in `DashboardView.gateway.test.tsx` (a
    react-grid-layout `onDrag called before onDragStart` drag-sim issue) fails in the full
    `test:gateway` run; untouched by this work.

## Decisions & why

- **New verb, not a wider registry type.** Joining descriptions at read time keeps the hot dispatch
  path + the SDK boundary untouched; the registry's `Vec<String>` is load-bearing across routing.
- **Static host catalog.** Host verbs are source code (no manifest), so their authoritative list is a
  `const` beside the dispatcher, kept honest by the prefix-coverage test.
- **ACP = capabilities, not health.** ACP is per-stdio-session, not a polled server — reporting a
  "running session count" would need durable session state (fights stateless/state-vs-motion). The
  page says so plainly.
- **Extension tool descriptions are empty (v1).** The `Install` record carries caps/tier/ui, not tool
  descriptions; the name still shows, labelled "no description provided" — honest, not hidden.

## Follow-ups (open questions in the scope)

- A "you can call this" column (per-principal cap check) — deferred; v1 lists existence.
- Deep-link each extension tool to `/ext/<id>` — easy once `source` is carried; left out of v1.
- Per-arg JSON schemas — needs an SDK/WIT change; out of scope.
