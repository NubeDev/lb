# Extensions — the `github-bridge` as an installed wasm artifact (session)

- Date: 2026-06-26
- Scope: ../../scope/extensions/github-bridge-scope.md
- Stage: S7 — platform maturity (STAGES.md). An S7 **follow-up** resolving the explicit S6 deferral
  (coding-workflow scope: *"Packaging as wasm extensions — when coding-workflow/github-bridge move
  from host services to installed artifacts"*). The S7 exit gate was already MET.
- Status: done

## Goal

Package the **github-bridge** — the inbound edge of the S6 coding workflow — as an installable, signed
**Tier-1 wasm extension** installed through the (now real-HTTP) registry, proving the full
installed-artifact lifecycle on a **second** real extension (the first was `hello`). The bridge
normalizes a raw GitHub webhook into the canonical `{ issue_id, payload, ts }` triple the host's
`workflow.ingest_issue` already consumes.

## The decision that shaped the slice (decided with the user)

Two findings during scoping forced a design choice, surfaced to the user rather than silently picked:

1. **The orchestrator cannot be a portable wasm guest.** `workflow/mod.rs` is explicit: the
   orchestrator must drive host-internal seams (`caps::check`, the S5 agent loop / `ModelAccess`,
   durable jobs, the transactional outbox) that a sandboxed guest reaches only *through* MCP, never
   *is*. So only the **github-bridge inbound edge** is the cleanly packageable piece.

2. **The stable WIT world has no host-tool-call import.** `rust/sdk/wit/world.wit` gives a guest exactly
   one host import — `host.log`. There is **no** way for a guest to call `workflow.ingest_issue` from
   inside the sandbox without adding a host import to the **forever ABI** (README §11.2, a major-bump
   class change; the native-tier "child→host MCP callback transport" is also unbuilt).

→ Chosen shape (user-approved): a **pure-transform bridge, no ABI change.** The wasm guest is a
stateless *normalizer* exposed through the existing `tool.call(name, input-json) -> json` export; the
**host** (`ingest_via_bridge`) takes the guest's normalized output and calls `ingest_issue`. The WIT
boundary is untouched, the sandbox stays minimal, and the split falls exactly on the trust/seam line:
normalization (pure, untrusted-input-shaped) → the sandbox; the must-deliver inbox write (state under
a capability gate) → the host.

## What changed

New code, one responsibility per file:

- **`rust/extensions/github-bridge/`** (new Tier-1 wasm crate, excluded from the workspace like
  `hello`/`hello-v2`):
  - `extension.toml` — the §13 manifest: `id = "github-bridge"`, `tier = "wasm"`, `request = []` (a
    pure transform needs no host capability), one tool `normalize`, `visibility = "private"`.
  - `src/lib.rs` — the guest: `normalize` maps a GitHub `issues`/`issue_comment` webhook to
    `{ issue_id = "<repo>#<number>", payload, ts }`. Pure, stateless, no callback; a malformed payload
    is a `bad-input` tool error, never a panic.

- **`rust/crates/host/src/workflow/ingest_via_bridge.rs`** (new host verb) — composes the two existing
  seams: `lb_mcp::call("github-bridge.normalize", raw)` then `ingest_issue(normalized)`. Two
  independent gates apply (the normalize tool's `mcp:github-bridge.normalize:call`, then the write's
  `mcp:workflow.ingest_issue:call`); neither is widened. A `Denied`/`NotFound` from the tool stays
  opaque; a transform fault is the new `WorkflowError::Bridge`.

- **`workflow/error.rs`** — added `WorkflowError::Bridge(String)` (the transform's failure source,
  distinct from `Agent`); **`workflow/tool.rs`** maps it to `ToolError::Extension`;
  **`workflow/mod.rs`** + **`lib.rs`** export `ingest_via_bridge`.

- **`rust/Cargo.toml`** — `extensions/github-bridge` added to the workspace `exclude` list.

Nothing in the registry/install path changed — `install_from_registry` installed the second extension
exactly as it installs `hello` (the seam was already general).

## Decisions & alternatives

- **Pure-transform bridge, host composes.** (See "the decision that shaped the slice".) Rejected:
  adding a `host.call_tool` import to the WIT world so the guest calls `ingest_issue` itself — a
  major-bump-class change to the forever ABI plus a host-side re-check for guest-originated calls,
  unnecessary for an installable normalizer. Flagged to the user; deferred as its own scope. Rejected:
  keeping the bridge a host service — then the S6 deferral is never resolved and the registry never
  proves a second real extension end to end.
- **`issue_id` scoped by repo (`<repo>#<number>`).** Two repos' issue #1 must not collide on the inbox
  upsert; the id must be deterministic from issue *identity* (not the event) so a re-delivered webhook
  upserts one item. Idempotency still lives on the host's `(channel, id)` — the guest just supplies a
  stable key.
- **`WorkflowError::Bridge`, not reusing `Agent`.** A transform fault is a genuinely new failure
  source; folding it into `Agent` (which wraps `AgentError`) would be misleading. One small variant,
  mapped to `ToolError::Extension` at the bridge.

## Tests

`crates/host/tests/github_bridge_test.rs` (lifecycle) + `github_bridge_normalize_test.rs` (transform +
per-tool deny) — split to keep each file under the 400-line FILE-LAYOUT limit. Mandatory categories
applied at this slice's surfaces, all through the **real** `github_bridge_ext.wasm`:

- **happy / round-trip:** a signed bridge installs through the registry; a raw webhook → normalize →
  `ingest_issue` lands one canonical `triage` item (`acme/api#2451`, `needs:triage`), idempotent on retry.
- **capability-deny:** install refused without `mcp:registry.install:call`; `normalize` refused without
  its grant (gate 1); `ingest_issue` refused without its grant (gate 2, after the transform ran).
- **workspace-isolation:** ws-B has no `Install` record; a granted ws-B caller runs the node-global
  *stateless* instance but its ingest lands in **ws-B's** inbox, **ws-A untouched**; an un-granted ws-B
  caller is `Denied`. (See the debugging entry — the test premise was corrected to the real, stronger
  property.)
- **offline:** once cached, the bridge installs with the registry `Source` offline (zero fetches).
- **rollback/hot-reload:** install 0.2.0, ingest a webhook, roll back to 0.1.0 — the inbox item survives
  (no durable guest state, §3.4).
- **transform branches:** an `issue_comment` payload folds the comment into the body; a malformed
  payload is `WorkflowError::Bridge`, never a panic.

Green output:

```
$ (cd rust/extensions/github-bridge && cargo build --target wasm32-wasip2 --release)
   Finished `release` profile [optimized] target(s) — github_bridge_ext.wasm

$ cargo test -p lb-host --test github_bridge_test --test github_bridge_normalize_test
running 5 tests (github_bridge_test)
test install_is_denied_without_the_grant ... ok
test ws_b_ingest_lands_in_ws_b_never_ws_a ... ok
test installs_the_bridge_and_ingests_a_webhook_end_to_end ... ok
test cached_bridge_installs_with_the_registry_offline ... ok
test rollback_keeps_the_ingested_inbox_intact ... ok
test result: ok. 5 passed; 0 failed

running 2 tests (github_bridge_normalize_test)
test normalize_maps_comment_and_rejects_malformed ... ok
test normalize_and_ingest_are_denied_without_their_grants ... ok
test result: ok. 2 passed; 0 failed

$ cargo test -p lb-host --test workflow_test --test workflow_isolation_test --test workflow_offline_test
workflow_test:            ok. 3 passed     # regression — WorkflowError + module unchanged in behavior
workflow_isolation_test:  ok. 2 passed
workflow_offline_test:    ok. 3 passed

$ cargo test -p lb-host --test registry_test --test registry_offline_test \
    --test registry_isolation_test --test registry_rollback_test   # regression — install path unchanged
registry_test:            ok. 4 passed
registry_offline_test:    ok. 3 passed
registry_isolation_test:  ok. 2 passed
registry_rollback_test:   ok. 2 passed

$ cargo fmt --check          # clean
$ cargo clippy -p lb-host --tests          # no new warnings in the slice's files
$ (cd rust/extensions/github-bridge && cargo clippy --target wasm32-wasip2 --release)   # clean
$ git add -A && bash rust/scripts/check-file-size.sh
FILE-LAYOUT: all source files within 400 lines (308 checked)
```

So **7 new + 8 workflow + 11 registry regression** green; fmt + clippy + file-size clean.

## Debugging

`debugging/extensions/loaded-extension-instance-is-node-global.md` — the isolation test, written to
assert "ws-B can't run ws-A's tool", failed: ws-B (granted in its own ws) *can* run the node-global
stateless instance. Root cause: the runtime registry is keyed by `ext_id` alone (since S1, by design —
a stateless instance is safe to share); the workspace wall lives at the **caps gate** (workspace-first)
and the **store** (ws-namespaced records), not at the instance. Production code is correct; the test
expectation was wrong. Rewrote it to assert the real, stronger property (ws-B's effects never touch
ws-A; an un-granted ws-B caller is denied). Regression test:
`github_bridge_test::ws_b_ingest_lands_in_ws_b_never_ws_a`.

## Public / scope updates

- Promote to `public/extensions/extensions.md` + `public/SCOPE.md`: the github-bridge ships as an
  installed Tier-1 wasm artifact; `ingest_via_bridge` composes the sandboxed transform + the host write.
- Resolve the coding-workflow scope deferral (back-linked) and the github-bridge scope's first three
  open questions (where the host calls normalize from — `ingest_via_bridge`; the tool needs no host
  cap — confirmed; visibility — ships private). The webhook-receiver role crate, the `host.call_tool`
  ABI question, multi-forge siblings, and the public-catalog union stay open.

## Dead ends / surprises

- **The WIT wall was the real constraint, not the orchestrator.** The first instinct ("package the
  workflow as wasm") collapses on two walls: the orchestrator needs host-internal seams, and the guest
  can't call back. Discovering the second (one `host.log` import, no tool-call import) is what fixed the
  slice's true shape — a pure transform the host composes. The honest slice is smaller and cleaner than
  the literal ask.
- **The node-global instance "leak" is not a leak.** It surfaced as a failing isolation test and looked
  alarming, but it's the stateless-extension principle paying off: share the pure instance, keep the
  wall at caps + store. Documented so it's never re-litigated.
- **Manifest-only v2 for rollback.** The bridge's rollback test bumps only the manifest `version` (same
  wasm) — the digest binds manifest+wasm, so it's a valid distinct artifact, enough to prove
  install-0.2.0-then-0.1.0 with the inbox intact. (hello-v2 ships a genuinely different wasm because it
  also proves behavior change; the bridge only needs the version delta.)

## Follow-ups

- A **webhook-receiver role crate** (beside `lb-role-registry-host`) that accepts a real GitHub webhook
  over HTTP (with `X-Hub-Signature` verification) and drives `ingest_via_bridge` — the live ingress.
- The `host.call_tool` WIT import question (guest→host MCP callback) — its own scope if/when a guest
  genuinely needs to invoke a host tool.
- Multi-forge sibling bridges (GitLab/Gitea) sharing the `{issue_id, payload, ts}` output contract; the
  public-catalog union so one signed bridge installs cross-workspace.
- STATUS.md: add the slice row + note the deferral resolved.
