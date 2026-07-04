# Frontend scope — the agent dock (persistent AI side panel, page-context aware, streamed)

Status: scope (the ask). Promotes to `public/frontend/` once shipped.

Add a persistent **agent dock**: from the footer status bar (or `mod+j`), on **any page**,
the user opens a right-docked AI panel and asks a question. The panel **stays open across
navigation** (it is shell-mounted, outside the routed page), keeps **durable history**
per session, supports **"new session"**, and always talks to the **workspace's active
agent** from Settings → Agent catalog. The invoke carries **where the user is** (surface,
path, typed search params) so the agent can answer about the thing on screen. Responses
**stream over SSE** with visible progress (thinking / tool calls / text deltas) from the
first moment — agent answers can be slow and a dead spinner reads as broken.

## Goals

- A launcher in the existing footer (`ui/src/features/shell/StatusBar.tsx`) plus a global
  `mod+j` toggle; the dock is available on every authenticated page.
- **Persistent, non-modal, resizable right side panel** built on `@nube/panel`
  (`Panel` + `useResizable` + `ResizeHandle`), mounted once at the shell
  (`RoutedShell.tsx`) beside `<Outlet/>` — the page shrinks, the user keeps working,
  navigation never closes it. IDE-style, same feel as the panel chrome Data Studio users
  already know.
- **Durable history, sessions, and "new session"** — each dock session is a real
  **channel** with a reserved id prefix (`dock.`). History is `channel.history`, live
  updates are the channel SSE stream, the answer is the durable `agent_result` item the
  shipped channel agent worker already posts. "New session" mints a fresh `dock.` channel;
  a session picker lists past ones.
- **The active agent, always** — the channel agent worker resolves the runtime via
  `agent.config` (explicit arg → workspace active pick → registry default,
  `resolve_default.rs`); the dock passes no runtime, so the Settings "ACTIVE" definition
  is what answers. Switching the active agent in Settings changes the dock's brain with
  zero dock code.
- The invoke includes a structured **page-context** payload derived from the router
  (surface + pathname + search params), injected into the agent's prompt as untrusted
  context — the agent knows "user is on X" without the user retyping it.
- **Live feedback end to end**: immediate acknowledgment, then streamed
  reasoning/tool-call/text deltas via the run-event SSE stream, then the durable final
  answer. Visible states for connecting, streaming, stalled, error, and done — no silent
  multi-second gaps.
- Same capability gates as every other channel/agent surface — no new privilege, no new
  verbs, no new tables.

## Non-goals

- **Not a new agent and not a new chat backend.** This is a new front door over shipped
  pieces: channels (storage + history + SSE), the channel agent worker (durable job,
  active-runtime resolution, `agent_result`), and the run-event stream (progress).
- **Not the channels page.** Dock sessions are filtered out of the channels surface
  (`dock.` prefix) — they are the dock's storage, not another room in the channel list.
  No presence strip, no multi-user affordances in the dock UI (though the storage would
  technically allow it).
- **No per-member-private channels.** Channels today have no membership/ACL model —
  access is workspace + `bus:chan/{cid}:pub|sub` caps, and member grants are
  workspace-wide. Dock history therefore has the **same visibility as any channel** in
  the workspace. Building a private-channel ACL is the channels topic's follow-up, not
  this scope's; the dock inherits it for free when it lands. Stated honestly in Risks.
- **No screen scraping.** Context is the *router state*, not the rendered DOM.
- **No agent-initiated navigation/actions on the page** in v1.
- **Not FlexLayout.** The Data Studio workbench (FlexLayout) is page-scoped inside a
  route; a dock that must outlive navigation cannot live there. Rejected in favor of a
  shell-mounted `@nube/panel` (see Intent).

## Intent / approach

The dock is a **thin channel client in a persistent shell panel**. Three shipped pieces,
one small host seam:

1. **Storage + transport = a channel per session.** Opening the dock (first send) creates
   a channel with id `dock-{user-slug}-{ulid}` (create-on-post — no new verb needed; the
   create gate is the existing `bus:chan/{cid}:pub` check). The dock reuses `useChannel`'s
   data path: `channel.history` on open, `openChannelStream` for live items, `postAgent`
   to send — which mints the run/job id client-side, posts a `kind:"agent"` item, and the
   durable **channel agent job** drives the run under the poster's caps and posts
   `agent_result`/`agent_error` back. History, durability, reconnect, and the active-agent
   pick all come from code that already ships.
2. **Progress = the run-event SSE stream.** While the run is live the dock subscribes to
   `GET /runs/{job}/stream` (the `useRunFeed` folding pattern / `run.stream.ts`) for
   reasoning/tool-call/text deltas — same as `AgentCard`'s `RunningCard`. The synchronous
   `POST /agent/invoke` is rejected as transport: it blocks until completion with no
   progress, the exact UX failure this scope exists to fix.
3. **Page context = a new optional `context` field on the agent item payload.** A small
   JSON object `{ surface, path, search }` built at the shell from what `RoutedShell.tsx`
   / `surfaceForPath` already know, carried in the `kind:"agent"` payload; the agent
   worker fences it into the prompt as **untrusted client-reported context**. Workspace
   and caps stay token-derived, unchanged.
4. **Frame = `@nube/panel` at the shell.** `RoutedShell` gains a right-hand slot beside
   `<Outlet/>`: `Panel` + `ResizeHandle` + `useResizable`, non-modal, open/closed state
   and width persisted (same idiom as the sidebar's cookie-persisted state). The message
   body reuses the presentation-only `MessageList` (`ui/src/features/channel/MessageList.tsx`
   — it takes items as props and has no page dependency) with `AgentCard` rendering.

Alternatives rejected:
- **Ephemeral in-memory history** (the earlier draft): the user wants history and
  sessions; channels give both durably with zero new surface. Rebuilding a lighter store
  would be a second chat-persistence path — exactly the drift rule 9 exists to prevent.
- **A `Sheet` overlay** (the earlier draft): modal-ish, blocks the page, fights "keep it
  open while I work". `@nube/panel` is the purpose-built resizable dock and is already a
  dependency.
- **FlexLayout at the shell**: heavyweight, and its layout state is per-page by design;
  the shell panel needs to be route-independent.

## How it fits the core

- **Tenancy / isolation:** nothing new — channel create/post/history/stream and
  `watch_run` all derive workspace from the token and are already isolation-tested;
  the `context` field is display/prompt data only, never a key.
- **Capabilities:** existing gates only. Send = `bus:chan/{cid}:pub` (create-on-post) +
  the agent job runs under the poster's captured caps; read = `bus:chan/{cid}:sub`;
  progress stream = `mcp:agent.watch:call`. Deny paths: no pub cap → post 403, dock shows
  a capability error; no watch cap → the progress stream 403s but the durable
  `agent_result` still arrives via the channel stream (degraded-but-honest: answer without
  live deltas, plus a "no live progress (missing agent.watch)" notice).
- **Placement:** either — pure UI + existing gateway routes; no role branch.
- **MCP surface (API shape, §6.1):** consumes existing verbs only — `channel.post` /
  `channel.history` / `channel.list` (write / get-list) and the two live feeds
  (`/channels/{cid}/stream`, `/runs/{job}/stream`). The only change is **additive**: an
  optional `context` object inside the `kind:"agent"` item payload (and, for parity, on
  `agent.invoke` args / `InvokeRequest`). No new tool, no new cap. Batch: N/A.
- **Data (SurrealDB):** no new tables. Dock sessions are ordinary `ChannelRecord`s with a
  reserved `dock.` id prefix; items/transcripts are the existing channel-item and
  run-event records. UI-only state (panel open/width, current session id) is client-side.
- **Bus (Zenoh):** none new — channel items and run events already flow.
- **Sync / authority / secrets:** N/A beyond existing channel/run semantics.
- **Stateless extensions / symmetric nodes / core-knows-no-extension:** untouched; the
  dock is core-shell UI over core verbs. The `dock.` prefix is a **UI naming convention**,
  not a host branch — no core crate treats it specially.
- **SDK/WIT impact:** none.

## The `dock.` session convention

- Session channel id: `dock-{user-slug}-{ulid}` — minted by the dock, created on first
  post. The prefix is reserved by convention in the UI:
  - the channels surface (`ChannelList`) filters out `dock-*` ids;
  - the dock's session picker is `channel.list` filtered **to** `dock-{user-slug}-*`.
  - (delimiter is `-`, not `.`, per "Resolved during implementation" #1 — a cap-grammar
    constraint; the reserved-prefix idea is unchanged.)
- "New session" = mint a new ulid; the old channel simply stops being current and remains
  listable/reopenable. No delete in v1 (channel.delete exists if we want "clear" later).
- The host does **not** know the prefix (rule: convention lives in the UI; the wall is
  caps, not the name).

## The invoke `context` contract

```jsonc
// optional field on the kind:"agent" item payload (and agent.invoke args for parity)
"context": {
  "surface": "dashboards",          // Surface from surfaceForPath — opaque string
  "path": "/t/acme/dashboards",     // tenant-stripped pathname
  "search": { "d": "sales", "from": "now-24h", "to": "now" }  // typed search params, flat
}
```

Host handling (agent worker + invoke path): serialize into a clearly-fenced block
appended to the prompt ("The user is currently viewing … — untrusted client-reported
context"). Size-capped (reject > 4 KB). Absent field ⇒ behavior identical to today.
Context is captured **per message at send time** — ask, navigate, ask again: the second
message carries the new page.

## Streaming UX states (the feedback contract)

Driven by `useRunFeed`-style folding of `RunEvent`s
(`run-start | step-start | reasoning-delta | text-delta | tool-call-* | run-finish`):

1. **Sent** — item posted (optimistic), run stream connecting.
2. **Working** — `run-start`/`reasoning-delta`/`tool-call-*` arriving: live activity line
   ("thinking…", "calling `series.query`…"), elapsed timer.
3. **Answering** — `text-delta`s append visibly as they arrive.
4. **Stalled** — stream open but no event for 15 s: keep the timer, add a "still working"
   hint; not an error. (A true run-timeout already exists server-side: the channel agent
   job's 15-min wall ceiling posts an honest `agent_error`.)
5. **Done** — `run-finish`; the durable `agent_result` item reconciles via the channel
   stream and becomes the message of record.
6. **Error** — post rejection, stream 401/403, `agent_error` item, or `EventSource`
   error: a real message with a retry affordance — never an infinite spinner.

## Resolved decisions

1. **Form factor — persistent `@nube/panel` right dock**, shell-mounted, resizable,
   non-modal; open state + width persisted. Escape (when dock focused) closes; focus
   returns to the launcher. Obeys `ui-standards-scope.md`.
2. **History — durable, channel-backed.** Each session is a `dock.`-prefixed channel;
   "new session" mints a fresh one; the picker lists the user's past sessions. No new
   persistence surface.
3. **Per-feature selection context — URL-only in v1, behind a provider seam.** The shell
   exposes a `PageContextProvider` defaulting to the router-derived object; features may
   override later (active panel, focused cell). v1 ships only the router default.
4. **Hotkey — `mod+j`.** No global hotkey layer exists today (the channel command palette
   opens via the `/agent` command, not a shortcut; the sidebar uses `mod+b` inside the
   shadcn provider), so `mod+j` collides with nothing. One `keydown` listener at the
   shell; no hotkey library.
5. **Agent selection — none in the dock.** The dock always rides the workspace's active
   catalog pick via the worker's `resolve_effective_runtime_id`; changing agents is a
   Settings concern.

## Example flow

1. User is on `/t/acme/dashboards?d=sales&from=now-24h` and presses `mod+j` (or the
   StatusBar button). The panel slides in on the right; the page reflows narrower. The
   session picker shows past `dock.` sessions; the newest is current. A caption shows the
   captured context ("asking about: dashboards · sales").
2. User types "why did throughput dip this morning?" and sends. The dock mints the run
   id, posts a `kind:"agent"` item with `{ goal, job, context }` to the current `dock.`
   channel (created on this first post if new). State: **Sent**.
3. The durable channel agent job picks it up, resolves the **active** runtime from
   `agent.config`, and drives the run. The dock's run stream shows `reasoning-delta`s and
   a `tool-call-start` live. State: **Working**, elapsed 3 s.
4. `text-delta`s stream the answer. State: **Answering**.
5. User navigates to `/t/acme/flows` mid-answer; the panel and stream are untouched
   (shell-mounted).
6. `run-finish`; the worker's durable `agent_result` item arrives on the channel stream
   and reconciles as the final message. A follow-up send captures the **new** page
   context (`flows`).
7. User clicks **New session**: a fresh `dock.` ulid becomes current; the previous
   session stays in the picker. Reopening the app later, `channel.history` restores
   whichever session is selected — history survives restarts because it was never
   anywhere but SurrealDB.

## Testing plan

Per `scope/testing/testing-scope.md` — real store/bus/gateway (`cd ui && pnpm
test:gateway` spawns `test_gateway`; `cd rust && cargo test --workspace`; no fakes,
rule 9; seed members/caps via `signInWithCaps` where the dev-login set is short):

- **Capability deny (mandatory):** member without `bus:chan/{cid}:pub` → post 403, dock
  shows the capability error (not a hang). Member without `mcp:agent.watch:call` → run
  stream 403 → dock degrades to "answer without live progress" and still renders the
  durable `agent_result`.
- **Workspace isolation (mandatory):** workspace-B token reading a workspace-A `dock.`
  channel history/stream → deny; workspace-B token on workspace-A's `/runs/{job}/stream`
  → 403 before any SSE body.
- **Host context injection:** invoke/agent-item with `context` → the runtime prompt
  contains the fenced block; oversize (>4 KB) rejected; absent context byte-identical to
  today.
- **Session lifecycle (gateway):** first send creates the `dock.` channel; history
  restores after remount; "new session" mints a second channel; the channels surface
  list excludes `dock.*`; the dock picker includes only the user's own prefix.
- **Streaming UX (gateway):** drive a real run end to end; assert
  Sent → Working → Answering → Done from real SSE frames; late-open gets the run-stream
  catch-up snapshot; killed stream → error + retry; worker `agent_error` renders the
  error state.
- **Navigation persistence:** start a run, change route, assert the panel stays mounted
  and the stream keeps folding.
- **Unit:** context builder (tenant-stripped surface/path/search), stall-timer state
  machine, `dock.` id mint/filter helpers.
- Offline/sync, hot-reload: N/A (no extension instance, no new durable surface).

## Risks & hard problems

- **Dock history is workspace-visible.** Channels have no membership ACL; anyone with
  workspace-wide `bus:chan/*:sub` (today's member grant) can read `dock.` channels. The
  UI filter is cosmetics, not a wall. This is the honest v1 posture — same trust level as
  every channel — and the fix (per-channel membership/ACL) belongs to the channels topic;
  the dock inherits it when it lands. Do not paper over this with a UI-only "private"
  label.
- **Prompt injection via page context.** `context` is client-supplied prompt material.
  Fencing + size cap + "untrusted" framing mitigate; the real wall stays caps (the run
  executes under the poster's captured grants).
- **EventSource token-in-query.** The existing `?token=` pattern; a dock on every page
  opens streams more often — same exposure as channels today, but re-check query-string
  log scrubbing.
- **Shell reflow vs dense pages.** A resizable right panel squeezes flows/Data Studio
  canvases. `@nube/panel`'s min-width + the resize handle mitigate; the panel must obey
  `ui-standards-scope.md` responsive rules (and auto-close below a width floor on
  mobile).
- **Two invoke doors drift.** The channel `kind:"agent"` payload and `agent.invoke` /
  `InvokeRequest` must both accept `context` or they diverge; parity is part of done.
- **`@nube/panel` CSS scoping.** Its stylesheet is scoped/token-aliased with no preflight
  by design — import `@nube/panel/style.css` per its rules or the host app's styles break
  invisibly under jsdom.

## Resolved during implementation (2026-07-05)

Three contradictions surfaced in the code; each was resolved for the long-term and the
doc updated to match (HOW-TO-CODE step 8):

1. **Session id separator is `-`, not `.`** (`dock-{user-slug}-{ulid}`). The capability
   grammar (`rust/crates/caps/src/grammar.rs`) splits a resource on **both `/` and `.`**,
   and a member's channel grant is `bus:chan/*:pub` where a single `*` matches **exactly
   one segment**. A dotted id (`dock.ada.01H…`) splits into three segments and would **not**
   match `chan/*` → the create-on-first-post would be **denied** for every ordinary member.
   A dash is not a grammar delimiter, so `dock-ada-01H…` stays one segment and the existing
   member grant covers it — no new/wider cap. The reserved-prefix, UI-only convention is
   unchanged; only the delimiter moved. (Proven by `dockId.test.ts` — the mint asserts the id
   carries no `.`/`/`; and the gateway test's create-on-post succeeds for an ordinary member.)

2. **The dock is built from `@nube/panel`'s NON-MODAL primitives (`useResizable` +
   `ResizeHandle`), not its `Panel` component.** `@nube/panel`'s `Panel` wraps a modal
   `Sheet` (Radix Dialog — overlay + focus trap), which is exactly the "Sheet overlay" this
   scope **rejected** (it blocks the page; the dock must reflow it and stay open while the
   user works). The primitives are the non-modal building blocks that exist for precisely
   this; the dock composes them in a shell flex slot beside `<Outlet/>`. `@nube/panel/style.css`
   is already imported globally (`main.tsx`), so the `--lbp-*` tokens the handle uses resolve.

3. **`PageContextProvider` takes an optional `source` override** (the decision-3 seam, made
   concrete). v1's shell passes none → the router-derived default; a test or a future feature
   passes an explicit `{ capture() }` to bypass the router. This is the "provider seam for
   later feature overrides" the scope named, realized as one prop.

No other open questions.

## Related

- `docs/scope/channels/` — the storage + agent-worker path this rides
  (`channel.post`/`history`/`list`, `agent_worker.rs`, `AgentCard`); per-channel
  membership/ACL is that topic's follow-up.
- `docs/scope/agent/agent-scope.md` — the central agent; `active-agent-wiring-scope.md`
  (the active-pick resolution the worker uses).
- `docs/scope/frontend/routing-scope.md` — URL as nav truth; the source of page context.
- `docs/scope/frontend/ui-standards-scope.md` — the shell/panel must obey it.
- Code seams: `ui/src/features/shell/StatusBar.tsx`,
  `ui/src/features/routing/RoutedShell.tsx`, `ui/src/features/routing/surface.ts`,
  `ui/src/features/channel/useChannel.ts` (`postAgent`), `MessageList.tsx`,
  `useRunFeed.ts`, `ui/src/lib/channel/run.stream.ts`, `packages/panel/` (`@nube/panel`),
  `rust/crates/host/src/channel/agent_worker.rs`,
  `rust/crates/host/src/agent/` (`dispatch.rs`, `invoke.rs`, `resolve_default.rs`),
  `rust/role/gateway/src/routes/run_stream.rs`, `routes/agent_invoke.rs`.
- Skill doc: **N/A** — no new MCP verb or route; the drivable surfaces (channels, agent)
  already belong to their topics. (If the `dock.` convention ever becomes a host-level
  concept, that scope names its skill.)
- README `§3` (rules 3, 5–7, 9), `§6.1` API shapes via `SCOPE-WRITTING.md`.
