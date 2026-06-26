# Multi-node + sync + SSE gateway slice (session)

- Date: 2026-06-26
- Scope: ../../scope/sync/sync-scope.md (+ bus, mcp, inbox-outbox, frontend)
- Stage: S3 — multi-node / sync / SSE (STAGES.md)
- Status: in-progress

## Goal

Build the S3 second-node + sync slice as a **vertical** slice through every layer (not "finish
the sync crate"). Two halves:

**PART 1 — make the node multi-node (headless):**
- Stand up a SECOND node in-process for tests (a hub + an edge), peers on the same Zenoh
  network. Edge vs hub is CONFIG/ROLE only — no `if cloud {…}` in `crates/`.
- Make the `mcp/dispatch` routing seam REAL: a tool call on node A resolves to an extension
  hosted on node B and routes over a Zenoh queryable, with callers and `authorize` unchanged.
  `caps::check` still runs on the calling node, workspace-first.
- Sync channel state edge↔hub per README §6.8 authority/merge: a message posted on the edge
  while offline applies idempotently on reconnect (the inbox is idempotent on `(channel,id)`).

**PART 2 — the SSE/HTTP gateway:**
- A browser reaches a REAL node over SSE/HTTP. The only UI file that changes is
  `ui/src/lib/ipc/invoke.ts` — swap the in-memory fake for a real transport; `channel.api`
  verbs and `ChannelView` stay identical.
- Push others' live messages + presence into the UI via SSE → `useChannel`'s `setItems` sink.

**Exit gate (S3), restated as the acceptance criterion:** a second node joins; a cross-node
tool call routes and is capability-checked; channel data syncs edge↔hub with idempotent offline
apply; the browser reaches a node over SSE/HTTP (replacing the S2 in-memory UI fake) and sees
live messages appear.

## What changed

_(filled in as the slice lands — see the sections below)_

## Decisions & alternatives

_(filled in as decisions are made)_

## Tests

_(green output pasted here)_

## Debugging

_(debug entries + regression tests)_

## Public / scope updates

_(promotions)_

## Follow-ups

_(deferred work)_
