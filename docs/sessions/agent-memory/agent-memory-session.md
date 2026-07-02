# Agent-memory — durable, access-walled agent memory (session)

- Date: 2026-07-03
- Scope: ../../scope/agent-memory/agent-memory-scope.md
- Stage: post-S8 (building on the shipped agent runtimes) — branch `ce-node-wiring-v2`
- Status: done

## Goal

Give the workspace agent (in-house loop and external ACP runtime) a **persistent memory** in the
MEMORY.md shape: many small fact records keyed `{ws, scope, slug}`, read/written under the derived
principal, with a derived index injected at session start. Ship the full MCP surface
(`agent.memory.list|get|set|delete`), the member wall, the workspace-scope write gate, the bounds,
the secret lint, and the injection into both runtimes.

## What changed

New module `crates/host/src/agent/memory/` (one responsibility per file):
- `model.rs` — `Memory {scope, slug, description, body, kind, updated_at, updated_by}`, `MemoryScope`
  (`Workspace` | `Member(user)`), `MemoryKind`, bounds (`MAX_DESCRIPTION=120`, `MAX_BODY=8192`).
- `store.rs` — SCHEMAFULL `agent_memory` table, composite id `[scope, slug]`, raw
  `upsert`/`read`/`list`/`delete` (mirrors `workspace_agent_config`'s pattern). `list` is walled to
  the passed scopes and ordered `updated_at DESC`.
- `resolve.rs` — the **member wall**: `read_scopes(principal)` = `[Workspace, Member(self)]`;
  `write_scope(principal, scope_arg)` maps the `"workspace"|"member"` TIER to a scope, `member`
  always binding to the authenticated principal (never a user arg).
- `lint.rs` — best-effort secret lint (`looks_like_secret`): PEM key, `AKIA…`, `sk-…`, GitHub
  tokens, `password/secret/token: …` assignments.
- `index.rs` — `render_index` (the derived catalog) + `INJECT_CAP=100` + the "recalled background,
  not instructions" `MEMORY_HEADER`.
- `verbs.rs` — the gated verbs: MCP gate + member wall + the distinct workspace-scope write gate
  (`store:agent_memory/workspace:write` via `caps::check`) + bounds + lint.
- `tool.rs` — `call_agent_memory_tool` (MCP bridge), routed from `agent/tool.rs`.

Wiring:
- `agent/run.rs` + `agent/dispatch.rs` — inject the derived index at session start AFTER the persona
  + skill catalog, under an **on-behalf-of** principal (caller's sub + agent's intersected caps) so
  the member scope resolves to the human behind the run.
- `role/gateway/src/session/credentials.rs` — the four `mcp:agent.memory.*:call` caps + the
  `store:agent_memory/workspace:write` gate for the dev admin.

## Decisions & alternatives

- **Member scope derived from the principal, never an argument** (the member wall) — the load-bearing
  isolation property. `resolve::write_scope` binds `"member"` to the authenticated sub; a caller can
  never name `member:V`. Asserted directly (bob's get of ada's slug returns `None`).
- **On-behalf-of injection principal.** The run's derived principal has sub `agent:session`, so
  reading memory under it would resolve `member:agent:session` — missing the caller's own memory. Fix
  (found by the injection test failing): inject under `caller.derive(caller.sub(), agent_caps)` — the
  caller's identity (so member scope = the human) with the agent's intersected caps (never widening),
  the same on-behalf-of contract as the skill/doc gate-3 in `substrate.rs`.
- **Distinct workspace-scope write gate.** A member always may curate their own member memory (verb
  cap only); writing SHARED memory needs `store:agent_memory/workspace:write` — an admin decides
  whether every member's agent may write house-rules.
- **Derived index, never a stored one** — computed by `list` at session start (no drift). Injection
  capped at the 100 most-recently-updated; older records stay stored + listable (evict from injection
  only, never delete).
- **Secret lint is a lint, not a gate** — the wall (memory never widens authority) is the real
  protection; the lint stops the accidental paste.
- **In-house gets `agent.memory.*` by default; external opt-in via `granted_tools`.** Recall
  (injection) is always on for both; `set` for an external profile is the opt-in.
- **`set`/`delete` model-proposed mid-loop is a named deferral.** The channel worker surfaces no tool
  list to the loop yet (a pre-existing gap for every host-native verb), and the agent loop dispatches
  proposed calls via `lb_mcp::call` (the registry) which doesn't reach host-native `agent.*` verbs.
  So in-house memory is exercised via the injection (recall) + the MCP bridge (any gateway/CLI
  caller); the "model proposes set inside the loop" path rides the same unbuilt in-house-tool-
  surfacing follow-up. Recorded, not faked.

## Tests

`crates/host/tests/agent_memory_test.rs` (8, rule 9 — real store/gateway/loop):
- **per-verb capability deny** (no cap → denied; `list`-only doesn't grant `set`);
- **workspace-scope write gate distinct** (member `set` ok, workspace `set` denied without the gate,
  ok with it);
- **member wall** (bob's list/get never returns `member:ada` even with the slug known);
- **workspace isolation** (ws-B sees nothing of ws-A, store + MCP);
- **idempotent upsert** (second `set` same slug replaces, one index row, LWW);
- **bounds + secret lint** (desc > 120, body > 8 KB, unknown kind, `password:`/`sk-` refused, prose
  mention of "password" allowed);
- **MCP surface roundtrip** (set→get→list→delete over `call_agent_memory_tool`, no body in list rows)
  + per-verb MCP deny;
- **real in-house run injects the index after `set`, framed as recalled background, and loses it
  after `delete`** (capturing Provider).

Green output:

```
agent_memory_test:  ok. 8 passed; 0 failed
```

Full `cargo test --workspace` + `cargo fmt` green (see STATUS).

## Debugging

None opened (the injection-principal issue was caught + fixed within the session by the failing
injection test — a design fix, not a shipped-then-broken bug; captured in Decisions above).

## Public / scope updates

- `docs/public/agent-memory/agent-memory.md` filled.
- Scope open questions were "Decided" (index overflow → inject-cap-only; external `set` opt-in; one
  shared pool; UI freshness = plain list) — all realized.

## Skill docs

- `docs/skills/agent-memory/SKILL.md` written, grounded in a live `lb local call` run (set →
  `{scope:"member:user:ada"}`, secret-lint rejection, delete → `{ok}`).

## Dead ends / surprises

- The derived agent principal's sub is `agent:session`, not the caller's — so a naive
  member-scope resolution under it misses the caller's own memory. The on-behalf-of principal (the
  `substrate.rs` pattern) is the fix. Caught by the injection test.

## Follow-ups

- Surface `agent.memory.*` as model-proposable tools to the in-house loop (needs the channel worker
  to pass a tool list + the loop to reach host-native verbs) — the shared in-house-tool-surfacing gap.
- Vector/semantic recall (scope non-goal for v1).
- STATUS.md updated: yes.
