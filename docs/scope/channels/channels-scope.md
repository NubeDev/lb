# Channels scope - registry, durable history, live stream, and presence

Status: shipped, backfilled from code. Original slice: `scope/frontend/collaboration-scope.md`.
Durable docs: `../../public/channels/channels.md`.

## Goal

Make channels a first-class collaboration surface:

- A user can list and create channels instead of using a single hardcoded `general`.
- Posting persists durable history and publishes live motion.
- The browser can stream live messages and presence over SSE.
- Channel access remains capability-first and workspace-scoped.

## Model

A channel has two layers:

- The message channel: `lb_host::channel`, using `lb_inbox::Item` for durable state and `lb_bus` for
  live motion.
- The channel registry: `lb_host::channel_registry`, a small store row per `(workspace, channel)` so
  the UI can enumerate channels.

The registry does not gate messaging. It is additive metadata. A channel can be registered explicitly
with `channel_create`, or implicitly on first successful post through `register_on_post`.

## Host surface

Implemented in `rust/crates/host/src/channel/`:

| Verb | Code | Gate | Behavior |
|---|---|---|---|
| `post` | `post.rs` | `bus:chan/{cid}:pub` | Persist `Item` to `lb_inbox`, register-on-post best effort, publish bus motion. |
| `history` | `history.rs` | `bus:chan/{cid}:sub` | Read durable items from the workspace namespace, oldest first. |
| `subscribe_channel` | `subscribe.rs` | `bus:chan/{cid}:sub` | Subscribe to `chan/{cid}/msg/**` bus motion. |
| `join` / `watch` | `presence.rs` | `bus:chan/{cid}:sub` | Declare and watch presence through Zenoh liveliness. |

Implemented in `rust/crates/host/src/channel_registry/`:

| Verb | Code | Gate | Behavior |
|---|---|---|---|
| `channel_create` | `create.rs` | `bus:chan/{cid}:pub` | Upsert a registry row before any message exists. |
| `channel_list` | `list.rs` | `bus:chan/*:sub` | List registered channels in the workspace, ordered by logical `ts`. |
| `register_on_post` | `register.rs` | raw helper after `post` gate | Best-effort upsert used by `post`. |

Capability resources come from `channel/key.rs`: `chan/{cid}` for grants, `chan/{cid}/msg/{id}` for
publish, and `chan/{cid}/msg/**` for subscribe. The workspace prefix is supplied by the bus/store
layers, not embedded in the cap string.

## Gateway and UI

Gateway routes:

- `GET /channels` -> `channel_list`
- `POST /channels` -> `channel_create`
- `GET /channels/{cid}/messages` -> `history`
- `POST /channels/{cid}/messages` -> `post`
- `GET /channels/{cid}/stream?token=<jwt>` -> live SSE stream

The SSE stream emits:

- `event: message` with a serialized `Item`
- `event: presence` with `{ member, present }`

SSE uses `?token=` because browser `EventSource` cannot set an `Authorization` header. The gateway
verifies that token before opening the stream and then runs the host `sub` gates.

The UI surface is split by responsibility:

- `ui/src/lib/channel/channel.api.ts`: named API calls for history, post, list, and create.
- `ui/src/lib/channel/channel.stream.ts`: SSE connection and event decoding.
- `features/channel/useChannel.ts`: one channel's history, send, and live message merge.
- `features/channel/useChannels.ts`: registry list/create for the switcher.
- `features/channel/usePresence.ts`: idempotent online roster reducer.
- `ChannelList` and `ChannelView`: rendering only.

## Security invariants

- Every host verb authorizes before touching bus or store.
- A post requires `bus:chan/{cid}:pub`.
- History, subscribe, presence, and list require `sub` authority.
- Workspace isolation is structural: store reads use the workspace namespace, and bus keys are
  workspace-prefixed by `lb_bus`.
- A forbidden read is opaque to the caller; it does not reveal whether a channel exists.
- The registry cannot leak messages because it stores only channel metadata.

## Tests

Covered by:

- `rust/crates/host/tests/collaboration_test.rs`: channel registry deny and workspace isolation.
- `rust/role/gateway/tests/gateway_test.rs`: signed session, deny, and ws-B cannot read ws-A history
  or channel list.
- `rust/role/gateway/tests/gateway_routes_test.rs`: explicit create, create-on-post, live SSE, and
  stream authentication.
- `ui/src/features/channel/*.gateway.test.tsx`: real-gateway channel list and message behavior.
- `ui/src/features/channel/usePresence.test.ts`: idempotent presence reducer.

## Open questions

- Channel metadata beyond `id`, `created_by`, and `ts` is not shipped yet.
- The SSE token is the full session token today; a shorter stream token or cookie is a deployment
  hardening follow-up.
- Registry repair from existing message history is not needed for normal operation because posting
  registers best effort, but an operator repair command could be useful after historical imports.
