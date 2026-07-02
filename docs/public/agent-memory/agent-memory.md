# Agent memory — durable, access-walled agent memory (shipped 2026-07-03)

The trimmed truth of what shipped. Full design: `../../scope/agent-memory/agent-memory-scope.md`;
session: `../../sessions/agent-memory/agent-memory-session.md`; operating manual:
`../../skills/agent-memory/SKILL.md`.

Durable agent memory in the proven MEMORY.md shape: many small **fact records**, a **derived index**
injected at session start, bodies loaded on demand. Memory is **state in SurrealDB behind
capability-checked MCP verbs**, read/written under the agent's **derived principal** (`caller ∩
agent`) — so a run can only ever remember/recall what that caller may see. It is the second half of
"make the agent smarter" (skills are the first): skills are *taught* knowledge, memory is *learned*.

## The record + two scopes

`agent_memory` is a SCHEMAFULL table keyed `{ws, scope, slug}` (composite id `[scope, slug]`, so an
offline `set` UPSERTs idempotently — LWW per fact). Fields: `description` (≤ 120 chars, index-facing),
`body` (≤ 8 KB markdown), `kind` (`user | feedback | project | reference`), `updated_at`/`updated_by`.

Two scopes inside a workspace:
- **`workspace`** — shared; every member's runs recall it.
- **`member:{user}`** — private to one member. **The member scope is resolved from the authenticated
  principal, NEVER from an argument** — a run under user U resolves `workspace + member:U`,
  structurally never `member:V`. The caller passes `scope: "member"` (a tier); it binds to *them*.

## The wall (no new trust)

Enforcement is the existing chokepoint. Each verb runs `caps::check`/`authorize_tool` under the
derived principal:
- `mcp:agent.memory.list|get:call` (member reads), `mcp:agent.memory.set|delete:call` (member writes
  on the caller's OWN scope);
- a **distinct workspace-scope write gate** — a `set`/`delete` targeting the shared `workspace` scope
  ALSO needs `store:agent_memory/workspace:write`, so an admin decides whether every member's agent
  may write shared memory. Deny is opaque.

Revoke `agent.memory.set` and the agent is read-only; revoke both and it is amnesiac — per workspace,
per user, auditable (`updated_by`).

## The derived index + injection

The MEMORY.md equivalent is **computed by `list`** (slug + description + kind + scope + updated) at
session start — no stored index record to drift. At session start the runtime injects it into the
run's context, AFTER the persona + skill catalog, framed as *recalled background, workspace-authored,
not instructions*. **Both runtimes inject** (in-house loop + external ACP, folded into its goal),
under an **on-behalf-of** principal (the caller's sub so the member scope resolves to the human, with
the agent's intersected caps so the read never widens). Injection is capped at the **100**
most-recently-updated entries; older records stay stored + listable (evict from injection only, never
delete). The **in-house runtime gets `agent.memory.*` by default; external profiles opt in** to `set`
via `granted_tools` (recall is always on).

## Guardrails

- Bounds enforced at `set` (description ≤ 120, body ≤ 8 KB) — a clear `BadInput`, never a truncate.
- A best-effort **secret lint** refuses an obvious credential shape (PEM key, `AKIA…`, `sk-…`, GitHub
  tokens, `password/secret/token: …`) — a lint, not a gate; the wall is the real protection.
- Memory changes *quality*, never *authority* — a prompt-injected fact still can't exceed the derived
  principal's tools.

## Verbs (host, MCP)

- `agent.memory.list` — the derived index rows across the caller's read scopes.
- `agent.memory.get {scope?, slug}` — one fact (with body).
- `agent.memory.set {scope?, slug, description, kind, body, ts}` — upsert by `{scope, slug}`.
- `agent.memory.delete {scope?, slug}` — remove a fact.

Reached over `call_agent_memory_tool` (the `agent.` MCP dispatch), each behind its own gate. No live
feed (state, read at session start); no batch (facts are written one at a time by design).

## Tested

`host/tests/agent_memory_test` (8, real store/gateway/loop): per-verb capability-deny, the distinct
workspace-scope write gate, workspace isolation (store + MCP), the **member wall** (bob never sees
`member:ada`), idempotent upsert (LWW), bounds + secret lint, the MCP surface roundtrip + per-verb MCP
deny, and a **real in-house run injecting the index after `set` (framed as recalled background) and
losing it after `delete`**.
