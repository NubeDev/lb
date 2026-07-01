# Channels in-channel agent — the `agent.runtimes` read verb + composer runtime picker (session)

- Date: 2026-07-01
- Scope: ../../scope/external-agent/agent-runtimes-scope.md (the run-lifecycle #5 read surface) ·
  ../../scope/external-agent/run-lifecycle-scope.md (#5) · ../../scope/channels/channels-agent-scope.md
- Builds on: ./channels-agent-background-session.md (durable-detached run) ·
  ./channels-agent-supervision-session.md (wall-time supervision)
- Stage: post-S10 (channels surface; the read-surface + UI-entry half of run-lifecycle #5)
- Status: done (the in-channel agent is a first-class palette command with a real runtime dropdown; the
  orphaned `/agent`-on-`MessageComposer` path is removed)

## Goal — un-orphan the in-channel agent in the rendered composer

The run path (durable enqueue → background reactor → `invoke_via_runtime` → `AgentCard`, wall-time
supervised) was DONE. But the channel input the UI actually renders is the **`CommandPalette`** (via
`ChannelView`), **not** the old `MessageComposer`. The palette's `/` menu listed only `tools.catalog`
MCP tools, and its `submit()` did `onSendChat`/`onPostQuery`/`onCallTool` — it never built the
`kind:"agent"` payload. So `/agent hey` showed "No commands match", and `useChannel.postAgent` +
`parseAgentCommand` existed but nothing rendered called them. This session wires the agent into the
palette as a real command and gives it a runtime **dropdown** (a read verb) instead of a typed `@id`.

Exit gate: `/` in a channel shows the agent command for a member with `mcp:agent.invoke:call`; accepting
it renders a runtime dropdown (default preselected; external profiles listed when present) + a goal
field; submit posts `kind:"agent"` and the `AgentCard` runs live → answer (or the supervised
`agent_error`). The dead `parseAgentCommand` / `MessageComposer` path is removed.

## The three locked decisions (and the alternatives rejected)

1. **A descriptor, not a special-cased `/agent` string.** The agent command is a real `tools.catalog`
   descriptor, fuzzy-matched like `federation.query` — not a string the composer sniffs.
   *Rejected:* resurrecting `MessageComposer` / `parseAgentCommand` (re-parsing chat text) — a second
   command grammar and a host-parses-chat smell the palette already obsoletes.
2. **The catalog gates on the descriptor NAME `agent.invoke` — zero special-casing.** `tools.catalog`
   keeps a tool only if `authorize_tool(principal, ws, <name>)` passes, so naming the descriptor
   `agent.invoke` reuses the run's existing `mcp:agent.invoke:call` gate to decide visibility (absent,
   not greyed — no existence leak). *Rejected:* a new `agent.command:call` cap or an `if` in the catalog
   — both duplicate a gate that already exists and can drift from "can run".
3. **`agent.runtimes` is minimal: ids + default.** The picker reads `{ default, runtimes }` and
   preselects the default. *Rejected:* the typed `@id` (helps nothing) and a health/version-rich shape
   (premature — the registry has no health signal to report; ids + default is all the dropdown needs).

## What shipped

**Rust (host):**
- `crates/host/src/agent/runtimes.rs` — `list_runtimes(node, principal, ws)`: gate
  `authorize_tool(…, "agent.runtimes")` → opaque `Denied`; return `{ default, runtimes:[sorted ids] }`
  from `node.runtimes()`. Read-only, ws-scoped, list-only (registry-derived — no store read, so no
  cross-ws data structurally). Added `RuntimeRegistry::default_id()`.
- `crates/host/src/agent/tool.rs` — the `"agent.runtimes" => list_runtimes(…)` dispatch arm (routing
  through the existing `agent.` branch — no routing change).
- `crates/host/src/agent/descriptor.rs` — `invoke_descriptor()` named `agent.invoke`, schema per
  decision 3, with the catalog-gates-on-the-name rationale (decision 2) in the file docs. Collected into
  `tools::host_descriptors()`.
- `role/gateway/src/session/credentials.rs` — granted `mcp:agent.invoke:call` (makes the command appear
  AND runs it) + the distinct read cap `mcp:agent.runtimes:call` (loads the picker), member-level.

**UI:**
- `lib/agent/runtimes.api.ts` — `agentRuntimes()` (`mcp_call` → `agent.runtimes`) → `{ default, runtimes }`.
- `features/channel/palette/argWidgets/RuntimeArg.tsx` + `useRuntimes.ts` — the dropdown (mirroring
  `SqlArg` + its `useSqlSchema`), default preselected.
- `lib/channel/palette.types.ts` — added the `"runtime"` `WidgetKind`.
- `features/channel/palette/CommandPalette.tsx` — the runtime widget render; a general **text-arg** input
  (the palette previously had no way to fill a plain text arg like the agent's `goal`); and the
  `agent.invoke` → `onSendAgent(goal, runtime)` submit route (NOT a raw tool call).
- `ChannelView.tsx` — threads `onSendAgent={postAgent}`.

**Removed the orphaned path:** deleted `parseAgentCommand` (+ its test) from `useChannel.ts` and the
unrendered `MessageComposer.tsx` (grep proved no importer). `useChannel.send` is now plain chat only.

## Tests (rule 9 — real backends, no `*.fake.ts`)

- Rust `agent_runtimes_test.rs` (real `Node`): read-surface unit (default-only + extra-runtime),
  capability-deny (opaque), workspace-isolation, catalog-integration (agent.invoke present iff the invoke
  cap is held). `cargo test -p lb-host --test agent_runtimes_test` → 5 green;
  `--test tools_catalog_test` → 4 green.
- UI unit `RuntimeArg.test.tsx` → 3 green (default preselected, options, the schema hint). `pnpm test`
  → 272 green.
- UI real gateway `CommandPalette.agent.gateway.test.tsx` → 2 green (capability-filtered command; accept
  → runtime dropdown + goal → submit posts `kind:"agent"` → real `drain_channel_agent_runs` drives it →
  `AgentCard` settles to the answer). Added a `/_seed/agent_drain` test-gateway route calling the real
  `drain_channel_agent_runs` (the reactor's own function — the test gateway doesn't spawn the timer),
  plus the `drainAgentRuns()` session helper. Existing channel gateway suites still green (11).

## Notes

- **The text-arg widget was a real gap.** The palette had entity + sql + (now) runtime widgets but no
  plain-text fill path, so a required text arg like `goal` was unfillable. Added a minimal general text
  input (⏎ commits / submits when it's the last arg), which benefits any future plain-text arg — not an
  agent special case.
- **`mcp:agent.invoke:call` was not in the dev member bundle** before this slice, so the agent command
  couldn't appear for a normal member at all. Granting it (alongside the distinct read cap) is what makes
  the command visible end to end.
