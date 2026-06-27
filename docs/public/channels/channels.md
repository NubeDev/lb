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

## Tests

The shipped behavior is covered by host, gateway, and real-browser-path tests:

- `rust/crates/host/tests/collaboration_test.rs`
- `rust/role/gateway/tests/gateway_test.rs`
- `rust/role/gateway/tests/gateway_routes_test.rs`
- `ui/src/features/channel/ChannelList.gateway.test.tsx`
- `ui/src/features/channel/ChannelView.gateway.test.tsx`
- `ui/src/lib/channel/channel.api.gateway.test.ts`
- `ui/src/features/channel/usePresence.test.ts`

Related docs: `../frontend/collaboration.md`, `../../scope/channels/channels-scope.md`, and
`../../sessions/channels/channels-docs-session.md`.
