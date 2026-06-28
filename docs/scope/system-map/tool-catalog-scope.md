# System-map scope — tool catalog + service detail pages (MCP & ACP)

Status: scope (the ask). Promotes to `public/system-map/` once shipped. Builds on
`system-map-scope.md` (the status grid + topology this extends).

We want an operator to **see every MCP tool the node can reach for their workspace — name, what it
does, and where it comes from** — and to **open a real page per service** (the MCP runtime and the ACP
adapter) instead of only a one-line card. Today the System page shows an *MCP* card with two counts
(`extensions`, `tools`) and a `bus` detail sheet, but there is no way to read the actual tool list,
no descriptions, and ACP has no presence at all. This scope adds the read verbs that surface that
catalog + the two service descriptions, and the two shell pages that render them.

> Read with: `system-map-scope.md` (the console this extends), README §6.5 (MCP dispatch — the
> contract every tool is reached through), `../host-tools/` (the built-in `host.*` verbs that must
> appear in the catalog), `../agent-run/` Part 4 (the ACP adapter whose static capabilities the ACP
> page reads), `../mcp/` (the registry the extension half of the catalog reads).

## Goals

- **A full tool catalog, with descriptions.** One workspace-scoped read returns every reachable MCP
  tool: both **extension-contributed** tools (from the runtime registry, descriptions from their
  manifests) and **host-native** tools (`host.*`, `system.*`, `agent.*`, `bus.*`, `store.*`,
  `inbox.*`, `outbox.*`, …) from a static host catalog. Each entry carries `{ tool, description,
  source, group }` so the UI can group + search.
- **A real MCP service page.** `/system/mcp` — the runtime summary (extension count, tool count) plus
  the searchable, source-grouped tool table.
- **A real ACP service page.** `/system/acp` — the ACP adapter's static capability/protocol facts
  (protocol version, supported `session/*` methods, the auth model, the rejected-client-servers
  decision, JSON-RPC error codes). Honest about what it is: a per-stdio-session adapter, not a polled
  network server, so this is **reachable capability info, not a live health feed**.
- **No new persistence.** The catalog is *derived live* from the registry + a static table at call
  time, exactly like the rest of the system map — a node restart loses nothing.

## Non-goals

- **No tool invocation from these pages.** Read-only catalog + descriptions; calling a tool stays the
  job of the surfaces that own it (the agent, a page's bridge).
- **No JSON-schema / argument introspection (v1).** The manifest carries `name` + `description`, not
  per-arg schemas; we surface what we have. Arg schemas are a future scope (needs an SDK/WIT change).
- **No live ACP session tracking.** ACP runs per-stdio-session; we do not add durable session state to
  report a running-session count (that would fight the stateless-extension and state-vs-motion rules).
  The ACP page describes the *adapter's capabilities*, not live connections.
- **No widening of the registry wire type.** We do NOT change `Hosted.tools: Vec<String>` (that ripples
  through `serve`/`dispatch`/remote routing/SDK). Descriptions are joined at read time from manifests.

## Intent / approach

Two new **read verbs** beside the existing three, same single-gate shape (`mcp:system.tools:call` /
`mcp:system.acp:call`, workspace-first §7, admin-only by grant convention):

- **`system.tools`** → `{ ws, role, tools: ToolInfo[] }`. Built in `collect.rs`'s sibling
  `collect_tools`: (1) walk the runtime `Registry` for every reachable extension and its declared
  tool names, joining each ext's live `Install` manifest for the description; (2) append a **static
  host-native catalog** — a `const` table in `host/src/system/catalog.rs` listing the built-in verbs
  with a hand-written one-line description and a `group` (the same prefixes `tool_call.rs::is_host_native`
  dispatches: `host.`, `system.`, `agent.`, `bus.`, `store.`, `inbox.`, `outbox.`, `dashboard.`,
  `template.`, `devkit.`, `series.`, `ingest.`). Each entry: `{ tool, description, source, group }`,
  where `source` is `"host"` or the `ext_id`.
- **`system.acp`** → `AcpInfo`: a `const`-derived struct describing the adapter (mirrors
  `role/acp/src/session.rs::initialize` + the method list + `rpc::codes`). Lives in
  `host/src/system/acp.rs` so the host owns the truth the page reads (the page never imports the acp
  role binary).

**Why a new verb, not enriching the registry?** The registry's `Vec<String>` is load-bearing across
`serve_call`, `dispatch`, remote-route encoding, and the SDK boundary. Widening it to carry
descriptions is a wide, risky change for a read-only display. Joining descriptions at *read* time from
the manifests already in the store (and a static table for host verbs) is local, cheap, and keeps the
hot dispatch path untouched — the same "derived live, owns no record" principle the whole map follows.

**Why a static host catalog?** Host-native verbs are *not* components — they have no manifest and are
not in the registry (`is_host_native` dispatches them directly). Their list IS source code, so the
authoritative description list is a `const` in the host, kept beside the dispatcher it mirrors. A test
asserts every `is_host_native` prefix has at least one catalog entry, so the table can't silently drift.

## How it fits the core

- **Tenancy / isolation:** every read is workspace-scoped (the registry is the node's, but the
  extension *installs* — and thus which ext tools are reachable — are read per `ws`; the static host
  catalog is workspace-agnostic facts). B's catalog never reveals A's installed extensions.
- **Capabilities:** two new caps, `mcp:system.tools:call` + `mcp:system.acp:call`, admin-only by the
  same grant convention as the other `system.*` verbs. Deny is opaque (`Denied` → 403). Added to the
  gateway dev-admin grant (`credentials.rs`) beside the existing three.
- **Placement:** either — it reads the local booted `Node` like the rest of the map; no cloud branch.
- **MCP surface:** two **read** verbs (get-shaped, whole-workspace, no id arg). No CRUD (read-only
  catalog), no live feed (poll-on-open like the rest of the console — the tool set changes only on
  install/reload, which is rare and operator-driven), no batch. Reached through the one MCP contract +
  mirrored 1:1 by `GET /system/tools` and `GET /system/acp`.
- **Data (SurrealDB):** reads existing `Install` records for manifest descriptions; writes nothing,
  adds no table.
- **Bus (Zenoh):** none (no motion; a derived read).
- **State vs motion:** pure state read, derived live. No durable snapshot.
- **Stateless:** the host owns the catalog; no extension state involved.

## Example flow

1. Admin opens `/t/acme/system` → clicks the **MCP Service** card → navigates to `/t/acme/system/mcp`.
2. The page calls `system_overview` (for the runtime counts) + `system_tools`.
3. `system.tools` gates `mcp:system.tools:call`, walks the registry (ext tools + manifest descriptions)
   and appends the static host catalog, returns `ToolInfo[]`.
4. The page renders a searchable table grouped by `source`/`group`; the admin filters to `agent.*`,
   reads each description.
5. The admin clicks the **ACP** card (new) → `/t/acme/system/acp` → `system.acp` returns the adapter's
   protocol version, methods, auth model, and error codes; the page renders them as labelled sections.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md` that apply:

- **Capability deny** (required): a token without `mcp:system.tools:call` (and without
  `mcp:system.acp:call`) is refused — opaque `Denied`. One test per new verb.
- **Workspace isolation** (required): with extension X installed in workspace A only, A's
  `system.tools` lists X's tools and B's does not (the host-native portion is identical in both — it's
  node facts, asserted equal).

Plus the key cases (host integration test, real booted `Node`, real seeded installs — no fakes):
- The static host catalog covers every `is_host_native` prefix (drift guard).
- `system.tools` includes a seeded extension's declared tool **with its manifest description**.
- Every `ToolInfo` has a non-empty `tool` and `source`.
- `system.acp` reports `protocolVersion = 1` and the five `session/*` methods the driver handles.

UI: extend `ui` unit coverage for the two new pages' rendering + the api-verb mapping, and the
real-gateway harness (`pnpm test:gateway`) for the two new `/system/*` routes returning real data.

## Risks & hard problems

- **Description drift for host verbs.** The static catalog can fall behind the dispatcher. Mitigation:
  the prefix-coverage test (every dispatched prefix has ≥1 entry). It cannot prove per-verb accuracy,
  but it stops a whole family going missing.
- **Manifest description availability.** Not every installed ext manifest is re-readable at call time
  (a remote-routed ext has only names). Fallback: empty description, `source = ext_id` — the name still
  shows. Stated, not hidden.
- **Catalog size.** The full tool list can be dozens of entries; the page must search/group, not dump.

## Open questions

- Should `system.tools` also flag which tools the **calling principal actually holds the cap for**
  (reachable vs merely existing)? Deferred — v1 lists existence; a "you can call this" column needs a
  per-tool cap check loop. Revisit if operators ask "why can't I call this".
- Should the MCP page link each tool to the extension that owns it (deep-link to `/ext/<id>`)? Easy
  follow-up once the catalog carries `source`; left out of v1 to keep the page read-only-simple.

## Related

- `system-map-scope.md` — the grid + topology this extends; `public/system-map/`.
- README §6.5 (MCP dispatch), §6.13 (frontend shell).
- `../host-tools/host-tools-scope.md` — the `host.*` verbs the catalog must include.
- `../agent-run/agent-run-scope.md` Part 4 — the ACP adapter the ACP page describes.
- `../mcp/` — the registry the extension half reads.
