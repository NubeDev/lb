# `agent.invoke` answers `403 "no such tool"` over `/mcp/call` — to a caller holding the cap

**Area:** agent (dispatch) / reminders
**Date:** 2026-09-07
**Symptom:** `POST /mcp/call {"tool":"agent.invoke", …}` returns **`403 no such tool`** with a token
whose claims contain `mcp:agent.invoke:call`. The same run started from the browser
(`POST /agent/invoke`) works. `tools.catalog` *lists* `agent.invoke`, so the caller can see a verb it
cannot call — which reads as a permissions bug and is not one.

## Root cause — a missing `match` arm, not a gate

`agent.invoke` was built on every layer **except** the MCP dispatch table:

| Layer | State |
|---|---|
| Capability `mcp:agent.invoke:call` | exists, checked by `authorize_invoke` |
| `tools.catalog` descriptor | exists — `agent::descriptor::invoke_descriptor()` |
| Local entry `invoke` / `invoke_via_runtime` | exists, used by the gateway route |
| Routed edge→hub path `invoke_remote` | exists, **exported and tested, with no production caller** |
| **`call_agent_tool` dispatch arm** | **absent** → falls through to `Err(ToolError::NotFound)` |

`tool_call.rs` routes `agent.*` into `call_agent_tool`, whose `match` handled `policy`, `decide`,
`runtimes`, `config`, `def.*`, `persona.*` and `memory.*`, then `_ => Err(NotFound)`. `NotFound`
renders as the opaque `403 "no such tool"` (deliberate — the MCP deny contract does not confirm a
tool's existence), so a *fully granted* caller and a *capless* one get byte-identical answers.

This is the sibling of the `is_host_native` trap already documented in `tool_call.rs`'s
`host_native_tests`: a verb family can have caps, catalog rows and a routed path and still 404,
because one registration point was missed. Here the gate is one layer in — the prefix `"agent."`
*was* registered, so the call reached the agent dispatcher and died in its `match`.

## Why it survived this long

Every shipped caller reaches the agent on a **different transport**:

- the browser/dashboard AI widget → `POST /agent/invoke` (`role/gateway/src/routes/agent_invoke.rs`);
- the command palette → `postAgent`, explicitly *not* a raw MCP call (see the comment at
  `tools/descriptor.rs:61`);
- the in-channel worker → the host fn directly;
- an edge node → `invoke_remote` … which nothing in production calls.

Nothing that *needed* the MCP bridge existed. The first caller that does is a **reminder**: an
`Action::McpTool` fires through `crate::tool_call::call_tool` and has no browser, no session and no
alternative door. So "run the agent on a schedule" — the rubix-ai BMS duty cycle
(`edge-bms-agent-scope.md` Phase 3) — was unbuildable, and the failure pointed at capabilities.

## The fix

`rust/crates/host/src/agent/invoke_tool.rs` — a dispatch arm mirroring the HTTP route exactly
(`reachable_tools` for the menu, `invoke_via_runtime` with `runtime = None` so the workspace's active
pick resolves through the one seam, same persona precedence, same opaque error mapping). Wired in
`tool_call.rs` beside `agent.def.test`, which is handled there for the same reason: both need the
`&Arc<Node>` the dispatcher holds.

**It adds no authority.** `authorize_invoke` (workspace-first, then `mcp:agent.invoke:call`) still
runs inside, and the run is bounded by `agent ∩ caller` exactly as on the browser path. It is a
transport, not a widening.

Two deliberate differences from the HTTP route:

1. **`job_id` is required.** The route derives a stable hash from `(ws, goal)` when a browser omits
   it. A reminder fires a *static* payload every tick, so a derived id would be identical on every
   firing and the loop — idempotent on `job_id` — would replay the first answer forever without ever
   calling the model. A named `BadInput` beats a silently frozen agent. (The companion `{{fire_ts}}`
   placeholder is what lets a schedule supply a varying one; see below.)
2. **`agent.invoke` is filtered out of the menu handed to the model.** The dispatcher applies no
   depth ceiling to host-native verbs, so a run that could propose `agent.invoke` would nest runs each
   carrying a full `MAX_STEPS` budget. Removing it from the *menu* (not the caps) keeps a deliberate
   nested invoke working while never handing the model the recursion. Honest limit: a model that
   invents the call anyway would still execute it — a hard nesting ceiling is a separate change.

## The second bug the same work exposed — a schedule that replays forever

Reminder `args` are stored verbatim and passed through unchanged, so a scheduled `agent.invoke` with a
fixed `job_id` returns its first answer every day, never calling the model — a frozen agent that looks
alive. Fixed generically in `reminder/range.rs`, the module that *already* owns fire-time payload
substitution (it resolves a named `range` into concrete days against the fire clock): a `{{fire_ts}}`
token in any string value of the payload is replaced with the fire clock's epoch seconds.

Generic over the action, per rule 10 — it reads string values, never tool names, so `mcp-tool` args
and an `outbox` JSON payload get identical treatment, and a payload without the token passes through
structurally unchanged (the placeholder is additive; existing reminders fire byte-identically).

Usage: `"job_id": "daily-review-{{fire_ts}}"`.

## The third gap the same work exposed — a pack could not declare a schedule

With both fixes in, a scheduled agent run works — but only if somebody creates the reminder by hand.
`pack.yaml` had `rules:`, `channels:`, `agent:` and `retention:` and no `reminders:`, so a pack that
ships seven FDD rules shipped a product that detects nothing until an operator wires the cron
off-manifest: undocumented, outside the receipt, and not reproducible on a fresh box. Every other
pack section seeds a thing that sits inert until something drives it; the schedule IS the driver, and
it was the one part that could not be declared.

Added as an inline section on the `retention:` model — `lb_packs::manifest_reminder` (a
dependency-free mirror of the `reminder.create` args, because `lb-reminders` depends on `lb-store`
and `lb-packs` is the pure half), `lb_packs::validate_reminder` (the lint the mirror makes necessary),
and `pack/reminder_action.rs` + a `Kind::Reminder` arm driving the same `reminder_create` the public
verb calls. **It adds no authority:** the reminder is stored with the applying principal, and every
firing re-resolves that principal's caps at the `call_tool` chokepoint — a pack-seeded schedule
carries nothing its admin lacks, and `pack.apply` without `mcp:reminder.create:call` gets `denied` on
the reminder objects and an honest partial receipt.

The lint is where the mirror pays for itself: an unknown `action_kind`, a kind missing the fields it
needs, a 6-field quartz cron — and, as a *warning*, the static-`job_id` trap from the section above.
A warning rather than an error because the manifest cannot know which verbs are idempotent on which
field; `job_id` without `{{fire_ts}}` is the one shape where the intent is unmistakable.

## Tests

- `crates/host/tests/agent/agent_invoke_mcp_test.rs` — the arm exists and the loop really runs (a
  proposed host-native call lands its side effect); **the regression guard** (a granted caller must
  never get `NotFound`); the mandatory capability-deny (capless → opaque `Denied`, nothing executed);
  the `job_id` / `goal` arg contract; and the menu filter.
- `crates/host/src/reminder/range.rs` unit tests — two firings produce distinct job ids; the token is
  substituted through nested objects and arrays; a payload without it is unchanged; `{{fire_ts}}` and
  `range` resolve together.
- `crates/host/tests/pack_reminders_test.rs` — a pack seeds a schedule that will really fire (the
  row is listable AND has a `nextAttemptTs` past the apply clock, so the reactor picks it up, and
  `{{fire_ts}}` is stored UNSUBSTITUTED — baking the apply clock in would freeze every firing);
  the per-object caps wall (no `mcp:reminder.create:call` → `denied`, the channel in the same pack
  still applies, nothing scheduled); LWW on re-apply (an edited cron moves the schedule, never forks
  it); and both lints, including that a warning does not gate the apply.
- `crates/packs/src/validate_reminder.rs` + `manifest.rs` unit tests — the closed action-kind set,
  the per-kind required fields, the cron field count, the replay warning, and `deny_unknown_fields`
  on a typo'd reminder key.

RED-then-green verified: disabling the dispatch arm fails 3 of the 6 tests with the intended message.

## The lesson

**A verb is not reachable because it is authorized, described and implemented.** Reachability is a
per-transport registration, and the MCP deny contract makes a missing arm indistinguishable from a
missing capability. When a caller with the right cap gets `no such tool`, look for the `match` arm
before auditing the grant — and when adding a verb, ask which transports actually need it, because
the browser exercising one door proves nothing about the others.
