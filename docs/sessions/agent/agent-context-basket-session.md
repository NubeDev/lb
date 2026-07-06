# Session — agent context basket (dock tools mode + context_items)

**Date:** 2026-07-06 · **Scope:** `docs/scope/agent/agent-context-basket-scope.md`

## What was asked

Make tool calling common between the dock and channels ("toggle tools/agent" on the dock: query a
datasource, write a note, list reminders), and make the gathered results **feed into the next agent
request**.

## What was found first

Tool calling was already common where it matters — one `mcp_call` bridge, one `tools.catalog` with
JSON-Schema descriptors consumed by both the palette forms AND the agent loop's tool menu
(`reachable_tools`). The dock simply had no tool UI, and nothing carried user-gathered results into
a run (`AgentPayload` carried goal + page context only).

## What was built

1. **`context_items` refs on the agent payload** (Rust `payload.rs` + `agent_job.rs` + UI
   `payload.types.ts`, all serde-default/skip-empty — byte-identical when absent).
2. **`channel/context_items.rs`** — resolve refs at drive time via `lb_inbox::get` (ws+channel
   scoped), fence into the goal as untrusted data; 8-ref reject cap, 8 KB/item truncation with
   marker, honest `not found` lines. Wired in `agent_worker::drive_queued_run`; the durable
   result/error still echoes the original goal.
3. **Dock Tools mode** — `DockModeToggle` mounts the SHARED channels `CommandPalette` against the
   dock session (`useDockSession` grew `postQuery/postRich/callTool/send` pass-throughs). Zero
   palette duplication.
4. **Context basket** — pure ops (`contextBasket.ts`) + state hook (`useContextBasket`, clears on
   session switch) + chip row (`DockContextBasket`) + a paperclip toggle per row via a new optional
   `contextAction` seam on `MessageList`/`MessageItem` (channels view passes nothing → unchanged).
   `AgentDock.sendAsk` folds basket + persona + page context into every ask (composer AND palette
   agent route) and clears the basket on send.

## Test evidence (all green, 2026-07-06)

- `cargo test -p lb-host --lib` → **136 passed, 0 failed** (new: fence formatting/truncation/caps,
  payload/job round-trips, **mandatory ws-isolation** `a_ref_never_resolves_across_the_workspace_wall`
  and cross-channel `a_ref_never_resolves_across_channels` against a real `Store::memory()`).
- `pnpm test` → 867 passed; the ONE failure (`radius-scale.guard` on `MarkdownView.tsx:88`) is a
  pre-existing uncommitted file from another workstream, not this change.
- `pnpm test:gateway src/features/agent-dock/AgentDock.gateway.test.tsx` → **10 passed** against a
  real spawned node, including the new case: tools-mode chat → paperclip gather → ask posts
  `context_items:[<id>]` → basket clears. Capability-deny + ws-isolation cases still green.

## Capability posture

No new caps. Refs resolve only inside the request's own `(ws, cid)` — structurally enforced by the
namespace-scoped store read; the run itself is still gated by `mcp:agent.invoke:call` under the
poster and every in-run tool re-checks `agent ∩ caller`.

## Debugging

Nothing broke that warranted a `docs/debugging/` entry. One test-authoring dead end: the first
gateway case seeded context via an agent ask, which left the composer busy forever in jsdom (no SSE
to deliver the drained answer) — reworked to seed via the tools-mode chat path, which also exercises
the palette mount.
