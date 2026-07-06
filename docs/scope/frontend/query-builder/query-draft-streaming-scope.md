# Query-draft streaming scope — an AI agent streams a query into the builder over the shipped bus SSE

Status: scope (the ask). Slice of [`query-builder-10x-scope.md`](query-builder-10x-scope.md). Promotes
to `public/frontend/query-builder.md` on ship.

An AI agent (or any MCP caller) should be able to **author a query live in front of the user**: pick
the table, add a join, tick columns — and the open Query workbench follows each step, with the canvas
diagram, the Rules rows, the Code text, and the SQL preview all updating together. No new transport,
no new verb, no new cap: the agent publishes the **typed model** (`SqlSourceState`) as frames on the
shipped generic bus (`bus.publish`), and the workbench follows over the shipped SSE
(`GET /bus/stream`, `openBusStream`).

## Goals

- **Stream the model, not a view.** One frame = one full `SqlSourceState`. All three editor bodies
  are already projections of that model (visual-canvas-builder slice), so canvas/rules/code sync is
  free — there is no per-editor protocol and never will be.
- **Full-state frames, idempotent.** Each frame replaces the workbench editor state. Reconnect/resume
  needs no history: the next frame is the whole truth. (State vs motion, rule 3: the bus carries
  motion only; durable saves still go through the shipped `query.save`.)
- **A subject convention, not a new namespace.** `querybuilder/<source>/draft` under the existing
  `ext/` wall (`bus.publish` walls it to `ws/{id}/ext/querybuilder/<source>/draft`). Workspace
  isolation is inherited — a caller can never name another workspace's subject.
- **Honest degrade.** No gateway (Tauri/tests), a denied `bus.watch`, or a malformed frame ⇒ the
  workbench simply doesn't follow. A frame never crashes the editor: it is parsed defensively and
  dropped if it isn't a plausible `SqlSourceState`.
- **Visible agency.** The workbench shows a small "live draft" indicator when frames are arriving,
  so the user knows the editor is being driven.

## Non-goals

- **No two-way co-editing.** The stream is agent→UI. A user edit after a frame simply wins locally
  (last writer); locking/merge semantics are a named follow-up if ever needed.
- **No token-level SQL streaming.** Frame-per-edit reads better in a builder; mid-stream token
  events stay deferred (agent scope non-goals).
- **No new backend.** `bus.publish`/`bus.watch`/`/bus/stream` ship already; both caps are already
  member-level. Nothing changes in `rust/`.
- **No persistence of drafts.** A draft frame is ephemeral motion. Saving is the existing explicit
  `query.save` flow.

## Intent / approach

New files (one responsibility each, FILE-LAYOUT):

| File | Purpose |
|---|---|
| `features/query-workbench/queryDraft.ts` | Pure: the subject convention (`draftSubject(source)`) + the defensive frame parse (`parseDraftFrame(unknown): SqlSourceState \| null`). No React. |
| `features/query-workbench/useQueryDraftFollow.ts` | The follow hook: opens `openBusStream(draftSubject(source))`, parses each frame, hands valid ones to the caller; reports `lastFrameAt` for the indicator. |

`QueryWorkbench.tsx` wires the hook to its existing `setState` and renders the indicator. The agent
side needs nothing: it already builds a `SqlBuilderQuery` headlessly (the canvas scope's "an
AI/headless caller can still build `SqlBuilderQuery` with no canvas") and calls the existing
`mcp:bus.publish:call` after each edit.

**Rejected: per-editor streams.** Three sync protocols for one model; would break the model-as-truth
invariant the canvas slice established. **Rejected: a new `querydraft.*` verb/route.** The generic
walled bus already expresses exactly this; a bespoke verb would be a second seam to secure and test.

## How it fits the core

- Rules 3, 5, 6, 7 inherited from the shipped bus verbs (workspace-first wall, `ext/` namespace,
  fire-and-forget motion). Rule 10: the subject embeds a datasource *name* (opaque config data),
  never an extension id. Rule 9: tested against the real gateway + real bus (below).

## Testing plan

- **Unit (pure):** `queryDraft.test.ts` — subject convention; `parseDraftFrame` accepts a valid
  state (round-trips), rejects junk/missing-mode/non-object frames.
- **Gateway (real node, `pnpm test:gateway`):** publish a frame via the real `bus.publish` and
  assert the mounted workbench follows (table appears, preview updates). Mandatory categories:
  **capability-deny** — a session without `mcp:bus.watch:call` gets a 403 from `/bus/stream` and
  the workbench degrades silently; **workspace-isolation** — ws-B publishing on the same subject
  never reaches ws-A's follower. jsdom has no `EventSource`; the test installs a minimal
  fetch-backed `EventSource` shim against the REAL gateway stream (a browser-API polyfill, not a
  fake backend — same spirit as the rect-stub).

## Open questions

1. **Per-datasource vs per-session subject** — v1 ships `querybuilder/<source>/draft` (one live
   draft per source per workspace). If concurrent agent drafts against one source become real,
   promote to a `draft/<id>` suffix; the frame shape doesn't change.
