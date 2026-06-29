# Channels scope - message edit and delete (own messages only)

Status: in flight. Sibling to `channels-scope.md` (which covers post/history/stream/presence).

## Goal

Let a member edit or delete a channel message they authored:

- Edit the body of one of their own messages (stable `id` preserved, ordering preserved).
- Delete one of their own messages from the durable history.
- Both propagate live to other viewers over the existing SSE stream.
- Channel access and ownership stay capability-first, workspace-scoped, and opaque on denial.

Only the message's author may edit or delete it. There is no moderator/admin path yet
(see Open questions).

## Authorization model

Two gates, in order, mirroring the capability-first rule (§3.5):

1. **Channel capability gate** — same as `post`: a `bus:chan/{cid}:pub` grant (workspace-first).
   Editing or deleting is a *write* on the channel, so the write capability governs it. No new
   `Action` variant is needed; `Pub` is reused.
2. **Author-ownership gate** — the stored item's `author` MUST equal the caller's
   `principal.sub()`. The item is loaded from the durable store *before* any mutation, and the
   ownership check runs against the stored record (not the request body), so a caller cannot
   edit/delete by claiming another author in the payload.

Denial is opaque: a caller who lacks the grant, who targets another workspace's message, or who
is not the author all collapse to `ChannelError::Denied` — they cannot tell a forbidden message
from a missing one. A genuinely missing message (the caller's own id that is not present) returns
`ChannelError::NotFound` so the UI can show "already gone" for the legitimate owner.

## State vs motion

Both verbs follow the post path's persist-before-publish split (§3.3):

- **edit** — overwrite the stored `Item` (same `(channel, id)`, new `body`, bumped logical `ts`),
  then re-publish the updated `Item` on `chan/{cid}/msg/{id}`. Because the UI's live merge is an
  upsert-by-id (`useChannel::mergeItem`), republishing the same id updates the row in place for
  every viewer — no new event type is needed.
- **delete** — remove the stored `Item`, then publish a tombstone on `chan/{cid}/del/{id}`. The
  tombstone is the item id. A delete cannot ride the `msg` key because that feed deserializes to
  `Item` and would drop a non-item payload, so it has its own key expression and SSE event.

## Host surface

Implemented in `rust/crates/host/src/channel/`:

| Verb | Code | Gate | Behavior |
|---|---|---|---|
| `edit` | `edit.rs` | `bus:chan/{cid}:pub` + author == `sub()` | Load item; deny if not author; overwrite `body` + bump `ts`; persist + republish on `msg` key. |
| `delete` | `delete.rs` | `bus:chan/{cid}:pub` + author == `sub()` | Load item; deny if not author; erase from store; publish id tombstone on `del` key. |
| `watch_deletions` | `delete.rs` | `bus:chan/{cid}:sub` | Subscribe to `chan/{cid}/del/**`, yielding each deleted id (the live echo for the stream). |

Supporting additions:

- `channel/key.rs` — `del_key(cid, id)` = `chan/{cid}/del/{id}` and `del_sub_key(cid)` =
  `chan/{cid}/del/**`.
- `channel/error.rs` — new `ChannelError::NotFound`.
- `lb_store::delete` — erase `table:id` within a workspace namespace (the store had no delete
  primitive before; only `write` upsert).
- `lb_inbox::get` / `lb_inbox::delete` — read one item by `(channel, id)` and erase one item,
  the raw verbs run after the host capability gate.

`post` is unchanged. (Related hardening, tracked in Open questions: enforce
`item.author == principal.sub()` on post so the ownership chain is server-verified end to end.)

## Gateway and UI

Gateway routes (new):

- `PATCH /channels/{cid}/messages/{id}` body `{ "body": "..." }` -> `edit` (returns the stored item).
- `DELETE /channels/{cid}/messages/{id}` -> `delete`.

The SSE stream (`GET /channels/{cid}/stream`) gains a third feed — the deletion watch — and emits:

- `event: message` with a serialized `Item` (unchanged; now also carries edits, via the id upsert).
- `event: delete` with `{ "id": "..." }` (new).
- `event: presence` with `{ member, present }` (unchanged).

The UI surface:

- `ui/src/lib/channel/channel.api.ts` — `edit(ws, channel, id, body)` and `remove(ws, channel, id)`.
- `ui/src/lib/channel/channel.stream.ts` — `onDelete?(id)` handler -> `event: delete`.
- `ui/src/features/channel/useChannel.ts` — `edit`/`remove` actions; `mergeItem` already handles
  edit upsert; a `removeItem` helper drops a deleted id from the local list.

## Security invariants

- Every host verb authorizes before touching bus or store.
- Edit/delete require `bus:chan/{cid}:pub` AND author ownership.
- Workspace isolation is structural (store namespace + workspace-prefixed bus keys).
- A non-author caller learns nothing: cross-user edit/delete is a `Denied`, indistinguishable from
  a missing-capability denial.
- The ownership check reads the STORED author, so a forged `author` in the request payload cannot
  grant edit/delete of another member's message.

## Tests

- `rust/crates/host/tests/messaging_edit_test.rs` — edit own message updates body + history; live
  subscriber sees the edit; non-author edit is denied; missing id is `NotFound`.
- `rust/crates/host/tests/messaging_delete_test.rs` — delete own message erases it from history;
  live deletion feed yields the id; non-author delete is denied; capability deny runs before the
  store is touched.
- Gateway route tests for PATCH/DELETE happy path + deny, mirroring the existing post/history tests.

## Open questions

- No moderator/admin delete yet — only the author. A `chan/{cid}:mod`-style capability or a role
  gate is a follow-up if moderation is needed.
- Post does not yet server-enforce `author == principal.sub()`; ownership today trusts the UI to
  set `author` to the session's sub. Enforcing it on post would close the trust gap and make the
  ownership chain fully server-verified.
- Edit history (viewing prior revisions) is not stored; the old body is overwritten. An audit/
  revision trail would require keeping prior `rev`s (the store already bumps `rev` on each write).
