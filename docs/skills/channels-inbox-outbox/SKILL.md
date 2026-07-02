---
name: channels-inbox-outbox
description: >-
  Manage Lazybones channels, the durable inbox (triage/approvals), and the transactional outbox
  (must-deliver effects) over the node gateway — list/create channels, read history, post/edit/delete
  messages, stream live over SSE, record and resolve inbox items, enqueue effects, and read delivery
  status. Use when a task says "create/seed a channel", "post a message over the API", "list/resolve
  inbox items", "approve/reject a triage item", "enqueue an outbox effect", "check delivery status",
  or "call channel/inbox/outbox verbs over REST/MCP". Covers both the dedicated REST routes and the
  universal `POST /mcp/call` bridge.
---

# Managing channels, inbox, and outbox over REST / MCP

A Lazybones node exposes three durable-messaging surfaces over its HTTP gateway (default
`http://127.0.0.1:8080`, override with `VITE_GATEWAY_URL`):

- **Channels** — the collaboration surface: a registry (list/create) + durable history + live
  post/edit/delete + an SSE stream. Verbs live in `rust/crates/host/src/channel/` and
  `channel_registry/`.
- **Inbox** — the normalized durable item behind every channel: triage/approval items awaiting a
  decision (`inbox.*`, `rust/crates/host/src/inbox/`).
- **Outbox** — the transactional must-deliver backstop for external effects (open a PR, notify a
  reviewer). Enqueue + read delivery status (`outbox.*`, `rust/crates/host/src/outbox/`).

Two equivalent call styles, exactly as with `dashboard-mcp`:

1. **Dedicated REST routes** — ergonomic; the gateway supplies the clock (`ts`/`now`).
2. **The universal MCP bridge** — `POST /mcp/call {tool, args}` for ANY host verb by dotted name
   (`inbox.record`, `outbox.enqueue`, …). This is rule 7: capabilities are MCP tools, and the UI,
   agents, and extensions all call them the same way.

Both derive the **workspace + principal from the bearer token** — never from the request body (the
hard wall, README §6/§7). Every verb is capability-gated server-side; a denial is **opaque** (you
cannot tell "forbidden" from "absent" or "empty"). State is persisted *before* it is published, so a
missed live push is always recoverable from history/list (state vs motion, §3.3).

## 1. Authenticate

```bash
# dev login: who + which workspace. An empty workspace bootstraps the caller as workspace-admin.
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login \
  -H 'content-type: application/json' \
  -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)
```

Send it on every subsequent call as `Authorization: Bearer $TOKEN`. The SSE stream is the one
exception — browser `EventSource` cannot set a header, so it takes `?token=<jwt>` in the query.

Required capabilities:

- **Channels** — posting/editing/deleting a message and registering a channel need
  `bus:chan/{cid}:pub`; reading history, listing, subscribing, presence, and streaming need
  `bus:chan/{cid}:sub` (list uses `bus:chan/*:sub`). Cap resources come from `channel/key.rs`:
  `chan/{cid}` for grants, `chan/{cid}/msg/{id}` to publish, `chan/{cid}/msg/**` to subscribe. The
  workspace prefix is supplied by the bus/store layers, not embedded in the cap string.
- **Inbox** — `mcp:inbox.list:call`, `mcp:inbox.record:call`, `mcp:inbox.resolve:call`.
- **Outbox** — `mcp:outbox.status:call`, `mcp:outbox.enqueue:call`.

## 2. The verbs

| Surface | Action | REST route | MCP bridge (`POST /mcp/call`) | Args |
|---|---|---|---|---|
| Channels | List registry | `GET /channels` | — | — |
| Channels | Create (register) | `POST /channels` | — | `channel` |
| Channels | Read history | `GET /channels/{cid}/messages` | — | — |
| Channels | Post message | `POST /channels/{cid}/messages` | — | `Item` body (see §3) |
| Channels | Edit message | `PATCH /channels/{cid}/messages/{id}` | — | `{body}` |
| Channels | Delete message | `DELETE /channels/{cid}/messages/{id}` | — | — |
| Channels | Live stream (SSE) | `GET /channels/{cid}/stream?token=<jwt>` | — | — |
| Inbox | List items | `GET /inbox/{channel}` | `{"tool":"inbox.list","args":{"channel":"…"}}` | `channel` |
| Inbox | Record item | *(MCP only)* | `{"tool":"inbox.record","args":{…}}` | `channel,id,body?,ts?` |
| Inbox | Resolve item | `POST /inbox/{item}/resolve` | `{"tool":"inbox.resolve","args":{…}}` | `item_id,decision,ts?` |
| Outbox | Enqueue effect | *(MCP only)* | `{"tool":"outbox.enqueue","args":{…}}` | `id,target,action,payload?,ts?` |
| Outbox | Delivery status | `GET /outbox` | `{"tool":"outbox.status","args":{}}` | — |

Notes on determinism (README §3 — no wall-clock inside a verb):

- The **dedicated REST routes fill the timestamp from the gateway clock**, so their bodies OMIT
  `ts`. The **`/mcp/call` path requires you to pass `ts`** in `args` (defaults to `0` if absent,
  which collapses ordering — pass a real monotone value).
- `inbox.record` and `outbox.enqueue` have **no dedicated REST route** — they are reached only over
  `/mcp/call` (they are producer-side verbs, not browser surfaces).

## 3. The channel message shape (`Item`)

`POST /channels/{cid}/messages` takes a normalized `lb_inbox::Item` as its JSON body:

```jsonc
{
  "id": "msg-0001",        // stable, unique within (ws, channel). Re-posting the same id UPSERTS (idempotent).
  "channel": "general",    // forced to {cid} by the host — the path wins.
  "author": "user:ada",    // normalized source identity (user:… | key:… | ext:…).
  "body": "hello",         // textual body.
  "ts": 1719800000000      // logical, monotone-per-channel ordering timestamp. NOT wall-clock.
}
```

`history` returns every item in the channel oldest→newest (the inbox owns the order key). Edit and
delete enforce **ownership against the stored author** — the host never trusts the body's author;
both gates are `pub`, and a non-owner is refused. `history`/`subscribe`/presence are `sub`.

The live SSE stream emits `event: message` with a serialized `Item` and `event: presence` with
`{ member, present }`.

## 4. Inbox — triage & approvals

The inbox is the durable state behind a channel: items awaiting a human decision. An **approval is
not a bespoke table** — it is an `Item` tagged `needs:approval` plus a `Resolution` sibling record
addressed by the same item id.

- `inbox.record` — create an item. **`author` is forced to the caller** (never spoofable); `id` is
  caller-supplied for idempotent upsert.
- `inbox.list` — the durable items on a channel awaiting a decision.
- `inbox.resolve` — settle an item with a `decision`; idempotent on `item_id` (last decision wins,
  so a `deferred` item can later `approved`). The deciding actor is recorded from the principal.

`decision` is one of `approved` | `rejected` | `deferred` (kebab-case).

```bash
# record a triage item, then approve it
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{
  "tool":"inbox.record",
  "args":{"channel":"triage","id":"issue-2451","body":"Fix flaky test","ts":1719800000000}}'

curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{
  "tool":"inbox.resolve",
  "args":{"item_id":"issue-2451","decision":"approved","ts":1719800001000}}'

# or via the dedicated route (gateway supplies ts):
curl -s -X POST "http://127.0.0.1:8080/inbox/issue-2451/resolve" -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{"decision":"approved"}'
```

## 5. Outbox — must-deliver effects

The transactional outbox is the durability backstop for every *external effect* a workspace
produces. An effect is enqueued (ideally in the same transaction as the change that justified it)
and a relay delivers it at-least-once with retry; the receiver dedups on `idempotency_key`.

- `outbox.enqueue` — stage an effect. `payload` is **opaque to the host** (the relay's target
  adapter interprets it) — pass a string or a JSON value (it is stringified). `id` becomes the
  stable idempotency key.
- `outbox.status` — a **read-only** snapshot grouped by lifecycle: `{ pending, delivered,
  dead_lettered }`. There is no mutation verb on this surface — the relay owns delivery.

An `Effect` carries `target`, `action`, `payload`, `idempotency_key`, `status`
(`pending|delivered|failed|dead-lettered`), `attempts`, `max_attempts` (default 5),
`next_attempt_ts`, `ts`. On repeated failure the relay backs off exponentially and finally moves the
row to the terminal `dead_lettered` set (parked, readable for audit/replay).

```bash
# enqueue a "open a PR" effect, then read the delivery snapshot
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{
  "tool":"outbox.enqueue",
  "args":{"id":"pr:issue-2451","target":"github","action":"create_pr",
          "payload":{"repo":"acme/app","head":"fix","base":"main","title":"Fix","body":"…"},
          "ts":1719800002000}}'

curl -s http://127.0.0.1:8080/outbox -H "authorization: Bearer $TOKEN" | jq '.pending,.delivered,.dead_lettered'
```

## Gotchas

- **Workspace/author/actor come from the token**, never from args/body. To act in another
  workspace, `login` into it. The message `Item.author` and `inbox` author are host-forced.
- **`ts` is required-ish on `/mcp/call`** and filled by the gateway on the dedicated REST routes.
  On the bridge it defaults to `0` — always pass a real monotone value or history ordering collapses.
- **Idempotency is by stable id:** re-posting a message `(channel, id)`, re-recording an inbox item,
  re-resolving `item_id`, or re-enqueuing an effect `id` all **upsert** — never duplicate. This is
  the precondition the sync/at-least-once paths rely on.
- **Edit/delete enforce ownership** against the stored author — a non-owner is refused (as `pub`).
- **`inbox.record` / `outbox.enqueue` are MCP-only** — no dedicated REST route.
- **`outbox.status` is read-only** — you cannot mark an effect delivered from the API; the relay does.
- **Denials are opaque** — a missing cap, a missing channel, and an empty result all look the same;
  check your token's caps if a call "vanishes".
- **The registry does not gate messaging** — it is additive metadata. Posting a message
  registers the channel best-effort (create-on-first-post), so a channel can be listable without an
  explicit `POST /channels`.
- **SSE auth is `?token=`**, not a header — the gateway verifies it before opening the stream.

## Related

- Channels model + verbs (source of truth): `docs/scope/channels/channels-scope.md`,
  `docs/public/channels/channels.md`.
- Inbox/outbox model: `docs/scope/inbox-outbox/inbox-outbox-scope.md`,
  `docs/scope/inbox-outbox/outbox-scope.md`, `docs/public/inbox-outbox/`.
- Managing dashboards over the same bridge: `docs/skills/dashboard-mcp/SKILL.md`.
- Capability / workspace rules: `README.md` §3, §6, §7; `docs/scope/auth-caps/`.
- Gateway routes: `rust/role/gateway/src/server.rs` and `routes/`.
