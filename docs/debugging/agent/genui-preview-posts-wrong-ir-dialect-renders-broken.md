# agent — a posted GenUI widget preview rendered as the invalid/draft state in the dock (wrong IR dialect, nothing rejected it)

**Date:** 2026-07-06 · **Area:** agent / channels / genui · **Status:** fixed

## Symptom

From the agent dock, "use this query to build a genui widget…" ran a healthy session — `dashboard.catalog` → `federation.query` (SQL proven) → `channel.post` all ✓ — but the posted widget showed `GenUiView`'s invalid/draft state instead of the composed card. The preview is the whole point of the flow (`docs/scope/channels/channel-widgets-scope.md`); a broken render defeats it.

## Root cause

The model emitted a **wrong IR dialect** in `options.genui.ir` — `{"components":{"root":{"type":"stack"}}}`:
`type` instead of `component`, no per-component `id`, no `ir.v`, no `surface:{surfaceId,root}` — and
**nothing validated a `rich_result` at post time**. `dashboard.save` runs `check_genui_cells` and its
loud `BadInput` had visibly let the model self-correct in an earlier run, but a conversation preview
never touches the save path, so the malformed body landed silently and the dock rendered the draft state.

## Fix

1. `rust/crates/host/src/channel/genui_check.rs` (new): `check_rich_result_genui` runs on every
   `channel::post` (WS/HTTP/MCP all funnel through it), after authorize, before persist. A
   `kind:"rich_result"` + `view:"genui"` body must carry a structurally valid `options.genui` block —
   validated by the SAME checker as the save path. Chat and non-genui payloads pass untouched. A
   posted preview with **no** IR is an error (unlike a savable dashboard draft).
2. `rust/crates/host/src/dashboard/genui.rs`: the cell check's core extracted as
   `check_genui_block` (one validator, two seams). Tightened to mirror the TS validator: per-component
   `id` must repeat the map key (`id-mismatch` was already a blocking error in `@nube/genui`
   `validate.ts`), and error messages now name the fix (`component` not `type`; the
   `surface:{surfaceId,root}` shape) because on the channel path they ARE the agent-loop feedback.
3. `ChannelError::BadInput` (new variant) → `ToolError::BadInput` over MCP (the loud feedback the
   model self-corrects from) and HTTP 400 (was a blanket 403) in `role/gateway/src/routes/post.rs`.
4. `docs/skills/channel-widgets/SKILL.md`: "Common IR mistakes" block naming each dialect slip.

## Regression tests

- `channel::genui_check` units (5): the exact live wrong-dialect body rejected with the fix named;
  valid IR passes; chat/non-genui untouched; missing block/IR rejected; missing surface names the shape.
- `channel_agent_worker_test::a_malformed_genui_rich_result_is_rejected_and_the_corrected_repost_lands`:
  a real run posts the wrong dialect (rejected, fed back), reposts corrected (lands) — exactly one
  genui item in history, the valid one; the run still answers.

## Lesson

Every write seam a renderer trusts needs the validator, not just the "main" one — a preview path that
skips the save path skips its gate too. And a validation error at an AI seam must name the fix, not
just the defect: the message is training signal for the very next turn.

## Round 2 (same day): the loop stalled on a stringified IR

Live retest: the gate fired correctly but the model sent `options.genui.ir` as a JSON-ENCODED
STRING three turns straight — "must be an object" wasn't actionable feedback. Fix: lenient-args
normalization (`dashboard/genui.rs::normalize_genui_block`) at both write seams — a string `ir`
that parses to an object is rewritten to the object (the renderer then sees the real IR); an
unparseable one gets "not a string — pass the IR inline, unquoted". Regression: `genui_check`
string-ir units + `dashboard_genui_test::allows_an_unauthored_draft` case (d). The whole path is
now pinned end-to-end by `ui/e2e/channel-genui-preview.spec.ts` (real node + built shell + real
browser: rejection message, normalization round-trip, and the composed surface actually rendering).

## Round 3 (same day): first-failure errors are a five-turn tax

Live retest converged — the widget posted and rendered — but took 5 rejected `channel.post` turns
because the validator returned one defect per call. Fix: collect ALL independent defects into one
message and append a minimal complete valid IR template (`IR_TEMPLATE` in `dashboard/genui.rs`).
Lesson: at an AI seam, validate like a compiler (all diagnostics at once + a working example), not
like a guard clause — each first-failure return is a full model turn.

## Round 4 (2026-07-07): the verb itself had no arg schema

Next live run never reached the IR at all — 13 × `missing arg: cid` to the turn ceiling, because
`channel.post` was a name-only catalog row. Fix: a real `post_descriptor` (cid described as "given
in your goal as `[conversation channel: <cid>]`"), `validate_args` misses now carry the arg's
`x-lb.description`, and `channel` is accepted as a cid alias. Lesson (third occurrence — now a
rule): EVERY write verb an AI persona leans on ships a descriptor WITH per-arg descriptions the
error path re-uses; a name-only tool row is a trap that costs a whole run.

## Round 5 (2026-07-07): broken JSON slipped through the gate AS CHAT

The next live run converged in ONE retry (the multi-defect error worked) and `channel.post` ✓ —
but the dock showed raw JSON. Stored body inspection: the model emitted the envelope with ONE
missing closing brace; invalid JSON hit the gate's "not JSON → chat" tolerance, landed as plain
text, and the UI's `parsePayload` (equally tolerant, mirrored by design) rendered it raw. Fix: a
`{`-leading body naming `"kind"` that fails JSON parse is now rejected with the parser's position
("body is not valid JSON (EOF … column 2239) — check for an unbalanced brace/bracket"); genuine
chat is untouched. Live-verified: the exact stored body → 403 with the position; the same body
with the brace restored → lands (the widget was otherwise valid). Lesson: a tolerant "not a
payload → chat" fallback and an AI author are a trap pair — anything that LOOKS like an attempted
payload must fail loudly, because the author cannot see what the renderer sees.
