# Session — agent tool schemas + `<think>` stripping + "Copy for AI" export

**Date:** 2026-07-05 · **Area:** agent · **Scope:** `docs/scope/agent/active-agent-wiring-scope.md`

## The ask

The user got the in-house agent (GLM-4.6) working (prior session's model-key fix) but reported it
"works very bad": asked to add widgets from a timeseries datasource, it asked *"what is your
datasource name?"* in prose and looped *"Let me try querying…"* with no tool calls, leaking
`</think>` tags. Two asks: (1) find why it behaves badly; (2) add a UI way to copy a run to paste to
an external AI for help improving the backend.

## Part 1 — why the agent behaves badly (backend)

Two defects at the model boundary (full write-up in the debugging entry):

1. **Empty tool parameter schemas.** `lb_mcp::ToolDescriptor.input_schema` (a real JSON Schema)
   existed in the catalog but was dropped by `menu.rs` (→ `AllowedTool`), had no field in the
   gateway `ToolSchema`, and `openai_compat` hardcoded `parameters:{type:object}`. Every tool was
   advertised as argument-less, so the model couldn't form a valid call and asked the user instead.
2. **`<think>` leak.** GLM inlines `<think>…</think>` in `content`; it was passed through verbatim.

### Changes
- `agent/model_access.rs` — `AllowedTool.input_schema: Option<Value>`.
- `agent/menu.rs` — carry `d.input_schema`.
- `agent/serve.rs` — routed path sets `input_schema: None` (wire tuple carries no schema yet;
  follow-up to widen `AgentInvokeRequest.tools`).
- `ai-gateway/request.rs` — `ToolSchema.parameters: Option<Value>` (dropped `Eq` — `Value` isn't `Eq`).
- `ai-gateway/bridge.rs` — map `input_schema` → `parameters`.
- `ai-gateway/providers/openai_compat.rs` — forward `parameters` (degrade to `{}` only when none) +
  apply `strip_think`.
- **New** `ai-gateway/providers/strip_think.rs` — strip `<think>…</think>` at the adapter boundary.
- ~14 test files: added `input_schema: None` to `AllowedTool` literals (mechanical).

### Tests (green)
- `openai_compat_test` — 3 new (schema reaches `function.parameters`; schemaless → `{}`; think
  stripped), real in-process HTTP body assertions.
- `strip_think` — 5 unit tests.
- **Live** (`zai_live_test.rs`, network-gated `ZAI_LIVE_KEY`): a plain turn returns "PONG" with no
  `<think>` leak, AND a turn with a schema'd `datasource_list` tool → GLM **proposed the call**
  (`calls=["datasource_list"]`) instead of prose. This is the end-to-end proof.
- Full host agent suites re-run green. `agent_routed_test::an_edge_invokes…` is a **pre-existing
  Zenoh peer-discovery flake** (`NotFound`, passes ~2/3 on rerun, unrelated to this diff — my
  `serve.rs` change only adds `input_schema: None`).

## Part 2 — "Copy for AI" export (UI)

A `DockCopyButton` in the agent dock header copies the session as markdown to the clipboard.
- **New** `agent-dock/exportTranscript.ts` — pure serializer: a context header (ws, user, persona
  focus, page surface) + each non-blank item labelled user/agent by author. Unit-tested
  (`exportTranscript.test.ts`, 4 cases).
- **New** `agent-dock/DockCopyButton.tsx` — click → `navigator.clipboard.writeText` → transient
  "Copied ✓"; disabled when empty; clipboard-denied is swallowed (nothing destructive).
- `agent-dock/AgentDock.tsx` — mount the button in the header, fed `session.items` + the resolved
  persona/surface.

UI suite green (644 passed). No new `tsc` errors in `agent-dock` (the flows/panel-builder `tsc`
errors are pre-existing, another session's in-flight work).

## Follow-ups (noted)

- Widen the routed `AgentInvokeRequest.tools` wire to carry `input_schema` so edge→hub runs get real
  schemas too (today only the local in-house path does).
- The export currently serializes the visible channel transcript + context. A richer payload (the
  per-turn tool calls + their results from the run job record) is a natural extension if the visible
  answer isn't enough to debug a run.
- Rotate the Z.AI token that was shared in-session to run the live test.
