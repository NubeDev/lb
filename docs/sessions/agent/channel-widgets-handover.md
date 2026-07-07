# HANDOVER — GenUI widget previews in the agent dock (channel-widgets)

**Date:** 2026-07-07 · **Branch:** `insights-v1` (Rust/e2e work committed in `8c30f43` + later
uncommitted doc/code — check `git status`) · **Test workspace:** `acme`, user `user:ada` ·
**Persona:** `builtin.widget-builder` · **Prev handover:** superseded by this file.

## The goal (unchanged)

From the agent dock: ask a data question → the agent posts a **live GenUI widget preview into the
dock conversation** (`rich_result` channel item) — rendered, not raw — pin-able to a dashboard.
Preview must NEVER `dashboard.save`.

## CURRENT STATE — one bug left (again), everything else fixed & proven

**The user's live symptom right now:** the agent's `channel.post` succeeds ("its still just
sending it on the channel"), but **NO rendered preview appears in the agent-dock panel**.

**What is PROVEN to work** (don't re-litigate):
- The dock's render path works: `ui/e2e/agent-dock-genui-preview.spec.ts` (1/1 green) posts a
  genui rich_result via the real gateway into a dock-prefixed session channel
  (`dock-user-ada-e2egenui01`), opens the dock in Chromium, selects the session in the picker, and
  the composed surface renders INSIDE the dock (screenshot
  `ui/e2e/__screenshots__/agent-dock-genui-preview.png`). Channels-surface twin:
  `channel-genui-preview.spec.ts` (3/3 green).
- The host gate chain works and is live-verified — see "Shipped fixes" below.

**Prime hypotheses for the remaining bug (investigate in this order):**
1. **The post lands in the WRONG channel.** The run goal ends with
   `[conversation channel: <cid>]` (`channel/agent_worker.rs`), but the model may post to another
   cid (one it saw via `channel.list`, or the page-context channel — the user was on the
   `datasources` surface). CHECK FIRST: `channel.list` + `channel.history` over `/mcp/call` to
   find WHERE the ✓'d rich_result actually landed vs the dock session's own `dock-user-ada-…` id
   (the dock picker shows the current session id). If mismatched → make the dock cid unmissable
   (e.g. repeat it in the skill/persona text, or have the worker inject/force `cid` for
   `channel.post` when the model omits/mangles it — the worker KNOWS the cid).
2. **The dock doesn't live-refresh on another author's post.** The e2e SELECTS the session after
   the item exists (history read). If the user keeps the dock open during the run, the new item
   must arrive via the live subscribe path (`useDockSession` → subscribe/SSE). Check whether
   `useDockSession` merges live bus items posted by `agent:session` while a run is active — write
   a gateway/e2e test: open dock on a session, THEN post the rich_result via API, assert it
   appears WITHOUT re-selecting the session.
3. Body stored ≠ body sent (already had one of these — see round 5). Verify with the
   history-inspection snippet below.

**Debug snippet — find where the widget actually landed** (adapt cid):
```bash
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login -H 'content-type: application/json' \
  -d '{"user":"user:ada","workspace":"acme"}' | python3 -c 'import json,sys;print(json.load(sys.stdin)["token"])')
# list channels (incl. dock-… sessions), then per-channel:
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{"tool":"channel.history","args":{"cid":"<cid>"}}'
# for each rich_result item: json.loads(body) must succeed and kind/view/options.genui.ir be sane.
```

## Shipped fixes (all live-verified on the node, rounds 1–5, 2026-07-06→07)

Failure ladder as observed live, each rung closed host-side:
1. **Wrong IR dialect** (`type` vs `component`, no ids, no `v`, no `surface`) rendered broken →
   `channel/genui_check.rs` gates every `channel::post` (after authorize, before persist) with the
   SAME validator as `dashboard.save`, extracted as `dashboard/genui.rs::check_genui_block`.
2. **One-defect-per-turn errors** burned 5 retries → validator now collects ALL defects into one
   message + appends a minimal valid IR template (`IR_TEMPLATE`).
3. **Stringified `ir`** stalled the loop → `normalize_genui_block` parses a JSON-string `ir` to
   the object at BOTH seams (channel post rewrites the body; `dashboard_save` pre-validation).
4. **`channel.post` had NO arg schema** → 13×`missing arg: cid` burned a whole run →
   `channel/tool.rs::post_descriptor` ({cid,id,ts,body}, cid described as "given in your goal as
   `[conversation channel: <cid>]`"), registered in `tools/descriptor.rs::host_descriptors`;
   `validate_args` misses now append the arg's own `x-lb.description`; `channel` accepted as cid
   alias; handler cid/id misses steer.
5. **Missing ONE closing brace** → invalid JSON slipped the gate's chat-tolerance, landed as chat,
   dock showed raw JSON → a `{`-leading body naming `"kind"` that fails JSON parse is now a loud
   `BadInput` with the parser's position; real chat unaffected.

Plumbing: `ChannelError::BadInput` (new) → `ToolError::BadInput` over MCP; HTTP 400 in
`role/gateway/src/routes/post.rs`. Skill `docs/skills/channel-widgets/SKILL.md` gained a "Common
IR mistakes" block (object-not-string ir, component/id/v/surface, action-less controls).

## Tests (all green; run after any change)

- `cd rust && cargo test -p lb-host --lib genui` — 8 gate units (incl. missing-brace, string-ir).
- `cargo test -p lb-host --test channel_agent_worker_test` — 11 (incl.
  `a_malformed_genui_rich_result_is_rejected_and_the_corrected_repost_lands`).
- `cargo test -p lb-host --test dashboard_genui_test` (8) / `dashboard_test` (10) /
  `widget_pin_test` (11) / messaging suites / `cargo test -p lb-role-gateway` (build
  `cargo build -p echo-sidecar` first or native_call_routes_test reds).
- E2E (needs node on :8080 = `make dev`, built shell on :4173 = `make ui-preview`):
  `cd ui && npx playwright test e2e/channel-genui-preview.spec.ts e2e/agent-dock-genui-preview.spec.ts`.

## Live-environment gotchas (bit us repeatedly)

- **The node NEVER hot-reloads.** After ANY Rust change: `cargo build -p node && make kill && make dev`.
- The user often restarts the node themselves — before debugging "it didn't change", verify the
  running binary: probe an endpoint for the new behavior, and `ls -l /proc/<pid>/exe` (no
  `(deleted)`).
- `make ui-preview` (port 4173) is only needed for Playwright; the user's dev shell is :5173.
- Transcript exports ("Copy for AI") show the run-feed's FULL tool inputs — the STORED channel
  item can differ (round 5). Always inspect `channel.history` for ground truth.
- Old dock sessions carry earlier broken items (raw-JSON chat) — they render raw forever; test in
  a NEW session.

## Key files

- Gate: `rust/crates/host/src/channel/genui_check.rs` · shared validator + template:
  `rust/crates/host/src/dashboard/genui.rs` · post path: `rust/crates/host/src/channel/post.rs`
- Descriptor: `rust/crates/host/src/channel/tool.rs::post_descriptor` + collector
  `rust/crates/host/src/tools/descriptor.rs` (enriched `validate_args`)
- Goal cid line: `rust/crates/host/src/channel/agent_worker.rs`
- Dock UI: `ui/src/features/agent-dock/AgentDock.tsx` (mounts shared `MessageList`),
  `useDockSession.ts` (items + live refresh — hypothesis 2 lives here), `dockId.ts` (id grammar)
- Render: `MessageItem.tsx` (`parsePayload` null → raw text fallback) → `ResponseView` →
  `WidgetView` → `packages/genui` (`GenUiView`)
- E2E: `ui/e2e/agent-dock-genui-preview.spec.ts`, `ui/e2e/channel-genui-preview.spec.ts`
- Debug history (rounds 1–5, full detail):
  `docs/debugging/agent/genui-preview-posts-wrong-ir-dialect-renders-broken.md`
- Session log: `docs/sessions/agent/channel-widgets-session.md` · Scope:
  `docs/scope/channels/channel-widgets-scope.md`

## Recommended next step (concrete)

Run the debug snippet against the user's LATEST run: find the ✓'d rich_result item, note its
channel id, compare with the dock session id the user had open. Then either (a) fix targeting —
strongest option: in `agent_worker`/dispatch, **force/inject the run's own cid** into a
`channel.post` whose cid ≠ the conversation channel (or at least warn in the tool result), since
the worker owns the ground truth; or (b) fix dock live-refresh per hypothesis 2 with a
subscribe-while-open e2e. Then a fresh dock session live retest: "make me a genui widget and show
me it here".
