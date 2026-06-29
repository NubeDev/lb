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
- `ChannelList`, `ChannelView`, `MessageList`, and `MessageComposer`: rendering.

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
  (table-only). Conservative — fails safe to the table.

The UI renders result items as cards: chart-first with a table toggle, `chart:null` → table-only, a
"showing first N rows" caption when truncated, an inline human error on `query_error`.

## Tests

The shipped behavior is covered by host, gateway, and real-browser-path tests:

- `rust/crates/host/tests/collaboration_test.rs`
- `rust/role/gateway/tests/gateway_test.rs`
- `rust/role/gateway/tests/gateway_routes_test.rs` (incl. `GET /mcp/catalog` + query round-trip)
- `rust/crates/host/tests/tools_catalog_test.rs` (catalog: authorized subset, deny, ws-isolation)
- `rust/crates/host/src/channel/{payload,chart,query_worker}.rs` unit tests (round-trip, chart
  picker, cap, opaque deny, re-entrancy)
- `ui/src/features/channel/ChannelList.gateway.test.tsx`
- `ui/src/features/channel/ChannelView.gateway.test.tsx`
- `ui/src/features/channel/usePresence.test.ts`
- the command-palette `parsePalette` unit + `*.gateway.test.tsx` (catalog one-fetch, 0ms open,
  keyboard round-trip, reduced palette for a no-cap principal)

Related docs: `../frontend/collaboration.md`, `../../scope/channels/channels-scope.md`,
`../../scope/channels/channels-command-palette-scope.md`,
`../../scope/channels/channels-query-charts-scope.md`, and
`../../sessions/channels/channels-docs-session.md`.
