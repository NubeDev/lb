# Agent context basket — gather tool results, feed them to the next ask

**Status: shipped (first slice, 2026-07-06).**

## The ask

The dock and the channel view already share ONE tool-calling substrate: the `mcp_call` bridge, the
`tools.catalog` descriptors (JSON Schema → the palette's guided arg rail), and the agent worker.
What was missing is the loop the user actually wants:

> Run a tool (query a datasource, list reminders, write a note, attach something) → the result is
> visible → **feed exactly that into the next agent request**.

Two gaps closed by this scope:

1. **The dock had no tool surface.** Its composer was ask-only; user-invoked tools existed only on
   the channels surface (`CommandPalette`).
2. **Nothing carried user-gathered results into a run.** The `kind:"agent"` payload carried only the
   goal + page context; prior `query_result`/`rich_result`/chat items never reached the model.

## The design

**Refs, not bodies.** The `kind:"agent"` payload gains `context_items: string[]` — the ids of items
in the SAME channel the user gathered. The worker resolves the bodies server-side at drive time
(`channel/context_items.rs`) and fences them into the run's goal as untrusted data, exactly like the
page-context fence (`agent/page_context.rs` is the sibling seam). Why refs:

- the request can't smuggle arbitrary bytes (the 4 KB page-context posture holds);
- the model sees exactly what durably lives in the channel — not a client-edited copy;
- workspace + channel scoping is structural: `lb_inbox::get(store, ws, cid, id)` is
  namespace-scoped, so a ref can only resolve inside the poster's workspace and the very channel
  the request was posted to (already `pub`-gated). **No new capability surface.**

Caps: > 8 refs → fail-closed honest `agent_error` (like an oversize page context); an item body
> 8 KB is truncated at a char boundary with an honest `… [truncated]` marker (durable server data —
truncation, unlike for client-sent context, is not an injection foothold); an unresolvable ref
fences as an honest `not found` line (the run still drives — context is best-effort).

**The dock mounts the shared palette, it does not clone it.** A `Ask | Tools` toggle above the
composer; Tools mode renders the channels `CommandPalette` (same catalog, same JSON-Schema arg rail,
same routes) against the dock session via `useDockSession`'s pass-throughs. A tool result is a
durable dock-channel item — which is precisely what a ref can then point at. Rejected alternative:
a dock-specific tool form (a second palette to drift, against the command-palette scope's
"built once, reused" decision).

**The basket is UI state, per session.** A paperclip toggle on every dock row gathers/releases the
item; a chip row above the composer shows what the next ask will carry; sending consumes the basket.
Rejected alternative: auto-attaching recent results (magic — the user must see and control exactly
what feeds the model).

## Wire contract (additive)

- `AgentPayload.context_items: Vec<String>` — serde-default, skipped when empty; an old post is
  byte-identical. Mirrored in `ui/src/lib/channel/payload.types.ts` (`encodeAgent` 6th arg).
- `ChannelAgentJob.context_items` — carried on the durable enqueue record; the reactor fences at
  drive time. The durable `agent_result`/`agent_error` echo the ORIGINAL goal (the fence is prompt
  material, not channel history).

## Shipped pieces

- Rust: `crates/host/src/channel/context_items.rs` (fence + caps + unit/store tests incl. the
  mandatory ws-isolation + cross-channel cases), `payload.rs` / `agent_job.rs` fields,
  `agent_worker.rs` resolve-at-drive wiring.
- UI: `features/agent-dock/{contextBasket.ts,useContextBasket.ts,DockContextBasket.tsx,
  DockModeToggle.tsx}`, palette mount + basket wiring in `AgentDock.tsx`, pass-throughs in
  `useDockSession.ts`, optional `contextAction` seam on `MessageList`/`MessageItem` (channels view
  unchanged — prop absent), `context_items` plumbing in `useChannel.ts`/`payload.types.ts`.
- Tests: Rust unit + store tests; UI unit (`contextBasket.test.ts`, `payload.test.ts`) and a real-
  gateway case in `AgentDock.gateway.test.tsx` (tools-mode chat → gather → ask carries the ref →
  basket clears).

## Open questions

- **Channels surface basket.** The payload + worker work for any channel; only the dock grew the
  gather UI. Add the same `contextAction` seam to `ChannelView` when a real need shows up.
- **File/PDF attachments.** "Attach a PDF" should ride the SAME refs once ingest exposes a
  channel-item handle for an uploaded asset (assets scope) — the fence then needs a binary-aware
  rendering (summary/extract), not a raw body dump.
- **Per-workspace caps.** 8 refs / 8 KB are fixed node defaults for the slice, mirroring the
  page-context posture; revisit with the run-supervision policy question.
