# Rules — `messaging` verbs (inbox/outbox/channel rhai handles) — session

- Date: 2026-07-04
- Scope: ../../scope/rules/rules-messaging-scope.md
- Stage: S8 — data plane (rules on the real workspace). See STATUS.md.
- Status: in-progress

## Goal
Let a rule body drive the inbox, outbox, and channel planes **explicitly** — read/write/resolve them
from inside the sandbox — but **only with the caller's own authority**: every action re-runs the same
`caps::check` a direct MCP call would, so a rule can do nothing on these planes its invoker couldn't,
and a missing grant throws an opaque error mid-run. Built in three slices, each green before the next.

## Slices
1. **`MessagingSeam` + `inbox`/`outbox` handles + the write meter** — DONE (green below).
2. **`channel.*` MCP verbs** (post/history/edit/delete over the existing host fns) — pending.
3. **`channel` rhai handle** with the worker-kind fence — pending.

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

### Still to do (slices 2–3, + the mandatory host-side tests)
- Slice 2: the `channel.post/history/edit/delete` MCP dispatch arms in `tool_call.rs` (gate-identical to
  `channel/authorize.rs`) + a deny-test at the dispatcher.
- Slice 3: the `channel` rhai handle + the worker-kind fence (reject `kind:"agent"`/`"query"`).
- Host integration (real store/bus/caps/MCP): capability-deny per verb, workspace-isolation (ws-B ≠
  ws-A), the caller-gated regression through a real `rules.run`, the `channel.post` worker-kind
  regression, offline/sync (a saved rule survives restart), and a Playground `*.gateway.test.tsx`
  against a real spawned node.

## Related
- Scope: ../../scope/rules/rules-messaging-scope.md
- Sibling: ../../scope/rules/rules-ai-wiring-scope.md (the `ai.*` seam this mirrors).
- Public (on ship): ../../public/rules/rules.md
