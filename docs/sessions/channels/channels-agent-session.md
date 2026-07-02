# Channels — in-channel agent (ask an agent, get an answer, in the channel) (session)

- Date: 2026-07-01
- Scope: ../../scope/channels/channels-agent-scope.md
- Also touches: ../../scope/external-agent/ (runtime-seam #1, model-routing groundwork)
- Stage: post-S10 (channels surface; builds on the shipped `agent.invoke`/`AgentRuntime` seam)
- Status: done (v1 — inline worker; external agent driven for real against Z.AI GLM-4.6)

## Goal

Let a channel member **ask an agent in a channel** and get the answer back in the same channel,
durably — the sibling of the shipped in-channel query worker. The agent behind it is selected through
the shipped `AgentRuntime` seam by a `runtime` field: the in-house default, or an **external** ACP
agent (Open Interpreter) once it can actually reach a model. Exit gate: post a `kind:"agent"` item →
a `kind:"agent_result"` (or `agent_error`) item appears in history, with the invoke gate checked
host-side and the deny path opaque; and the SAME channel path drives the **real** external agent
(Open Interpreter → Z.AI GLM-4.6) end to end.

## What changed

### External-agent: made the codex wrapper actually reach Z.AI GLM-4.6 (the missing provider config)

The shipped `CodexWrapper` (`crates/external-agent/src/wrappers/codex.rs`) built
`interpreter exec --json -m glm-4.6 <goal>` but emitted **no provider config**, so a run couldn't reach
Z.AI (codex defaults to its own login + the OpenAI *Responses* API). Reproduced the working invocation
by hand, then baked it in:

- `ModelEndpoint` gained `base_url: Option<String>` (`crates/external-agent/src/profile.rs`). When set,
  the codex wrapper emits the `model_providers` `-c` overrides:
  `name`, `base_url`, `env_key`, **`wire_api=chat`**, and `-c model_provider=<id>`. `wire_api=chat` is
  the load-bearing fix — codex defaults to `/responses` (404 on Z.AI); Z.AI (and, later, our gateway)
  speak Chat Completions.
- The default profile endpoint (`role/external-agent/src/profiles.rs`) now uses provider id
  **`zaicoding`** (deliberately NOT codex's built-in `zai`, which points at the throttled standard
  endpoint + wants `ZHIPU_API_KEY`) and `base_url = https://api.z.ai/api/coding/paas/v4`. The key is
  read from `ZAI_API_KEY` — the wrapper passes the env var **name**, never a value.
- Verified live: `EXTAGENT_SMOKE=1 ZAI_API_KEY=… cargo test -p lb-role-external-agent --test smoke_test`
  → `external agent (open-interpreter-default) answered: "PONG"`.

### Runtime registry is now part of the `Node` spine

The `RuntimeRegistry` previously lived only at the wiring layer (handed to `serve_agent`), so a host
service with just `&Node` couldn't reach it. Added `Node.runtimes: Mutex<Arc<RuntimeRegistry>>`
(`crates/host/src/boot.rs`) with `node.runtimes()` (clone the `Arc` out; never hold the lock across a
run) and `node.install_runtimes(reg)` (the binary swaps in the external-enabled registry after boot).
Boot installs a **default-only** registry over a new `UnconfiguredModel`
(`crates/host/src/agent/unconfigured.rs`) — the honest empty state of the "in-house model provider not
wired yet" gap (STATUS): the `default` runtime is present (so `resolve` holds and external runtimes are
selectable) but returns a clear "no in-house model configured" answer rather than pretending to run.
Added `Node::boot_on_bus` and routed the three direct-`Node{…}` test helpers
(`cross_node_routing`, `offline_sync`, `ext_publish`) through host constructors (the field stays
encapsulated).

### Node binary installs the external runtimes when the feature is on

`node/src/external_agent.rs` gained `install(&node)`: **feature-ON** it builds a registry with the
in-house `default` (over `UnconfiguredModel`) + the external `AcpRuntime` entries (Open Interpreter
default, VT Code, Codex) via the existing `register` hook, then `node.install_runtimes(…)`. **Feature-OFF**
it is a no-op. Called once from `main.rs` right after boot. This is the one place the feature changes
node behaviour (rule 1). So a `cargo build -p node --features external-agent` node has
`runtime:"open-interpreter-default"` reachable from a channel.

### Kind-tagged agent payloads (Rust + TS, mirrored 1:1)

`crates/host/src/channel/payload.rs` gained three kinds alongside the query ones (same additive
envelope, no `Item` migration): `agent` `{ goal, runtime?, job }`, `agent_result`
`{ goal, runtime, job, answer, truncated? }`, `agent_error` `{ goal, error }`, plus `agent_result_body`
/ `agent_error_body`. Mirrored in `ui/src/lib/channel/payload.types.ts` with `encodeAgent` + `newRunId`.

### Inline agent worker (sibling of the query worker)

`crates/host/src/channel/agent_worker.rs` — `run_if_agent`, hooked in `post.rs` right after
`run_if_query`. On a `kind:"agent"` item it resolves the runtime from `node.runtimes()` and drives the
run via `invoke_via_runtime` **under the poster's principal** (`agent_caps = poster.caps()`, so the run
can do exactly what the asker is granted), then posts `agent_result` / `agent_error` back under
`system:agent-worker` (id `a:<job>`, correlated to the run) via the shared `deliver`.

- **Re-entrancy guard:** only `kind:"agent"` triggers; the worker's own result/error items parse to a
  different variant and return early (tested).
- **Opaque deny:** a missing `mcp:agent.invoke:call` grant AND a named-but-unknown runtime both collapse
  to "agent not permitted" — no capability/runtime-existence leak. A genuine run fault is honest.
- **Answer cap:** 256 KB, `truncated` flag (mirrors the query worker), char-boundary safe.

### UI: AgentCard + `/agent` composer command

- `ui/src/features/channel/AgentCard.tsx` renders the three kinds (running chip → answer → opaque
  error); `MessageItem` routes agent kinds to it (QueryCard guards to only `query_result`).
- `MessageList` computes the set of settled run ids (items whose id is `a:<job>`) and passes it down so
  a completed run's `agent` request hides its "running…" placeholder (no stuck spinner).
- `useChannel` gained `postAgent` + `parseAgentCommand`: typing `/agent [@runtime] <goal>` in the
  composer builds the structured payload (the UI mints the run id via `newRunId`); the host never parses
  chat text. `@open-interpreter-default` selects the external agent; omit it for the in-house default.

> **SUPERSEDED (2026-07-01, agent-runtimes-picker-session.md).** The rendered composer is the
> `CommandPalette` (via `ChannelView`), **not** `MessageComposer` — so the `/agent [@runtime] <goal>`
> chat-string path above was orphaned (`parseAgentCommand` was never reached from the rendered input).
> The agent is now a **first-class palette command** (`agent.invoke`, gated by `mcp:agent.invoke:call`
> via the catalog's per-tool `authorize_tool`), and the `@runtime` is a real **dropdown** backed by the
> `agent.runtimes` read verb. `parseAgentCommand` + `MessageComposer` are DELETED. `postAgent` stays —
> it's the payload path the palette now routes `agent.invoke` to (via `onSendAgent`).

## Design decisions (scope open questions, resolved)

- **Inline worker, not a background job (v1).** Faithful reuse of the query worker's proven
  `post → inline worker` model; works end to end today. Awaiting a long run blocks the poster's `post`
  for its duration — accepted for v1 and documented. **Non-blocking, supervised, resumable background
  execution is the run-lifecycle #5 follow-up** (the request item is published BEFORE the worker runs,
  so a watcher already sees the run start live).
- **The UI mints `job`** (like `AgentView`'s `jobId`) so a client can subscribe to the run stream the
  instant the request lands.
- **Runtime = the seam, not a branch.** The worker passes `runtime` straight into the registry; the
  in-house default works today, the external agent slots in via the same field once installed. No
  channel-side change to swap agents.
- **In-channel per-tool Allow/Deny card:** excluded from v1 (relies on `agent_policy` only) — fast-follow.

## Live run-feed (the "both" streaming half — shipped in this session's second pass)

The card now **watches the agent work in real time**, not just the final answer:

- **Driver streams per-line.** `crates/external-agent/src/driver.rs` `drive` gained an optional
  `sink: Option<&UnboundedSender<RunEvent>>` — it taps each `RunEvent` to the sink **the moment its
  stdout line decodes**, instead of the old collect-then-burst. `AcpRuntime::run`
  (`role/external-agent/src/lib.rs`) passes a channel + a detached publisher task that forwards each
  event onto the ws-walled run subject via `publish_run_event` live. (The collected `Vec` is still
  returned for the answer; the sink is purely additive — `drive(.., None)` is unchanged.)
- **Failure reason surfaced.** On a failed run the `agent_error` now carries the terminal
  `RunFinish.answer` (e.g. a provider `429 Too Many Requests` or a tool error) via `finish_message`,
  instead of the empty `TextDelta` join.
- **UI run-feed.** `ui/src/lib/channel/run.stream.ts` (`openRunStream` — SSE client for
  `GET /runs/{job}/stream?token=`, mirrors `channel.stream.ts`; `RunEvent` TS type mirrors the Rust
  `#[serde(tag="type", kebab-case)]`), `useRunFeed` (folds events into `{text, reasoning, tools[],
  finished}`), and `AgentCard`'s `RunningCard` renders live tool-call rows (spinner → ✓/✗), streamed
  reasoning, and text while the run is pending (until the durable `agent_result` supersedes it).
- **`postAgent` is non-blocking.** It fires the request and does NOT await the (long, inline) run — the
  request item, live feed, and durable answer all arrive over SSE; the composer stays responsive. (It
  never *aborts* the fetch, which would cancel the server-side run.) Caveat: the inline run is still
  tied to the held POST connection, so **closing the tab mid-run cancels it** — true background/
  supervised execution remains the run-lifecycle #5 follow-up.

Auth note: the run stream requires `mcp:agent.watch:call` on the session token (checked by `watch_run`,
ws-walled), same as any agent-run watcher.

## Not built here (named, linked TODO — not faked)

- **Background/supervised execution** so the inline run detaches from the POST connection (survives tab
  close) — run-lifecycle #5.
- **External-agent #3 capability-wall** (built-ins-off + OS sandbox + MCP-only tools): reaching an
  external subprocess is dev-only until #3 ships. The worker fails closed to the in-house default when
  an external runtime isn't installed. **Do not run untrusted external agents in production before #3.**
- **External-agent #4 model-routing** (agent's model via OUR gateway + scoped token): today the external
  agent reaches Z.AI directly via `ZAI_API_KEY`. The `base_url` seam is exactly where #4 repoints it at
  the gateway's OpenAI-compatible endpoint.

## Tests (rule 9 — real store/bus/loop/channel; only the model provider HTTP is ever stubbed)

- **Rust unit:** `payload.rs` agent-kind round-trips (3); `agent_worker.rs` answer cap (2);
  `codex.rs` provider-override emission with/without `base_url` (2).
- **Rust integration** `crates/host/tests/channel_agent_worker_test.rs` (5, real in-house loop over
  `MockProvider`): happy path (`agent_result` with the answer, `a:<job>` id), opaque capability-deny,
  opaque unknown-runtime, re-entrancy, workspace-isolation.
- **Rust live e2e** `role/external-agent/tests/channel_smoke_test.rs` (opt-in, `EXTAGENT_SMOKE=1`):
  channel `post` → agent worker → seam → Open Interpreter → **Z.AI GLM-4.6** → `agent_result`. Ran green:
  `in-channel external agent answered: "PONG"`.
- **UI unit:** `payload.test.ts` agent-kind parse/encode (+); `parseAgentCommand.test.ts` (5);
  `useRunFeed.test.ts` fold reducer (6 — text accumulation, tool-call in-place update, finish fallback).
- `cargo fmt` clean; the affected Rust suites (`lb-host` + `lb-external-agent` + `lb-role-external-agent`,
  44 test binaries) pass; feature-OFF and feature-ON node builds both clean; UI channel suite 42 green.

> **Note:** a concurrent in-progress crate `role/cli` (untracked) landed in the root `Cargo.toml` as a
> workspace member without declaring `lb-role-gateway` in `[workspace.dependencies]`, which breaks
> workspace-wide cargo resolution — so the live Z.AI re-run of the streaming path is blocked until that
> is completed. The full path was proven `"PONG"` earlier this session (before that edit), and all
> non-live tests + the driver streaming unit path pass.

## How to run the real thing

```
# 1. build the node with the external agent compiled in
cargo build -p node --features external-agent
# 2. give it the Z.AI key (name only ever lives in the profile; value in the env)
export ZAI_API_KEY=…            # the coding-plan key (NOT ZHIPU_API_KEY)
# 3. in a channel, ask the external agent via the `/` command palette:
#    `/` → pick the agent command → type the goal → pick `open-interpreter-default` in the runtime
#    dropdown (or leave the default) → send. (Superseded the `/agent @… <goal>` chat string — see the
#    SUPERSEDED note above; the runtime is now a real dropdown, not a typed `@id`.)
```

## Follow-ups

1. Background/supervised execution so the run detaches from the POST connection (survives tab close) —
   run-lifecycle #5.
2. External-agent #3 capability-wall before any non-dev external run.
3. ~~`agent.runtimes` read verb (#5) → a runtime picker in the composer instead of a typed `@id`.~~
   **DONE (2026-07-01)** — agent-runtimes-picker-session.md (the `agent.invoke` palette command +
   `RuntimeArg` dropdown backed by `agent.runtimes`).
4. Move the in-house `run_session` onto the same live-tap so its per-token deltas stream too (today the
   in-house loop publishes per-step; the external agent now streams per-line).
