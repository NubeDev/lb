# Channels scope — the `/` + `@` command palette (catalog-driven, capability-filtered)

Status: scope (the ask). Promotes to `public/channels/` once shipped.
Topic: `channels`. Powers `channels-query-charts-scope.md` (its first tenant) and every future
in-channel verb. Builds on the MCP bridge (`rust/crates/host/src/tool_call.rs`) and the capability
system (rule 5/7).

The channel input is a **command surface**, not a chat box. Typing `/` opens a palette of the MCP tools
the caller is **authorized to call**; typing `@` references entities the caller can **list** (people,
channels, datasources, agents, tables). The palette is catalog-driven and capability-filtered, so the
menu *is* the permission model rendered — a user never sees a verb they cannot run. Built once as a
reusable component, every verb (`/query`, `/ask`, `/rule`, …) inherits the same keyboard-first UX.

## Goals

- `/` lists the tools this principal can call — **registered tools ∩ caps held** — with titles and
  argument schemas, so the palette can render a guided argument form.
- `@` references addressable entities (people, channels, datasources, tables, agents), each backed by
  an existing list verb, filtered by caps the same way.
- The palette resolves a keystroke sequence into a **structured `{tool, args}`** (or a `kind`-tagged
  channel `Item`) — the host never parses chat text.
- **Amazing UX is a tested requirement, not a hope** (see Acceptance criteria): 0ms open, keyboard-only,
  chip tokens, no dead-ends, opaque denies.
- One reusable `CommandPalette` so the query feature and all future verbs share the same UX.

## Non-goals

- No new execution path — the palette only *composes* a call; dispatch is the existing `POST /mcp/call`
  bridge or `channel.post` (for `kind`-tagged items).
- No host-side parsing of `/...` or `@...` text — parsing is client-side only.
- No bespoke per-command registry — the catalog is derived from the MCP registry + caps, nothing hand-maintained.
- Not a full IDE: schema-aware SQL autocomplete is in scope as a *widget*, but a multi-tab editor,
  query history browser, and saved queries are later work.
- No mobile/touch gesture design in this slice (keyboard-first; touch falls back to tap-to-select).

## Intent / approach

Two new pieces, one host + one UI:

**Tool descriptors — JSON Schema is now first-class** (the enabling change). Today the registry carries
**tool names only** (`rust/crates/mcp/src/registry.rs` — `Hosted.tools: Vec<String>`); there is no arg
schema anywhere. The palette's argument rail needs one, so each tool gains a declared **JSON Schema**
for its input:

- A tool descriptor is `{ name, title, group, input_schema }`, where `input_schema` is a standard
  JSON Schema object (`type: "object"`, `properties`, `required`). Per-property we add two optional
  vendor hints under an `x-lb` extension key: `x-lb-entity` (`datasource | channel | member | agent |
  table`) drives `@`-picker autocomplete, and `x-lb-widget` (`sql | text | …`) selects the arg widget.
  Standard JSON Schema everywhere else (so existing tooling and validators just work).
- **Extension tools** declare `input_schema` in their **manifest**, alongside the tool name they already
  declare (extensions scope). The registry is widened from `Vec<String>` (names) to carry the descriptor
  per tool. This touches the manifest shape and the registry — flag it as an SDK/WIT-adjacent change
  (see checklist) and version the manifest field additively (absent schema → a single free-text arg,
  so old extensions still appear in the palette, just without a guided rail).
- **Host-native verbs** (`federation.query`, `channel.*`, `rules.*`, …) declare their descriptor in
  code next to the verb (one `descriptor()` per verb file, FILE-LAYOUT), collected by `tools.catalog`.
- The host **validates args against the schema before dispatch** (defense in depth; the per-verb handler
  still does its own checks). A request failing schema validation is a clean `InvalidArgs`, not a panic.

**Host — `tools.catalog`** (the new verb). Returns, for the calling principal, only the tools they are
authorized for, each as the descriptor `{ name, title, group, input_schema }` above. The handler walks
the MCP registry + host-native descriptors and, for each tool, runs the **same `authorize_tool` gate the
call itself would** — so the catalog can never advertise a tool the call would deny. It reuses the gate;
it does not re-derive permission logic. Gated by `mcp:tools.catalog:call` (every principal that can use
the UI holds it; it leaks only the tool *shape*, not data).

**UI — `CommandPalette`.** A single reusable component over a real editor input:

- `/` at line-start → command mode; `@` anywhere → mention mode; one menu, one key-map, modes
  reclassify on the fly (no second component, no flicker).
- The catalog is fetched **once on channel mount and cached** — `/` opens from memory in 0ms, no network
  in the hot path. `@` entity lists are stale-while-revalidate cached; typing filters locally.
- Accepting a tool drives an **argument rail** from its schema: an `entity` arg auto-opens the matching
  picker (the existing list verb); a `widget:"sql"` arg renders a real mini SQL editor with
  schema-aware table/column autocomplete sourced from the discovery SELECTs already in
  `useDatasourceQuery`.
- Picked entities become **chip tokens** — solid, non-fragile, not raw text that one backspace can
  corrupt. On submit the palette emits the structured payload.

**Alternative considered — a hand-maintained command registry in the UI.** Rejected: it duplicates the
permission model, drifts from the real tool set, and would show commands a user can't run (or hide ones
they can). Deriving the palette from `tools.catalog` makes the menu *provably* the caller's true verb
set (rule 5/7), and new extensions get palette entries for free.

## How it fits the core

- **Tenancy / isolation:** `tools.catalog` is workspace-scoped — it lists tools authorized for this
  principal *in this workspace*. Entity listers (`datasource.list`, `channel_list`, `members`) are
  already workspace-scoped. A ws-B user's palette can never surface a ws-A source or channel.
- **Capabilities:** the palette is *defined by* caps — registered tools ∩ caps held. A tool the caller
  lacks `mcp:<tool>:call` for is **absent**, not greyed out (no existence leak). `tools.catalog` itself
  is gated. Entity pickers inherit each lister's read gate. **Deny is invisible by construction.**
- **Placement:** either — `tools.catalog` is symmetric host code over the registry; no `if cloud`.
- **MCP surface** (API shape, §6.1):
  - **Get / list:** `tools.catalog` (read-only; the caller's authorized tool set with arg schemas).
    This is the *only* new verb. Entity `@`-pickers reuse existing listers (`datasource.list`,
    `channel_list`, `members`, agent registry) — no new read verbs.
  - **Create / update / delete:** none — the palette composes calls, it owns no state.
  - **Live feed:** none in this slice. Catalog is refetched on cap change (cheap); a future `watch`
    could push catalog invalidation, but polling-on-mount + revalidate is sufficient. Say so.
  - **Batch:** N/A.
- **Data (SurrealDB):** none new — `tools.catalog` reads the in-memory registry + the caller's grants
  (already in the store). No new table.
- **Bus (Zenoh):** none — the palette is request/response over the gateway; emitted calls use the
  existing bridge or `channel.post`.
- **Sync / authority:** node-local registry read; no offline concern beyond "catalog cached, usable
  offline until caps change."
- **Secrets:** none — the catalog returns tool *shapes*, never secret material; DSNs stay in the secret
  store, surfaced only as a pickable source *name*.
- **Stateless:** the palette holds only ephemeral UI state; nothing durable.
- **One responsibility per file** (FILE-LAYOUT): host `tools/catalog.rs`; UI split into
  `CommandPalette.tsx` (render), `useCatalog.ts` (cached fetch), `useMentions.ts` (entity listers),
  `argWidgets/` (one file per widget: `EntityPicker.tsx`, `SqlArg.tsx`), `parsePalette.ts` (keystroke →
  structured payload). No `utils.ts`.

## Example flow

1. Alice opens `#data`. On mount the UI calls `GET /mcp/catalog` once → `[datasource.query, agent.ask,
   rules.eval, channel.list, …]` (her authorized set; `datasource.add`/`grants.assign` absent — no admin cap).
2. She types `/`. The menu opens **instantly from cache** (0ms), fuzzy-ranked by recency: `/query` first.
3. `/que` + Enter accepts `datasource.query`. Its schema has `source` (`entity:datasource`) and `sql`
   (`widget:sql`). The argument rail auto-opens the **datasource picker** (from cached `datasource.list`).
4. `@wa` → `@warehouse` chip. Focus moves to the SQL widget — a real editor. Typing `FROM da` suggests
   `daily` (schema autocomplete from the discovery SELECT). `Ctrl+Enter` submits.
5. The palette emits the structured channel `Item` `{ kind:"query", source:"warehouse", sql:"…" }` to
   `POST /channels/data/messages` — never a parsed string. The query-charts worker takes over.
6. Bob (no `datasource.query` cap) opens the same channel: `/` shows **no `/query`** at all. He can
   still `/ask` an agent. He never learns `warehouse` exists.

## Acceptance criteria (UX is tested, not hoped)

These are hard, checkable requirements — the implementing session must demonstrate each:

- **0ms open:** `/` opens with no network call in the path (catalog pre-cached on mount). Asserted by a
  test that opens the palette with the network seam unused after mount.
- **Capability-filtered:** a principal without `mcp:datasource.query:call` gets a catalog **without**
  `datasource.query`; the palette renders no `/query`. Asserted against the real gateway with two
  seeded principals (one granted, one not) — no existence leak.
- **Keyboard-only round-trip:** select tool → fill `@entity` → submit, using only `/ @ ↑ ↓ ⏎ ⌫ Esc Tab`.
- **Chip integrity:** one `⌫` after a chip removes the *whole* chip, never half a source name.
- **No dead-ends:** empty entity list renders a reason ("No datasources you can query"), never a blank
  box or infinite spinner; a denied execution renders an inline human error, opaque about existence.
- **Structured emit:** submitting produces the exact `{tool,args}` / `Item` payload; the host receives
  no raw `/`-text. Asserted by inspecting the request body in the gateway test.

## Testing plan

Per `scope/testing/testing-scope.md`; no mocks — real gateway, real registry, real seeded caps.

- **Capability deny (mandatory):** `tools.catalog` returns only authorized tools — seed principal A with
  `datasource.query`, principal B without; assert B's catalog omits it and B's palette has no `/query`.
  Assert calling `tools.catalog` itself without its gate denies opaquely.
- **Workspace isolation (mandatory):** ws-B principal's catalog and `@`-entity lists contain no ws-A
  tools/sources/channels. Mirror the `gateway_test.rs` ws-A/ws-B structure.
- **Catalog unit (host):** registry with a mix of authorized/denied tools → catalog == the authorized
  subset, with arg schemas intact. Table-driven, one file.
- **Parse unit (UI):** `parsePalette` keystroke sequences → structured payloads; `@`-chip insert/delete,
  mode reclassification, fuzzy ranking. Pure, no network.
- **UI integration (real gateway, `*.gateway.test.tsx`, no fakes):** mount renders catalog from one
  fetch; `/` opens with no further fetch (0ms criterion); keyboard round-trip emits the structured
  payload; a no-cap principal sees a reduced palette. Seed via the real gateway per rule 9.

## Risks & hard problems

- **Catalog ↔ call drift.** If `tools.catalog` filters tools with *different* logic than the call gate,
  the palette could offer a tool that then denies (or hide one that would pass). **Mitigation:** the
  catalog MUST call the same `authorize_tool` per tool — one gate, two callers. Tested by asserting
  every catalog tool actually dispatches for that principal.
- **Catalog staleness after a grant change.** Caps can change mid-session. **Mitigation:** revalidate
  the catalog on focus/reconnect and after any `grants.*` call the UI makes; accept brief staleness
  (worst case: a denied call renders an opaque inline error — never a crash).
- **Latency creep.** The 0ms feel dies the instant a fetch sneaks into the `/` path. Guard with the test
  above; keep entity lists cached + SWR.
- **Arg-schema expressiveness.** Over-modeling arg types becomes a mini form-engine. Lean on standard
  JSON Schema for structure and keep the vendor hints small (`x-lb-entity`, `x-lb-widget`); let unknown
  or schema-less args fall back to a plain text input.
- **Registry/manifest widening (SDK-adjacent).** Carrying `input_schema` per tool changes the manifest
  shape and the `Registry` struct — a stable-boundary touch. Make it **additive and versioned** (absent
  schema is valid → degrades to a free-text arg) so existing extensions keep working without a rebuild.
- **Schema autocomplete cost.** Table/column suggestions come from discovery SELECTs — cache per source,
  don't re-discover on every keystroke.

## Open questions

All resolved in the build session ([channels-command-palette-session.md](../../sessions/channels/channels-command-palette-session.md)):

- **`tools.catalog` gate — DECIDED: `mcp:tools.catalog:call`**, held by every UI-capable principal
  (carried in the session token caps; without it there is no palette). A denial is opaque (403).
- **Arg-schema source — DECIDED:** a JSON Schema `input_schema` per tool. Manifest field name is
  `input_schema` (`ext-loader/manifest.rs`, `#[serde(default)]` → additive); the registry widened
  `Hosted/Remote.tools` from `Vec<String>` to `Vec<ToolDescriptor>` (`name_only` keeps the bare-name
  path). Host-native verbs declare a `descriptor()` per verb file. Standard JSON Schema +
  `x-lb-entity` / `x-lb-widget` hints; absent schema → one free-text arg.
- **Entity hint vocabulary — DECIDED:** `datasource | channel | member | agent | table`, each backed
  by an existing lister (`datasource.list`, `channel_list`, members, agent registry).
- **Catalog invalidation — DECIDED: poll-on-focus/reconnect** (and after any `grants.*` call). A
  bus-pushed `watch` is a deferred future option.
- **Recency/frequency ranking — DECIDED: client-local (localStorage)**; promote to `prefs.*` later if
  cross-device ranking is wanted.

## Related

- `scope/channels/channels-query-charts-scope.md` — the first tenant; its `/query` flow drives this.
- `scope/channels/channels-scope.md` — the channel transport the emitted `Item`s ride.
- `scope/datasources/datasources-scope.md` — the `@datasource` lister and the discovery SELECTs reused
  for SQL autocomplete (`ui/src/features/datasources/useDatasourceQuery.ts`).
- `rust/crates/host/src/tool_call.rs` — the one MCP bridge every emitted call dispatches through.
- `rust/crates/host/src/authz/grants.rs` (`grants_list`) — the cap-list the catalog intersects with.
- `README.md` §3 (rules 5/7), §6 (channels/bus), §6.7 (secret redaction).
- `doc-site/content/public/channels/channels.mdx` — promotion target on ship.
