# Channels — the `/` + `@` command palette (session)

- Date: 2026-06-29
- Scope: ../../scope/channels/channels-command-palette-scope.md
- Stage: post-S8 (channels surface; building on the shipped MCP bridge + capability system)
- Status: in-progress

## Goal

Build the catalog-driven, capability-filtered command surface for the channel input: `/` opens a
palette of the MCP tools the caller is **authorized** to call (registered tools ∩ caps held), `@`
references entities. The palette is the reusable heart; `/query` (channels-query-charts) is its
first tenant. Exit gate: a principal sees exactly the tools they can run (no existence leak), the
palette opens with zero network in the hot path, and the host receives structured `{tool,args}` —
never raw `/`-text.

## What changed

**Tool descriptors became first-class (the enabling change).** The registry carried tool *names
only* (`Vec<String>`); it now carries a `ToolDescriptor { name, title, group, input_schema? }`:

- `crates/mcp/src/registry.rs` — `ToolDescriptor` added; `Hosted`/`Remote` targets widened from
  `Vec<String>` to `Vec<ToolDescriptor>`; `ToolDescriptor::name_only` keeps the bare-name path
  backward compatible; `Registry::descriptor_entries()` exposes the schema-bearing entries that
  `tools.catalog` walks (`entries()` stays for the names-only `system.tools` console view).
- `crates/ext-loader/src/manifest.rs` — `Tool.input_schema: Option<serde_json::Value>` added,
  `#[serde(default)]` so an extension that omits it still loads (additive + versioned by absence).
- `crates/host/src/load.rs` / `reload.rs` — `descriptors_from(manifest)` builds the descriptors for
  both load paths (one owner, cannot drift).
- Host-native verbs declare their descriptor in code next to the verb:
  `crates/host/src/federation/query.rs::query_descriptor()`, collected by
  `crates/host/src/tools/descriptor.rs::host_descriptors()`.

**The `tools.catalog` verb** (`crates/host/src/tools/`):
- `catalog.rs::tools_catalog` — gates the verb itself (`mcp:tools.catalog:call`), then enumerates
  host-native + extension descriptors and runs the **same `authorize_tool` gate the call itself
  runs** per tool, keeping only the authorized subset (one gate, two callers → the catalog can never
  advertise a tool that would then deny). Sorted by qualified name for a stable menu.
- `descriptor.rs::validate_args` — defense-in-depth JSON-Schema arg validation (the subset the
  palette's verbs use: `type:object`, `required`, shallow `properties.<k>.type`). A schema failure is
  a clean `BadInput`, never a panic; `None` schema passes.
- `tool.rs::call_tools_tool` — the `tools.*` MCP bridge dispatch.
- `crates/host/src/tool_call.rs` — added the `tools.` dispatch branch (it was recognized as
  host-native at line 60 but had no dispatch arm — an MCP call to `tools.catalog` would have fallen
  through to `call_ingest_tool`). Also runs `validate_args` before dispatch for any tool with a
  declared schema.
- `role/gateway/src/routes/mcp_catalog.rs` + `server.rs` — `GET /mcp/catalog` convenience route.
  The UI also reaches it through the existing `mcp_call` → `POST /mcp/call` bridge.

**UI** (`ui/src/...`): a single reusable `CommandPalette` with `useCatalog` (one fetch on mount,
cached), `useMentions` (`@` listers over existing verbs), `parsePalette` (pure keystroke→payload),
and the `argWidgets/` (`EntityPicker`, `SqlArg`). Result items render as cards in the channel view.
(Built in this session — see the test section / file list for the exact files.)

## Decisions & alternatives

- **Chose** standard JSON Schema + two vendor hints (`x-lb-entity`, `x-lb-widget` under an `x-lb`
  key) over a bespoke form model. Off-the-shelf validators compose; the two hints are all the UI
  needs for a guided rail without a form engine. Rejected: a hand-rolled arg-type enum (drifts from
  real validators, re-invents JSON Schema).
- **Chose** widening the registry additively (`name_only` + `#[serde(default)]` on the manifest
  field) over a breaking change. An old extension with no `input_schema` still appears in the palette
  with a single free-text arg — no rebuild. Rejected: a required field (breaks every shipped
  extension).
- **Chose** one `authorize_tool`, two callers (the catalog reuses the call's exact gate). Rejected: a
  separate "is-listable" predicate — it would drift from the call gate and either over- or
  under-advertise (the scope's #1 risk).
- **Chose** a hand-maintained UI command registry was **rejected** (per scope): it duplicates the
  permission model and drifts. The catalog makes the menu provably the caller's true verb set.

## Tests

Mandatory categories that apply: **capability-deny** (catalog omits unauthorized tools; calling
`tools.catalog` without its gate denies opaquely) and **workspace-isolation** (a ws-B caller's
catalog surfaces no ws-A tool). Plus a `tools.catalog` unit (authorized subset + schema intact), the
`validate_args` unit (in `descriptor.rs`), the UI `parsePalette` pure unit, and a real-gateway
`*.gateway.test.tsx` (catalog from one fetch, 0ms open, keyboard round-trip, reduced palette for a
no-cap principal).

Green output: _pasted below once the backend test agent + UI test agent report._

```
(cargo test -p lb-host / -p lb-role-gateway and pnpm test / pnpm test:gateway output here)
```

## Debugging

None opened for the palette itself. (The query worker's `truncated` round-trip bug is logged under
the query-charts session: [channels/query-result-missing-truncated-field.md](../../debugging/channels/query-result-missing-truncated-field.md).)

## Public / scope updates

Promoted to `docs/public/channels/channels.md` (the catalog verb + descriptor contract). Scope open
questions resolved: gate name `mcp:tools.catalog:call` (default-granted to UI-capable principals via
the session token caps); arg-schema source = JSON Schema in the manifest (`input_schema`, additive);
registry widened `Vec<String>` → `Vec<ToolDescriptor>`; entity hint vocabulary
`datasource|channel|member|agent|table`; catalog invalidation = poll-on-focus/reconnect; ranking =
client-local.

## Dead ends / surprises

- The `tools.` prefix was already registered as host-native, but the dispatch `match` had no arm for
  it — a half-wired bridge that compiled. Added the arm so the verb is reachable via the universal
  MCP contract (rule 7), not just the dedicated route.

## Follow-ups

- A bus-pushed catalog `watch` (vs. poll) is deferred — poll-on-focus is sufficient for this slice.
- STATUS.md updated.
