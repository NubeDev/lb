# Rules — `messaging` verbs (inbox/outbox/channel rhai handles) — session

- Date: 2026-07-04
- Scope: ../../scope/rules/rules-messaging-scope.md
- Stage: S8 — data plane (rules on the real workspace). See STATUS.md.
- Status: DONE — all three slices + offline/sync + the Playground gateway test are green.

## Goal
Let a rule body drive the inbox, outbox, and channel planes **explicitly** — read/write/resolve them
from inside the sandbox — but **only with the caller's own authority**: every action re-runs the same
`caps::check` a direct MCP call would, so a rule can do nothing on these planes its invoker couldn't,
and a missing grant throws an opaque error mid-run. Built in three slices, each green before the next.

## Slices
1. **`MessagingSeam` + `inbox`/`outbox` handles + the write meter** — DONE (green below).
2. **`channel.*` MCP verbs** (post/history/edit/delete/list over the existing host fns) — DONE.
3. **`channel` rhai handle** with the worker-kind fence — DONE (green below).

---

## Slice 1 — MessagingSeam + inbox/outbox handles + WriteMeter

### What changed
- `crates/rules/src/seam.rs` — added the `MessagingSeam::call(tool, input) -> Result<Value, SeamError>`
  trait (the ONE bridge from a rule to the inbox/outbox/channel MCP verbs) + the `SeamError`
  (`Denied` opaque / `Failed` verbatim) it returns. Mirrors the existing `DataSeam`/`AiSeam`.
- `crates/rules/src/meter/` — split `meter.rs` into a folder (FILE-LAYOUT, one meter per file):
  `ai.rs` (the unchanged `AiMeter`) + **`write.rs`** (the new `WriteMeter`: one shared per-run budget
  charged by every motion-producing write, reads uncharged; a rejected charge rolls back and is not
  counted; `charge()` returns the write's ordinal for deterministic ids). `mod.rs` re-exports both.
- `crates/rules/src/verbs/inbox.rs` — the `inbox` handle: `list` (read, uncharged), `record`/`resolve`
  (writes, charged). Ids default to `rule-inbox-{now}-{seq}` when the author omits one (deterministic).
  Holds the shared helpers `map_str`/`map_to_json`/`seam_err` the outbox handle reuses.
- `crates/rules/src/verbs/outbox.rs` — the `outbox` handle: `enqueue` (write) + `status` (read). The
  relay-driver verbs (`due`/`mark_delivered`/`mark_failed`) are deliberately absent (Resolved decisions
  — a rule stages, it does not drain).
- `crates/rules/src/verbs/mod.rs` — `register` now wires `inbox`/`outbox` and returns a `RunHandles`
  struct (`ai` + `inbox` + `outbox`) instead of the bare `AiHandle`.
- `crates/rules/src/engine.rs` — `RuleEngine::new` takes the `MessagingSeam` + `max_writes`; `run`
  builds the `WriteMeter`, pushes the three handles into the scope, threads `run.now` for id derivation.
- `crates/rules/src/runtime.rs` — `RuleRun` carries the logical `now` (feeds deterministic write ids).
- `crates/host/src/rules/seam.rs` — `HostMessagingSeam`: a thin `handle.block_on(call_tool(...))`
  closed over the caller's principal + pinned ws (the same block_on bridge `HostDataSeam` uses). Maps
  `ToolError::Denied` → `SeamError::Denied` (opaque), any other fault → `Failed` (verbatim).
- `crates/host/src/rules/config.rs` — `max_writes()` reads `LB_RULES_MAX_WRITES` (default **32**).
- `crates/host/src/rules/run.rs` — builds `HostMessagingSeam`, passes it + `max_writes()` into
  `RuleEngine::new`, threads `now` into `RuleRun::new`.

### Decisions & alternatives
- **The seam returns `SeamError` (a two-variant enum), not `Result<Value, String>`.** The handle must
  keep a capability deny **opaque** (a bare "denied", no plane/cap detail) while surfacing a genuine
  fault (bad input) verbatim. A stringly-typed error would force the handle to sniff substrings (the
  brittle thing `map_eval_error` already has to do for `source not allowed`); a typed `Denied` variant
  makes "opaque vs verbatim" a match, not a guess.
- **One shared write meter, reads uncharged** (per Resolved decisions) — mirrors the `AiMeter` the
  author already reasons about; a single "writes/run" number is the honest DoS bound. `charge()`
  returns the ordinal so ids stay stable across a re-run without a second counter.
- **`meter.rs` → `meter/` folder.** Two meters = two responsibilities = two files (FILE-LAYOUT). The
  `AiMeter` file moved verbatim (`git mv`), so its port lineage/attribution is preserved.
- **The rules-crate tests keep a `RecordingMessaging` seam** (in `tests/support/mod.rs`) — the
  sanctioned trait boundary standing in for `HostMessagingSeam`, exactly as `RecordingData`/`ScriptedAi`
  already stand in for the data/AI seams. The REAL `call_tool`/caps chokepoint is exercised in the host
  integration tests (slice 2+). No fake of node behavior (CLAUDE §9).

### Tests (green)
`crates/rules/tests/messaging_test.rs` — 6 tests on the real engine path (sandbox + governors + scope):
- `inbox_record_dispatches_the_verb_with_caller_json` — the handle builds the right tool-id + JSON,
  threads the logical `now` as `ts`.
- `outbox_enqueue_and_status_dispatch` — enqueue carries target/action/payload; status reads.
- `reads_are_uncharged_writes_are_charged` — 3 reads + 2 writes under a cap of 2 → OK (reads free).
- `write_meter_bounds_a_dos_loop` — a 1000-iteration `outbox.enqueue` loop under a cap of 3 aborts with
  a budget error, and **exactly 3** writes reached the seam (rejected charge not counted). The DoS bound.
- `ids_are_deterministic_across_a_rerun` — same `now` + body ⇒ identical ids (re-run upserts); a
  different `now` ⇒ a different id; the id embeds the logical clock, no wall-clock/random.
- `denied_verb_is_opaque_with_no_partial_write` — a denied `outbox.enqueue` surfaces a bare "denied"
  (opaque, the caller-gated regression) and produced **zero** enqueue dispatch (no partial write within
  the denied verb).

```
$ cargo test -p lb-rules
   ...
     Running tests/cage_test.rs
test result: ok. 5 passed; 0 failed; 0 ignored
     Running tests/grid_test.rs
test result: ok. 7 passed; 0 failed; 0 ignored
     Running tests/messaging_test.rs
running 6 tests
test inbox_record_dispatches_the_verb_with_caller_json ... ok
test denied_verb_is_opaque_with_no_partial_write ... ok
test outbox_enqueue_and_status_dispatch ... ok
test write_meter_bounds_a_dos_loop ... ok
test reads_are_uncharged_writes_are_charged ... ok
test ids_are_deterministic_across_a_rerun ... ok
test result: ok. 6 passed; 0 failed; 0 ignored
     Running tests/ai_fence_test.rs   ... ok (unchanged; ported to the new signatures)
   Doc-tests lb_rules ... ok
```
`cargo build -p lb-host` — clean (the host wiring compiles). `cargo fmt -p lb-rules -p lb-host` — clean.

---

## Slice 2 — `channel.*` MCP verbs

### What changed
- `crates/host/src/channel/tool.rs` (new) — `call_channel_tool`: the thin `channel.post`/`history`/
  `edit`/`delete`/`list` dispatch arms over the existing host fns. Each is **gate-identical** to the
  WS/`POST /channels` path — the host fn re-runs its own `bus:chan/{cid}:{Pub|Sub}` gate; this file adds
  NO new gate, only a JSON front door. **Author is forced to the caller's `sub`** (a request `author` is
  ignored — never spoofable), matching `inbox.record`. `channel.post` keeps FULL PARITY with the WS
  path (a `kind:"query"`/`"agent"` body still triggers the inline/background workers — the rule-layer
  fence is slice 3). `ChannelError::Denied` → opaque `ToolError::Denied`; `NotFound` surfaces.
- `crates/host/src/channel/mod.rs` — `mod tool;` + `pub use tool::call_channel_tool`.
- `crates/host/src/lib.rs` — re-export `call_channel_tool`.
- `crates/host/src/tool_call.rs` — `is_host_native` now matches `channel.` (broadened from the
  `channel.chart_pref.` prefix); a new dispatch arm routes `channel.*` to `call_channel_tool`, placed
  AFTER the `channel.chart_pref.` arm so the per-viewer plot-override verbs still win.
- `role/gateway/src/session/credentials.rs` — dev member now holds `mcp:channel.history:call` +
  `mcp:channel.edit:call` (the two verb names not covered by the `mcp:*.<verb>:call` wildcards; post/
  list/delete already are). This only opens the outer MCP door — each verb still re-checks the channel
  `Pub`/`Sub` gate (held via `bus:chan/*:pub|sub`) inside.

### Decisions & alternatives
- **`channel.` is host-native, not a runtime-registry route.** A channel is the host's own plane over
  the embedded store+bus (like `inbox.`/`outbox.`), so it dispatches in `tool_call.rs`, not via
  `lb_mcp::call`. A future channel *extension* would carry its own `<ext>.` id and still route to the
  registry — `channel.` is the host service, one owner (no rule-10 leak: the id stays opaque data).
- **Author forced, worker-kinds NOT fenced at the generic verb.** The generic `channel.post` keeps
  parity with the WS path deliberately (Resolved decisions) — a UI/agent posting a `query`/`agent` item
  must still spawn the worker. Only the *rule handle* (slice 3) fences worker kinds, so the fence lives
  in the rule layer and the verb stays uniform.

### Tests (green) — `crates/host/tests/channel_mcp_test.rs`, real Node/store/bus/caps, via `call_tool`
- `post_history_edit_delete_list_roundtrip` — the full surface round-trips through the dispatcher.
- `author_is_forced_not_request_supplied` — a forged request `author` is ignored; stored author = caller.
- `post_denied_without_pub_cap_is_opaque_with_no_write` — a `Pub`-less caller is denied (opaque) at the
  dispatcher and NO write landed (a Sub-capable read shows an empty channel). The dispatcher deny-test.
- `history_denied_without_sub_cap` — a `Sub`-less caller is denied `channel.history` (opaque).
- `workspace_isolation_ws_b_cannot_touch_ws_a` — ws-B never sees ws-A's message (own namespace), and
  ws-A is unaffected by ws-B's write to the same `cid`. The workspace wall.

```
$ cargo test -p lb-host --test channel_mcp_test
running 5 tests
test post_denied_without_pub_cap_is_opaque_with_no_write ... ok
test history_denied_without_sub_cap ... ok
test author_is_forced_not_request_supplied ... ok
test workspace_isolation_ws_b_cannot_touch_ws_a ... ok
test post_history_edit_delete_list_roundtrip ... ok
test result: ok. 5 passed; 0 failed; 0 ignored
```
Existing `channel_chart_pref_test` (3) + `channel_query_worker_test` (2) still green (the `channel.`
broadening did not disturb the `channel.chart_pref.` routing).

---

## Slice 3 — the `channel` rhai handle + the worker-kind fence

### What changed
- `crates/rules/src/verbs/channel.rs` (new) — the `ChannelHandle`: `channel.post(cid, #{kind?, body,
  id?})`, `channel.history(cid, n)` (+ a `channel.history(cid)` overload = whole history),
  `channel.edit(cid, mid, #{body})`, `channel.delete(cid, mid)`, `channel.list()`. Thin builders over
  the slice-2 `channel.*` MCP verbs via the `MessagingSeam` — each call re-runs the host's ws pin +
  `bus:chan/{cid}:{Pub|Sub}` gate. `history`/`list` are reads (uncharged); `post`/`edit`/`delete` are
  writes (charged against the shared `WriteMeter`). `post` ids default to `rule-channel-{now}-{seq}`
  (deterministic). Reuses `map_str`/`map_to_json`/`seam_err` from `inbox.rs` (the shared helpers).
- **The worker-kind fence lives in `fenced_body`**: `kind:"agent"`/`kind:"query"` is rejected with an
  author error ("a rule cannot spawn a run — use a flow") **before `charge()`** — so a fenced post
  never touches the meter or the seam. `kind:"text"`/no-kind → the raw `body` string (a plain chat
  message, WS parity); any other kind serializes as a JSON envelope inside `body` (payload parity).
- `crates/rules/src/verbs/mod.rs` — `mod channel;` + `pub use ChannelHandle`; `register` wires the
  handle and `RunHandles` grew a fourth field `channel` (now `ai`+`inbox`+`outbox`+`channel`).
- `crates/rules/src/engine.rs` — pushes the `channel` handle into the run scope alongside the others.

### Decisions & alternatives
- **The fence is a rule-layer concern, not a verb concern (scope Resolved decision).** The generic
  `channel.post` MCP verb (slice 2) keeps full WS parity — a `query`/`agent` post spawns the worker —
  because the UI/agent legitimately do that. Only the *rule handle* fences those kinds: a bounded,
  synchronous rule must not quietly kick an unbounded background run (and invite recursion). So the
  restriction lives in `fenced_body`, and the generic verb stays uniform (rule 7, no special-casing).
- **Fence before charge.** The reject is placed before `meter.charge()`, so a fenced post costs the
  author nothing against the write budget and produces zero dispatch — proven by
  `channel_post_rejects_agent_kind_before_any_write` (asserts `count("channel.post") == 0`).
- **`kind` detection mirrors the host `payload.rs` contract** — `kind` is a key inside the item body,
  and only `query`/`agent` are worker-triggering. The handle keys off exactly those two.

### Tests (green)
`crates/rules/tests/messaging_test.rs` — 8 new channel tests on the real engine path (14 total in file):
- `channel_post_history_edit_delete_list_dispatch` — the full surface builds the right tool-id + JSON
  (cid/id/body/ts, the `n` tail on history).
- `channel_reads_are_uncharged_writes_are_charged` — under a cap of 1: 4 reads + 1 post all succeed.
- `channel_post_loop_is_bounded_by_the_write_meter` — a 1000-post loop aborts at the cap of 3; exactly
  3 reach the seam (the DoS bound).
- `channel_post_rejects_agent_kind_before_any_write` / `channel_post_rejects_query_kind` — the fence:
  an author error + **zero** dispatch (no run spawned, no charge).
- `channel_post_text_kind_passes_the_fence` — `kind:"text"` posts, body is the raw text.
- `channel_post_denied_is_opaque_with_no_partial_write` — a `Pub`-denied post surfaces a bare "denied"
  and lands no write.
- `channel_post_ids_are_deterministic_across_a_rerun` — same `now`+body ⇒ same id (re-run upserts).

`crates/host/tests/rules_test.rs` — 4 new integration tests through a REAL `rules.run` (11 total),
real store/bus/caps/MCP (`FULL` grant grew `mcp:channel.post|history:call` + `bus:chan/*:pub|sub`):
- `rule_channel_post_lands_a_real_message` — a rule's `channel.post` commits a real message read back
  via the real `channel.history` verb; author is forced to the caller (`user:test`).
- `rule_channel_post_is_caller_gated_and_opaque` — a `Pub`-less caller is denied mid-run, opaquely, and
  a Sub-capable read shows an empty channel (the caller-gated regression, no partial write).
- `rule_channel_post_worker_kind_is_fenced_at_the_handle` — a FULL-caps rule posting `kind:"agent"` is
  rejected by the handle; the channel stays empty (no run spawned) — the fence at the real chokepoint.
- `ws_b_rule_cannot_post_into_a_ws_a_channel` — a ws-B rule posting to "ops" lands only in ws-B's
  namespace; ws-A's "ops" is untouched (the workspace wall through a real run).

```
$ cargo test -p lb-rules --test messaging_test
test result: ok. 14 passed; 0 failed; 0 ignored
$ cargo test -p lb-host --test rules_test
running 11 tests
test rule_channel_post_lands_a_real_message ... ok
test rule_channel_post_is_caller_gated_and_opaque ... ok
test rule_channel_post_worker_kind_is_fenced_at_the_handle ... ok
test ws_b_rule_cannot_post_into_a_ws_a_channel ... ok
...
test result: ok. 11 passed; 0 failed; 0 ignored
```
Slice-2 `channel_mcp_test` (5) / `channel_chart_pref_test` (3) / `channel_query_worker_test` (2) and the
`messaging_*` host suite still green (the handle added no new gate; the generic verbs are undisturbed).
`cargo fmt -p lb-rules -p lb-host` — clean.

### Offline / sync + the real-gateway integration (both green)

**Offline/sync** — `crates/host/tests/rules_test.rs::channel_posting_rule_and_its_message_survive_a_restart`
(12 tests in file now). A persistent `Store::open(temp_dir)` under `Node::boot_with_store`: boot 1 saves
a channel-posting rule + runs it (the post lands a durable record — channel history is *state*, §3 rule 3);
a fresh Node re-opened on the SAME store still holds both the saved rule AND the posted message. Proves the
run's effect is durable and the rule is a record (rule 4 — the cage itself holds no state).

**Integration (real spawned gateway)** — `ui/src/features/rules/RulesMessaging.gateway.test.tsx` (3 tests,
`pnpm test:gateway`). A rule body runs through the real `rules.run` client and the channel is read back
through the real `channel.history` client (the UI's own MCP verbs — rule 7, no fake backend):
- a rule's `channel.post` lands a durable message authored as the signed-in caller (`user:ada`);
- the worker-kind fence: a rule posting `kind:"agent"` is rejected and spawns NO run (empty channel);
- caller-gated: a caller lacking `bus:chan/*:pub` (signed in via `/_seed/session`) is denied, opaquely,
  with no partial write.

```
$ cargo test -p lb-host --test rules_test
test result: ok. 12 passed; 0 failed; 0 ignored
$ pnpm test:gateway RulesMessaging
 ✓ src/features/rules/RulesMessaging.gateway.test.tsx (3 tests)
 Test Files  1 passed (1)   Tests  3 passed (3)
```

The slice is complete: the `channel` handle is proven at the crate level (real engine), the host level
(real store/bus/caps/MCP through `rules.run`), across a restart (durable), and end-to-end through the UI's
own clients against a real spawned node.

### UI examples (the Playground Examples tab)

`ui/src/features/rules/examples/examples.ts` — a full set of ready-to-run messaging examples in the
existing catalog (auto-rendered by `ExampleList`, one click loads into the editor). **All are zero-seed
safe** — they read/write only the inbox/outbox/channel planes, which resolve empty-but-valid on a
brand-new workspace (confirmed against the boot/create path: no data/series/channel is seeded on workspace
creation; `source("series")`/`source("store")` always resolve but return empty, and the plane reads all
return empty collections, never errors). Each simple example teaches ONE verb so a newcomer sees exactly
what it does:
- **Inbox** — `inbox-record` (raise), `inbox-list` (read), `inbox-record-then-list` (raise+read in one),
  `inbox-resolve` (close with a verdict).
- **Outbox** — `outbox-enqueue` (stage a must-deliver effect), `outbox-status` (read the queue).
- **Channel** — `channel-post`, `channel-read` (history), `channel-list`.
- **Escalate: inbox + outbox + channel** — the full surface in one rule (raise → stage a page → post),
  now unconditional (dropped the `cooler.temp` breach guard that depended on seeded data and silently
  posted nothing), so a first run shows real output.

**Handle fix surfaced by the examples:** `inbox.resolve`'s `decision` is the `Decision` enum
(`"approved"`/`"rejected"`/`"deferred"`), which deserializes from a **string** — but the rhai handle took
a `Map` and JSON-encoded it, so the verb rejected it (`decision: invalid value: map, expected map with a
single key`). Changed `crates/rules/src/verbs/inbox.rs` `resolve(item_id, decision: &str)` to take the
verdict string directly and reject anything but the three valid verdicts with an author error. Crate test
`inbox_resolve_takes_a_string_verdict` (15 in file now) covers both the valid dispatch and the reject.

The catalog docstring promises its bodies actually run ("an example that lies is worse than none"), so
`RulesMessaging.gateway.test.tsx`'s 4th test — `every messaging example in the catalog runs green with no
seeding` — runs ALL ten messaging example bodies (inbox/outbox/channel + escalate) through the real
`rules.run` on a fresh workspace with zero seeding and asserts the posting examples land real messages.
`pnpm test:gateway RulesMessaging` → **4 passed**; the existing `AuthoringPanel` (7) example tests still
green (the new rows render without wiring).

### Result JSON view — deep-parse + interactive tree

The `channel.history` result exposed a readability gap: a row's `body` is a JSON **string** (the kind-tagged
payload rides inside `body` as text), so the old `<pre>`-based `JsonView` showed it as an escaped
`"{\"kind\":\"agent_result\",…}"` blob (and the Table view as one giant line). Fixed in two parts:
- `ui/src/features/rules/parseEmbeddedJson.ts` (new) — deep-parses any field whose value is *itself* a JSON
  object/array string into a real nested value, recursively (payload-in-payload), while leaving faithful
  scalars (`"42"`/`"true"`/a plain message that merely starts with `{`) untouched — over-parsing would
  silently change the data the author sees. 5 unit tests (`parseEmbeddedJson.test.ts`).
- `ui/src/features/rules/JsonView.tsx` — rewritten to run that pre-parse and render with
  **`@microlink/react-json-view`** (added dep; the maintained react-json-view fork): a collapsible,
  syntax-highlighted tree, expanded by default (faithful, never abridged), built-in per-node clipboard, a
  base-16 theme mapped onto the workbench's `--bg/--fg/--muted/--accent` tokens (matches dark mode, not a
  foreign palette). Deep arrays group after 100 and every node stays click-collapsible for a large history.

The tree renderer is the shared `ui/src/features/rules/JsonTree.tsx` (deep-parse + react-json-view + the
theme), used by BOTH result views so neither dumps an escaped blob:
- **JSON view** (`JsonView.tsx`) — the whole `RunResult` as the tree.
- **Table view** (`ScalarCard.tsx`) — a *structured* scalar (an object/array, e.g. a `channel.history`
  array) now renders the same `JsonTree` instead of a raw one-line `JSON.stringify` blob; a true scalar
  (number/string/bool) still shows as the big headline value. This was the actual bug in the screenshot:
  a channel-history result is `output.kind:"scalar"` with an array value, so the **Table** view (default)
  hit `ScalarCard`'s `JSON.stringify` and showed the escaped wall of text.

`RulesView.gateway.test.tsx` grew two tests — `a structured scalar renders as a parsed tree in the Table
view, not an escaped blob` (the default view) and `the JSON view deep-parses an embedded JSON-string field
into an expandable tree` — each runs a rule returning a JSON-string `body` and asserts the nested
`kind`/`agent_result`/`answer` render as parsed tree content (not the escaped blob).
`pnpm test:gateway rules/RulesView` → **11 passed**; `pnpm test parseEmbeddedJson` → **5 passed**.

## Slice: interactive messaging writes collapsed to `now=0` (id + ts) — 2026-07-04

**Bug (user-reported):** two `channel.post`s in the Playground showed only ONE message (each
replaced the last), and the message had no timestamp (sorted oldest).

**Cause:** `rules.run` derives the run's logical clock `now` from the request `ts`, defaulting
to `0` when absent (`host/src/rules/mod.rs:140`). The messaging handles derive a **deterministic**
id from `now` (`rule-channel-{now}-{seq}`) for scheduled-re-run idempotency — but the interactive
path never sends `ts`: the UI client doesn't, and the gateway `run_rule` route (unlike
`datasources`/`dashboard`, which thread `gw.now()`) didn't inject the live clock. So every
interactive run got `now=0` → same id (store upserts the same row) and `ts:0` (sorts oldest).
All three handles (`channel.post`/`inbox.record`/`outbox.enqueue`) share `now`, so all three were
affected.

**Fix:** inject the live wall clock at the gateway edge when the caller omitted `ts` —
`rust/role/gateway/src/routes/rules.rs::run_rule`:

```rust
if input.get("ts").is_none() {
    input["ts"] = json!(gw.now());
}
```

`Gateway::now()` returns the pinned test clock or live unix-seconds. The clock is injected at the
edge; core crates stay clock-free ("no wall-clock in core"). A caller that supplies its own `ts`
(scheduler/programmatic re-run) keeps its deterministic, idempotent write — `ts` is only filled
when missing, so the idempotency contract (`rules-messaging-scope.md`, "Deterministic, idempotent
writes") is untouched.

**Test:** `rules_routes_test.rs::interactive_channel_posts_append_with_distinct_ids_and_ascending_ts`
— two runs over gateways with an advancing fixed clock (one shared node) post one message each; a
read-only run asserts the history holds **2** distinct-id, ascending-, non-zero-`ts` rows; a fourth
run at the SAME clock upserts (still 2), proving idempotency survives. Green:
`cargo test -p lb-role-gateway --test rules_routes_test` → 14 passed; `cargo test -p lb-rules` → 15
passed; `cargo test -p lb-host --test rules_test` → 12 passed; `pnpm test:gateway RulesMessaging` →
4 passed. (Pre-existing flake unrelated to this change: `lb-host`'s
`control_engine_appliance_routing_test` — a Zenoh cross-node queryable-reachability timeout, see the
flaky-bus-timing history.)

Debug entry: ../../debugging/rules/interactive-rule-messaging-writes-collapse-to-now-0.md

## Related
- Scope: ../../scope/rules/rules-messaging-scope.md
- Sibling: ../../scope/rules/rules-ai-wiring-scope.md (the `ai.*` seam this mirrors).
- Public (on ship): ../../public/rules/rules.md
