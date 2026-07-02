# External-agent scope — the `agent.runtimes` read verb + the composer runtime picker

Status: scope (the ask) — **SHIPPED (2026-07-01).** The run-lifecycle **#5 read surface**, built as its
own slice on top of the shipped durable-detached run + wall-time supervision. Split out of
[run-lifecycle-scope.md](run-lifecycle-scope.md) (which owns the run itself); this doc owns only the
**read surface + the UI entry point** that lets a member *see and pick* a runtime instead of typing an
`@id`. Promotes to `public/external-agent/`.

The in-channel agent was **orphaned in the rendered composer**: the channel input is the
`CommandPalette` (not the retired `MessageComposer`), whose `/` menu lists only `tools.catalog` MCP
tools and whose submit does `onSendChat`/`onPostQuery`/`onCallTool` — it never built the `kind:"agent"`
payload. `/agent hey` showed "No commands match". This slice makes the agent a **first-class palette
command**, the same mechanism `federation.query` uses, and gives it a real runtime **dropdown** backed
by a new read verb.

## Goals

- **`agent.runtimes`** — a read verb listing the runtimes this node has configured + the default id, so
  the composer can render a runtime dropdown. Read-only, list-only, workspace-scoped, gated by its own
  read cap `mcp:agent.runtimes:call`. Shape: `{ default, runtimes: [ids…] }`.
- **The agent as a palette command** — a host descriptor named **`agent.invoke`** in
  `host_descriptors()`, so the `/` menu offers it. Its `input_schema` is
  `{ goal:{type:string}, runtime:{type:string, x-lb:{widget:"runtime"}} }`, `required:["goal"]`.
- **The runtime widget** — a new `x-lb:{widget:"runtime"}` hint renders a `RuntimeArg` dropdown (mirroring
  `SqlArg`), fed by `agent.runtimes`, default preselected — replacing the old typed `@id`.
- **Route to the payload path, not a tool call** — on submit, when the accepted tool is `agent.invoke`
  the palette calls a new `onSendAgent(goal, runtime)` prop (→ `useChannel.postAgent`) so it flows
  through the `kind:"agent"` payload path and renders the existing `AgentCard` (running → live feed →
  answer / `agent_error`, incl. the shipped supervision timeout). It must **not** dispatch a raw
  `agent.invoke` tool call.

## Non-goals

- The run itself — `invoke_via_runtime`, the channel agent worker, the background reactor, and wall-time
  supervision are DONE (run-lifecycle #5); this slice adds only the read surface + the UI entry.
- `agent.profile.*` write CRUD — profiles remain deploy config (run-lifecycle open question).
- Health/version per profile — the shape is **minimal: ids + default** (the resolved open question); a
  richer per-profile shape is a later addition, not this slice.

## The three locked decisions (and the alternatives rejected)

1. **A descriptor, not a special-cased `/agent` string.** The agent command is a real
   `tools.catalog` descriptor, discovered + fuzzy-matched like any other — not a hard-coded string the
   composer sniffs. *Rejected:* re-parsing chat text for `/agent` (the retired `MessageComposer` /
   `parseAgentCommand` path) — it re-introduces a host-never-parses-chat violation and a second command
   grammar the palette already owns.
2. **The catalog gates on `agent.invoke` via the descriptor NAME — zero special-casing.** `tools.catalog`
   keeps a tool only if `authorize_tool(principal, ws, <name>)` passes. Naming the descriptor
   `agent.invoke` means the run's EXISTING `mcp:agent.invoke:call` gate decides catalog visibility: a
   member who can run the agent sees the command; one who can't doesn't (absent, not greyed — no
   existence leak). *Rejected:* a new `agent.command:call` cap or an `if tool == agent` in the catalog —
   both duplicate a gate that already exists and risk drift between "can see" and "can run".
3. **The runtime arg is a real dropdown backed by `agent.runtimes` — minimal shape.** The picker reads
   `{ default, runtimes }` and preselects the default. *Rejected:* the typed `@id` (unvalidated, leaks
   nothing but helps nothing) and a health/version-rich shape (premature — ids + default is all the
   picker needs; the registry has no health signal to report yet).

## Capabilities

- **`mcp:agent.runtimes:call`** — a **distinct read cap** for the list verb (list-only, no mutation).
  Granted member-level in the dev-login bundle so the picker loads for a normal member.
- **`mcp:agent.invoke:call`** — the run's own gate; it also makes the command APPEAR (decision 2). Now
  granted member-level in the dev-login bundle (previously absent — the command couldn't appear at all).

## MCP surface

- **get/list:** `agent.runtimes {} -> { default, runtimes:[…] }` — registry-derived, no store read.

## Testing plan (rule 9 — real backends; categories per `scope/testing/testing-scope.md`)

Rust (`crates/host/tests/agent_runtimes_test.rs`, real `Node`, no mocks):
- **Read-surface unit:** default-only node → exactly `{default:"default", runtimes:["default"]}`; a node
  with an extra registered runtime lists both (sorted).
- **Capability-deny (opaque, §2.1):** no `mcp:agent.runtimes:call` → `ToolError::Denied`, no id leaked.
- **Workspace-isolation (§2.2):** a ws-B principal sees only this node's config (registry-derived — no
  cross-ws data structurally), never a ws-A record.
- **Catalog integration:** a member WITH `mcp:agent.invoke:call` sees `agent.invoke` in `tools.catalog`;
  one WITHOUT does not (absent).

UI:
- **Unit** (`RuntimeArg.test.tsx`): the widget preselects the default + lists configured ids; the
  `x-lb:{widget:"runtime"}` schema hint selects the widget.
- **Real gateway** (`CommandPalette.agent.gateway.test.tsx`, `pnpm test:gateway`, no `*.fake.ts`):
  capability-filtered command; accept → runtime dropdown (default) + goal field; submit posts a
  `kind:"agent"` item; the run is driven through the real host path and the `AgentCard` settles to an
  answer.

## Open questions

- **Per-profile health/version in the shape** — resolved for now to *minimal: ids + default*. Revisit
  when the registry gains a health signal (an external subprocess liveness probe).
- **`agent.profile.*` write CRUD** — still deferred to run-lifecycle (profiles are deploy config).
