# Flows — the debug node + debug panel (session)

- Date: 2026-07-08
- Scope: ../../scope/flows/debug-node-scope.md
- Stage: S8+ (flows shipped; this adds the Node-RED debug node + sidebar over the shipped plane)
- Status: done

## Goal

Ship the Node-RED **debug node + debug sidebar** over the shipped plane, end to end — one new
host-resolved built-in `debug` node, one new live-feed verb (`flows.debug.watch`) + its gateway SSE
route, and a canvas debug panel that renders `json`/`text`/`markdown` type-aware with auto-collapse
for long content. Hold the load-bearing v1 principle: **debug is motion only** (fire-and-forget on
the bus, no SurrealDB record — rule 3 made literal). Persistence-to-disc stays the named follow-up.

## What changed

### New built-in node (`lb-flows`)
- `crates/flows/src/builtins/observability.rs` — the `debug` descriptor (`kind = sink`, one `payload`
  in, no out, `category = "Observability"`). `config`: `label`/`format`/`collapse_bytes`/`rate_limit`.
  Exports the `DEFAULT_COLLAPSE_BYTES` (2048) / `DEFAULT_RATE_LIMIT` (50/s) constants the host +
  tests agree on.
- `crates/flows/src/builtins/mod.rs` — a new `observability` category module appended after
  `function`; the `EXPECTED` list + the `builtins_in_one_shape` count test bumped to 33, and
  `data_pack_nodes_are_envelope_transforms` narrowed to `EXPECTED[12..32]` so `debug` (a sink) is
  excluded from the transform assertion.
- `crates/flows/src/lib.rs` — re-exports the two defaults at the crate root.

### Host execution + the debug motion stream (`lb-host`)
- `crates/host/src/flows/run_debug.rs` — the per-flow subject/publish/watch trio (a near-verbatim
  copy of `watch.rs` re-seamed onto `flow/{flow_id}/debug`): `flow_debug_subject`,
  `publish_debug_event`, `debug_message`, `dropped_event`, `DebugEventSub`, `watch_flow_debug`, and
  the host-side `resolve_format` (Decision 5 — `auto` sniffs object/array/JSON-string→json,
  markdown-marked string→markdown, else text; explicit `format` authoritative). Unit-tested in-crate.
- `crates/host/src/flows/execute_node/debug.rs` — the dispatch arm. Reads `inputs["payload"]`,
  resolves the format, publishes a debug message, settles `Ok` with the payload passed through (the
  Decision-5 envelope records like any sink; the published motion is the projection, never the
  record). Houses the **publish governor** (Risk 1): a per-`(ws,flow,node)` sliding-1s-window state
  in a `OnceLock<HashMap>` (the `seed_lock` shape), admitting ≤ `rate_limit` real messages and
  flushing one `dropped: k` sentinel at window close. Unit-tested in-crate.
- `crates/host/src/flows/execute_node/mod.rs` — `debug` mod + dispatch arm wired.
- `crates/host/src/flows/mod.rs` — `run_debug` module + `watch_flow_debug`/`FlowDebugWatch`
  re-exported; `flows.debug.watch` noted as a direct SSE call (like `flows.watch`).
- `crates/host/src/lib.rs` — `watch_flow_debug`/`FlowDebugWatch` exported.

### Gateway
- `role/gateway/src/routes/flows.rs` — `flow_debug_stream` (`GET /flows/{id}/debug/stream?token=`):
  a near-verbatim copy of `flow_run_stream`, deltas-only (no snapshot — v1 motion-only), `event:
  debug` frames.
- `role/gateway/src/routes/mod.rs` + `server.rs` — route exported + mounted
  (`/flows/{id}/debug/stream`) beside the flow's other routes.
- `role/gateway/src/session/credentials.rs` — `mcp:flows.debug.watch:call` added to the dev member
  caps (member-level read of your own workspace's debug tail; the `debug` node itself needs no cap —
  it runs inside `flows.run`).

### Frontend (`ui`)
- `lib/flows/debug.stream.ts` — `openFlowDebugStream` (mirrors `flow.stream.ts`; `null` on no
  gateway/Tauri so the panel renders "unavailable" honestly).
- `lib/flows/flows.types.ts` — `DebugMessage` shape (`kind: debug|dropped`, attribution, format,
  value, `collapseBytes`, `dropped`).
- `features/flows/debug/` — one component/hook per file (FILE-LAYOUT):
  - `JsonTreeView.tsx` — `@microlink/react-json-view` (already a dep), shadcn-token theme inlined.
  - `TextView.tsx` — monospace `<pre>`, null→`"null"`.
  - `DebugValueView.tsx` — format dispatch + the **auto-collapse** (Decision 6): a `Collapsible`
    disclosure on values over `collapseBytes`; full value always on the wire.
  - `DebugMessageRow.tsx` — row chrome: type badge, label, run id, ts, copy; renders a `dropped`
    sentinel inline ("N messages dropped").
  - `useDebugStream.ts` — folds the SSE frames onto component state; pause/resume, clear,
    filter-by-node, follow, drop-oldest past `MAX_LOG = 500`.
  - `DebugPanel.tsx` — the right-side drawer (Node-RED sidebar posture); honest empty states.
- `features/flows/FlowCanvas.tsx` — a floating Bug toggle opens the drawer; the panel subscribes to
  the open flow's debug stream. Reuses `features/channel/MarkdownView` for markdown (no duplicate
  helper — rule 8).

## Decisions resolved (the scope's open questions)

1. **Workspace-wide debug aggregate** — **rejected `null` in v1** (per the scope's recommendation).
   `flows.debug.watch` requires a `flow_id`; a workspace "debug console" is a separate UX with its
   own backpressure story. The panel is per-flow.
2. **Persistence-to-disc** — **deferred**, as designed. v1 ships motion-only with no new table; the
   follow-up reuses the **series** substrate (`debug:{ws}:{flow}:{node}` via `ingest.write`), not a
   new table. The motion-only regression test (`debug_is_motion_only_no_debug_record_is_written`)
   guards the load-bearing principle.
3. **`catch`/`status`/`complete`/`link`** — **deferred to sibling scopes**, per `data-nodes` +
   `flow-context` defer-lists. Recommended as the next pack; `catch` should publish onto the **same**
   `flow_debug` subject with a `kind`-tagged variant so the panel renders errors inline without a
   second subscription.
4. **Drop policy under breach** — **sliding 1s window, `{dropped: k}` sentinel** (the scope's
   recommendation), matching `batch`'s force-release honesty. Implemented + unit-tested.

## Testing

All against real infra (`mem://` store + real bus + real caps + real `lb-jobs`), no mocks
(testing-scope §0). **Green:**

- Rust — `crates/host/tests/flows_debug_test.rs` (7): the debug node publishes motion onto the
  per-flow subject; format resolution (json/markdown/text under `auto`); **motion-only regression**
  (no `flow_debug_log` record written; the node's Decision-5 envelope did record); `flows.debug.watch`
  **cap-deny** without `mcp:flows.debug.watch:call`; **workspace-isolation** (ws-B cannot subscribe
  to ws-A's subject; the wall holds); **late-attach is deltas-only** (no replay — the honest v1
  contract); the **publish governor** throttles under load.
- Rust — `lb-flows` unit (81 incl. the bumped `builtins_in_one_shape` at 33 nodes + the
  observability descriptor tests); `run_debug` format-resolver + `execute_node/debug` governor
  unit-tested in-crate.
- Rust — `lb-role-gateway` (16) still green (the new SSE route mounts beside the existing flow routes).
- UI unit — `features/flows/debug/DebugValueView.test.tsx` (9): json/text/markdown dispatch + the
  auto-collapse (under threshold renders in full; over collapses + expands on "show more";
  `collapseBytes:0` never collapses); the row's attribution + the `dropped` sentinel.
- UI gateway — `features/flows/flowsDebug.gateway.test.ts` (2, real spawned gateway): the `debug`
  node ships in the real palette under `Observability` (sink, `payload` in, no out, the config
  defaults); a flow with a `debug` node saves + runs to a terminal settle with the node in the
  snapshot.

Commands (green):
```
cargo test -p lb-host --test flows_debug_test        # 7 passed
cargo test -p lb-flows --lib                         # 81 passed
cargo test -p lb-role-gateway                        # 16 passed
pnpm test --run src/features/flows/                  # 57 passed (incl. 9 new debug)
pnpm test:gateway --run src/features/flows/flowsDebug.gateway.test.ts   # 2 passed
pnpm exec tsc --noEmit                               # clean
```

The SSE transport itself (the `/flows/{id}/debug/stream` route + the bus subject) is proven at the
Rust layer; jsdom has no `EventSource` (the same constraint every other stream UI test documents),
so the UI gateway test proves the palette + run wire, and the unit test proves the rendering.

## Notes / pre-existing breakage on this branch

A parallel AI session is mid-migration of the `lb_auth::Claims` struct (added `constraint`/`run_id`
fields). Several pre-existing test files and the `test_gateway` harness seed were not updated to the
new shape, which blocked `cargo test --workspace` and the UI gateway harness build. I made the
trivial mechanical fix to **`role/gateway/src/bin/test_gateway_seed.rs`** (the one place that
blocked the gateway harness) so my gateway test could run; the remaining broken test files
(`crates/host/tests/{assets_skill,spine,channel_query_worker,agent_runtimes,core_skills_mcp}_test.rs`,
`crates/caps/tests/*`) are the other session's to finish — I did not touch them. No code of mine is
involved in that migration.

## Promotes / follow-ups

- Promotes to `public/flows/flows.md` (a new "debug node + panel" section appended).
- Skill doc `skills/flows-debug/SKILL.md` — the `flows.debug.watch` live-feed surface is agent-/
  API-drivable; written here from a live run (the SSE handshake + event shape + a `curl` tail).
- The `catch`/`status`/`complete`/`link` pack is the recommended next flows scope (Open Q 3).

## Rework — the debug window discoverability (post-review)

The first cut mounted the panel via a **floating `absolute bottom-4 right-4` Bug button** inside the
canvas `<section>`. That section has no `position: relative`, so the `absolute` button escaped to the
nearest positioned ancestor and was effectively invisible — the operator could not find the debug
window (the exact regression Node-RED's always-visible tab exists to prevent). Reworked to match
Node-RED's posture:

- **The Bug toggle now lives in `FlowCanvasHeader`** (the always-visible toolbar) — a `Debug` button
  in the right cluster, `variant="default"` + `aria-pressed` when open (so it reads as "active tab").
  Always reachable, never a floating escapee.
- **Auto-opens** when a flow carrying a `debug` node is opened (Node-RED shows the sidebar when
  there's something to debug); the operator can still close it from the header.
- Regression guard: `FlowCanvasHeader.test.tsx` (3) — the button mounts in the header, fires
  `onToggleDebug` on click, and reflects the open state via the close label + `aria-pressed`.

Green after the rework: `FlowCanvasHeader.test.tsx` 3, `DebugValueView.test.tsx` 9, full flows suite
57/57; `tsc` + `lint` clean.
