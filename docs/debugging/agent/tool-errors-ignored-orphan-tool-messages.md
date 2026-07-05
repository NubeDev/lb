# Agent blindly retried the identical rejected tool call — tool results were orphan `role:"tool"` messages

**Area:** agent (run loop ↔ ai-gateway wire shape)
**Date:** 2026-07-05
**Symptom:** Live (GLM-4.6, widget-builder persona), the model issued the *same* rejected
`federation.query` three turns in a row, ignoring the rejection's steering text; earlier it had
retried an identically-malformed `dashboard.save` five turns straight. The errors were being fed
back — and having no visible effect.

## Root cause

The OpenAI-compat adapter folded tool outcomes into the request as **orphan** messages:

- prior-turn results went out as `role:"tool"` messages with a `tool_call_id` but **no preceding
  assistant message carrying the matching `tool_calls`** (the shape the spec requires and the model
  was trained on);
- older turns' outcomes sat in history as a combined `role:"tool"` summary with no id at all.

Measured live against Z.AI with three candidate shapes: with the conformant shape (assistant
`tool_calls` echo → keyed `role:"tool"` result) the model kept full call context (queried the right
source); with the orphan shapes it lost the thread and guessed datasource names. An orphan tool
message is half-ignored — the model neither trusts nor anchors it.

## Fix

- `CallOutcome` (host `model_access.rs`) and the gateway `ToolResult` now carry the
  originally-proposed call's `name` + `input` (filled at every construction site; the rehydrate
  fold pairs them from `ToolCallProposed` transcript events).
- `openai_compat::body()` emits the CONFORMANT shape for prior results: one assistant message
  echoing the proposed `tool_calls`, then a `role:"tool"` result keyed to each id. A result without
  a name (legacy caller) degrades to the old orphan message rather than fabricating a call.
- History `role:"tool"` summaries are re-rolled as plain user text (`[tool results]\n…`) — wire-valid
  and faithfully carried.

## Regression tests

`rust/role/ai-gateway/tests/openai_compat_test.rs::prior_results_are_folded_in_the_conformant_tool_call_shape`
(real-HTTP body assert). Host suites (`agent_test`, `agent_rehydrate_test`, `agent_skill_test`,
`agent_decision_test`, …) pin the widened `CallOutcome` plumbing.

**Verified live:** the next widget-builder run led `datasource.list → federation.schema → …` and
recovered from every tool error within a turn instead of repeating it.
