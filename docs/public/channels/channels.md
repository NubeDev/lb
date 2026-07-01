# Channels

Channels are the shipped collaboration messaging surface. They combine durable state, live motion, a
small registry, and presence.

## Shape

The durable message shape is the shared inbox item:

```text
Item {
  id: string,
  channel: string,
  author: string,
  body: string,
  ts: number
}
```

Items are stored in the workspace namespace through `lb_inbox`, keyed by `(channel, id)`. Reposting
the same id is idempotent.

The registry shape is:

```text
ChannelRecord {
  id: string,
  created_by: string,
  kind: "channel",
  ts: number
}
```

Registry rows live in the `channel_registry` table inside the workspace namespace. They make channels
listable; they do not own the message history.

## Host flow

Posting to a channel does three things in order:

1. Authorize `bus:chan/{cid}:pub` against the session principal and workspace.
2. Persist the item to the workspace store via `lb_inbox::record`.
3. Best-effort upsert the registry row, then publish the item on the bus.

History reads authorize `bus:chan/{cid}:sub` and read durable items from the store. A missed live event
is recovered by reading history.

Live subscribe authorizes the same `sub` grant and listens to the workspace-prefixed bus key
`chan/{cid}/msg/**`.

Presence uses Zenoh liveliness. Joining holds a presence registration; dropping it or crashing removes
presence. The same `sub` grant covers declare and watch.

## Gateway routes

| Route | Meaning |
|---|---|
| `GET /channels` | List registered channels in the token workspace. |
| `POST /channels` | Register a channel before any post. |
| `GET /channels/{cid}/messages` | Read durable history. |
| `POST /channels/{cid}/messages` | Post one item. |
| `GET /channels/{cid}/stream?token=<jwt>` | SSE stream for live `message` and `presence` events. |

All routes authenticate first. The workspace comes from the token. The stream uses `?token=` only
because `EventSource` cannot send bearer headers.

## UI

The browser code is split into small files:

- `ui/src/lib/channel/channel.api.ts`: API calls.
- `ui/src/lib/channel/channel.stream.ts`: SSE stream.
- `ui/src/features/channel/useChannels.ts`: registry list/create.
- `ui/src/features/channel/useChannel.ts`: one channel's history/send/live merge.
- `ui/src/features/channel/usePresence.ts`: online roster.
- `ChannelList`, `ChannelView`, `MessageList`, and `palette/CommandPalette` (the input as a `/`
  command surface — supersedes the removed `MessageComposer`): rendering.

`useChannel` loads durable history on mount, opens a live stream when a gateway is available, and
merges live messages idempotently by id. After sending, it refreshes from durable history so the store
remains the source of truth.

`usePresence` folds unordered join/leave events into a set, then returns a sorted roster.

## Capabilities

- Post/create: `bus:chan/{cid}:pub`
- History/subscribe/presence: `bus:chan/{cid}:sub`
- List all registered channels: `bus:chan/*:sub`

There is no separate channel-registry capability. The registry reuses the channel surface gates.

## Command palette (`/` + `@`)

The channel input is a command surface. Typing `/` opens a palette of the MCP tools the caller is
**authorized** to call; typing `@` references entities. The menu *is* the permission model rendered:
a tool the caller cannot run is absent (no existence leak).

**`tools.catalog`** is the one new verb behind it. It returns, for the calling principal in this
workspace, only the authorized tools — registered tools ∩ caps held — each as a descriptor:

```text
ToolDescriptor {
  name: string,           // "federation.query" (qualified) or "<ext>.<tool>"
  title: string,          // menu label
  group: string,          // verb-family / contributing extension id
  input_schema?: object   // standard JSON Schema (type:object, properties, required)
}
```

Per-property vendor hints live under an `x-lb` key inside `input_schema`: `x-lb-entity`
(`datasource | channel | member | agent | table`) drives the `@`-picker; `x-lb-widget` (`sql | text |
…`) selects the arg widget. The catalog runs the **same `authorize_tool` gate the call itself runs**
per tool (one gate, two callers) — it can never advertise a tool that would then deny.

- **Tool descriptors are first-class:** the registry was widened from tool *names* (`Vec<String>`) to
  `Vec<ToolDescriptor>`. Extension tools declare `input_schema` in their manifest (`[tools]`,
  `#[serde(default)]` — additive: an old extension with no schema still appears, with one free-text
  arg). Host-native verbs declare a `descriptor()` per verb file.
- **Defense in depth:** the dispatcher validates a call's args against the declared schema before
  dispatch (`ToolError::BadInput` on failure, never a panic). The handler still does its own checks.
- **Gate:** `mcp:tools.catalog:call`, held by every UI-capable principal. A denial is opaque.

| Route | Meaning |
|---|---|
| `GET /mcp/catalog` | The caller's authorized tool catalog (`{ ws, tools }`). |

The UI also reaches it through the existing `mcp_call` → `POST /mcp/call` bridge
(`invoke("mcp_call", { tool: "tools.catalog", args: {} })`). The palette caches the catalog on mount
(zero network when `/` opens), revalidates on focus/reconnect, and emits a structured `{tool,args}` /
channel `Item` — the host never parses `/`-text.

## In-channel queries & charts

A member posts a SQL query into a channel by posting an `Item` whose `body` carries a kind-tagged JSON
payload. A `kind` key inside the existing `body` distinguishes the shapes — no `Item` schema
migration, and an untagged body stays an ordinary chat message:

```text
{ kind: "query",        source, sql }
{ kind: "query_result", source, sql, columns, rows, chart?, truncated? }
{ kind: "query_error",  source, sql, error }
```

An **inline host worker** runs inside `channel.post`: when the posted item is `kind:"query"`, it runs
the SQL through the existing `federation.query` verb **under the poster's principal**, caps the result
(≤500 rows / ≤256 KB, `truncated:true` when trimmed), picks a chart, and posts a `query_result` (or
`query_error`) item back under `system:query-worker`. The whole exchange is durable history and
streams over the existing SSE route.

- **Two grants, in order:** channel `bus:chan/{cid}:pub` to post the query item, then
  `federation.query`'s datasource grant when the worker runs. A `sub`-only member sees results but
  can't run their own; a `pub`-without-datasource-grant member gets an opaque `query_error` ("query
  not permitted" — a missing grant and a missing source collapse to the same message, no
  existence leak).
- **Re-entrancy guard:** only `kind:"query"` triggers work — the worker's own result/error items
  never feed it.
- **Chart spec** (`{ type: "line"|"bar"|"histogram", x, series:[{field}], bins? }`) is host-computed
  into the payload so every subscriber renders identically: temporal x → line; categorical x +
  numeric → bar; single numeric column, many rows → histogram; nothing plottable → `chart:null`
  (table-only). Conservative — fails safe to the table. **Row shape note:** `federation.query`
  returns `rows` as column-aligned **arrays** (`[[c0,c1,…],…]`); the persisted `query_result` keeps
  that compact form (the UI maps a chart series' `field` name to its column index). The worker zips a
  keyed view of the rows only to feed the picker (`query_worker::keyed_rows`) — the picker keys cells
  by column name, so without the zip every result plotted as `chart:null`.

The UI renders result items as cards: chart-first with a table toggle, `chart:null` → table-only, a
"showing first N rows" caption when truncated, an inline human error on `query_error`.

## Tests

The shipped behavior is covered by host, gateway, and real-browser-path tests:

- `rust/crates/host/tests/collaboration_test.rs`
- `rust/role/gateway/tests/gateway_test.rs`
- `rust/role/gateway/tests/gateway_routes_test.rs` (incl. `GET /mcp/catalog` + the `query_error` deny round-trip)
- `rust/role/gateway/tests/gateway_query_test.rs` (the **happy-path** query round-trip against a real
  seeded sqlite federation source: a `query_result` with columns/rows + a non-null line chart in
  history AND over SSE, plus the workspace-isolation query path)
- `rust/crates/host/tests/tools_catalog_test.rs` (catalog: authorized subset, deny, ws-isolation)
- `rust/crates/host/src/channel/{payload,chart,query_worker}.rs` unit tests (round-trip, chart
  picker, cap, opaque deny, re-entrancy)
- `ui/src/features/channel/ChannelList.gateway.test.tsx`
- `ui/src/features/channel/ChannelView.gateway.test.tsx`
- `ui/src/features/channel/usePresence.test.ts`
- the command-palette `parsePalette` unit + `*.gateway.test.tsx` (catalog one-fetch, 0ms open,
  keyboard round-trip, reduced palette for a no-cap principal)

## Rich responses — the channel is a generic MCP front-end (descriptor-driven)

A command, tool, or agent can answer in a channel with a **rich, typed response** — a chart, a table, a
stat, or an interactive control — rendered by the **shipped dashboard widget contract**, not a bespoke
channel renderer. The channel is a **second mount surface** for `WidgetView`/`views/*`, leashed to the
viewer's grant, host-re-checked per call. There is **no new render system, view vocabulary, or trust
tier** — adding a view is a widget-builder change, consumed here for free.

**The frontend has zero tool-specific knowledge.** It names exactly one tool — `tools.catalog` — and for
every command it: lists it, renders its `input_schema` widgets **by string** (the `x-lb.widget` hint),
and posts its **declared** response render. There are **no `tool.name` branches**. Two additions carry
the whole contract:

- **`kind:"rich_result"`** — a v-stamped render envelope in a channel `Item` body:
  `{ kind:"rich_result", v:2, view, source?|data?, options?, action?, tools? }`. Additive on the existing
  `query_result`/`agent_result` payloads (a body with no recognized `kind` stays chat). `tools` is the
  set the response's bridge may forward — the host intersects it with the viewer's install grant and
  re-checks per call. Mirrored one-to-one in `channel/payload.rs` ↔ `payload.types.ts`.
- **`ToolDescriptor.result`** — the **output** half of a command's contract (the `x-lb-render`
  envelope). `input_schema` drives the *form* (the palette's arg rail); `result` drives the *response*.
  Both are standard-JSON-Schema-compatible vendor extensions. A command with `result` → the palette
  posts that render (the collected args interpolated into `source.args`); without → a plain call. New
  response shapes ship from the backend/an extension with **zero** channel-UI change.

**The widget/view vocabulary is open — UI built-ins ∪ extension-contributed widgets.** The arg-widget
registry (`palette/argWidgets/registry.ts`) and the response views both resolve a string to a renderer:
a built-in (`cron`/`select`/`sql`/`entity`/`text`/`number`/`boolean`/`date` for args;
`table`/`chart`/`stat`/`switch`/`button`/… for responses) **or** an `ext:<id>/<widget>` (the shipped
`ExtWidget` federation, install-gated, leashed by `[[widget]].scope ∩ grant`). An unknown widget/view
degrades to an honest text/summary fallback — never a crash. (Arg-side `ext` widgets currently fall back
to text: the shipped `mountWidget` contract has no value channel; response-side `ext` widgets mount for
real.)

**`ResponseView`** is the thin adapter — it reads the `render` block, builds a v2 `Cell`, and mounts it
through the shipped `WidgetView`; `MessageItem` routes `kind:"rich_result"` to it beside `AgentCard`/
`QueryCard`. An interactive **table** with `options.rowControls` renders through **`ResponseTable`**,
which reuses the shipped `SwitchControl`/`ButtonControl` **per row**, passing the row object as the
control's `VarScope.values` — so a per-row control binds the row's fields with `${id}` (the shipped vars
engine) and the interaction value with `{{value}}`, no new templating slot.

**Fixed vs generative:** built-in views render **in-process** (trusted, no author code);
`template`/`plot`/`d3` render in the **sandboxed iframe** — the producer picks the tier by picking the
view. No in-process path for generated UI. `view:"template"` is the Phase-1 generative surface; A2UI/
JSON-render is a deferred additional sandboxed view.

**Migration:** the shipped `kind:"query_result"` is now **expressible** as a `rich_result`
(`view:"table"|"chart"`). The old `QueryCard` path is kept unchanged (a `query_result` still renders via
`QueryCard`) with a no-regression test — additive, no rip-out.

The first tenant is [reminders](../reminders/reminders.md#reminders-in-a-channel-the-first-rich-response-tenant):
`/remind` (a backend-declared cron+action form) and `/reminders` (an interactive table with row
controls), driven with zero reminders-specific UI.

**Named follow-ups:** make the legacy `agent.invoke`/`federation.query` palette branches
descriptor-declared routes too (finish the tool-agnostic palette); A2UI/JSON-render as an additional
sandboxed view; pin a response to a dashboard; a live-updating card.

Related docs: `../frontend/collaboration.md`, `../frontend/dashboard.md` (the widget contract this reuses),
`../reminders/reminders.md`, `../../scope/channels/channels-scope.md`,
`../../scope/channels/channels-command-palette-scope.md`,
`../../scope/channels/channels-query-charts-scope.md`,
`../../scope/channels/channels-rich-responses-scope.md`,
`../../sessions/channels/channels-docs-session.md`, and
`../../sessions/channels/channels-rich-responses-session.md`.
