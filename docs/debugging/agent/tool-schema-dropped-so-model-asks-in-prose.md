# The agent asked questions instead of calling tools (empty tool parameter schemas)

**Area:** agent · **Status:** resolved · **Date:** 2026-07-05
**Scope:** `docs/scope/agent/active-agent-wiring-scope.md`
**Session:** `docs/sessions/agent/agent-tool-schema-and-think-session.md`

## Symptom

The in-house agent (Z.AI GLM-4.6) completed turns but behaved badly: asked to "add a widget from my
timeseries datasource", it replied in prose — *"What is the name of your timeseries datasource?"* —
instead of calling `datasource.list`/`federation.schema`. A later run leaked raw `</think>` tags and
narrated *"Let me try querying the datasource…"* repeatedly with **no tool calls emitted**. The
widget-builder persona clearly granted the discovery tools, yet none were called.

## Root cause

**Two independent defects, both at the model boundary.**

1. **Tool parameter schemas were dropped end to end.** The catalog descriptor
   (`lb_mcp::ToolDescriptor`) carries `input_schema: Option<Value>` — a real JSON Schema with
   `properties`/`required`. But:
   - `agent/menu.rs::reachable_tools` built `AllowedTool { name, description }` and **discarded**
     `input_schema`;
   - `AllowedTool` and the gateway's `ToolSchema` had **no field** to carry it;
   - `providers/openai_compat.rs::body` hardcoded `"parameters": { "type": "object" }`.

   So every tool was advertised to the model as taking **no arguments**. A capable model, unable to
   form a valid call (it doesn't know the args), falls back to asking the user in prose — exactly the
   transcript. This is the primary "works very bad" cause.

2. **`<think>` blocks leaked.** GLM inlines chain-of-thought as `<think>…</think>` in the message
   `content`; `parse_completion` passed `content` through verbatim, so the reasoning reached the
   channel answer.

## Fix

**Schema threading** (catalog → model, one field added at each hop):
- `agent/model_access.rs` — `AllowedTool` gains `input_schema: Option<serde_json::Value>`.
- `agent/menu.rs` — carry `d.input_schema` into the `AllowedTool`.
- `ai-gateway/request.rs` — `ToolSchema` gains `parameters: Option<Value>` (dropped `Eq` from
  `ToolSchema`/`AiRequest`: `serde_json::Value` is `PartialEq` but not `Eq`; only `assert_eq!` needs
  it).
- `ai-gateway/bridge.rs` — map `AllowedTool.input_schema` → `ToolSchema.parameters`.
- `providers/openai_compat.rs` — forward `parameters`, degrading to `{type:"object"}` only when a
  tool declares no schema.

The routed edge→hub path (`agent/serve.rs`) advertises `input_schema: None` for now — its wire
`AgentInvokeRequest.tools` is a `(name, description)` tuple; widening that wire to carry the schema is
a noted follow-up. The local in-house path (the one that was broken) carries the real schema.

**`<think>` stripping**: new `providers/strip_think.rs` — strip well-formed `<think>…</think>` pairs
(case-insensitive, multiline) from `content` at the adapter boundary; an unterminated block is left
verbatim (better a visible tag than a swallowed answer). Applied in `parse_completion`.

## Regression tests

- `ai-gateway/tests/openai_compat_test.rs` (real in-process HTTP server, asserts the request BODY):
  `a_tools_real_input_schema_reaches_the_provider_parameters` (the real schema reaches
  `function.parameters`), `a_tool_with_no_schema_degrades_to_an_empty_object`,
  `a_think_block_is_stripped_from_the_answer`.
- `strip_think` unit tests (5): leading block, multiline/case-insensitive, multiple blocks,
  plain-content passthrough, unterminated-left-verbatim.
- **Live proof** (`ai-gateway/tests/zai_live_test.rs`, network-gated on `ZAI_LIVE_KEY`):
  `a_live_turn_with_a_real_tool_schema_proposes_a_call_not_prose` — against the REAL Z.AI endpoint,
  GLM-4.6 given a schema'd `datasource_list` tool **proposed the call** (`calls=["datasource_list"]`,
  content "I'll check what datasources are available…") instead of asking in prose. This is the
  end-to-end confirmation the fix changes model behavior, not just the wire.

## Lesson

A tool advertised with an empty parameter schema is worse than no tool: the model knows it *exists*
but can't call it, so it stalls into asking the user. When a rich descriptor (`input_schema`) is
funneled through a narrower intermediate type (`AllowedTool`/`ToolSchema`), the narrowing silently
drops the field that makes the feature work — thread the whole contract, and assert the provider
request body downstream (a real-HTTP capture test), not just the parsed response.
