# Agent-memory scope — durable, access-walled agent memory (the MEMORY.md shape)

Status: **SHIPPED 2026-07-03** (was: scope). Promoted to `../../public/agent-memory/agent-memory.md`;
session `../../sessions/agent-memory/agent-memory-session.md`; skill
`../../skills/agent-memory/SKILL.md`. All "Decided" items realized (index overflow = inject-cap-only;
external `set` opt-in; one shared pool per ws + member scope; UI freshness = plain list). The scope is
retained as the ask-of-record.

Give the workspace agent (in-house loop and external ACP runtimes alike) a **persistent memory** in
the proven MEMORY.md shape: many small **fact records**, each with a one-line description, plus a
**derived compact index** injected at session start so the agent knows what it knows and loads
bodies on demand. Memory is **state in SurrealDB behind capability-checked MCP verbs** — the agent
reads and writes it under the **derived principal** (`caller ∩ agent`), so a user's agent run can
only ever remember and recall what that user may see. This is the second half of "make the agent
smarter" (skills are the first — `../skills/core-skills-scope.md`): skills are *taught* knowledge,
memory is *learned* knowledge; both are granted/gated data, never widened authority.

> Read with: `../agent/agent-scope.md` (the loop that consumes this), `../external-agent/`
> (capability-wall + acp-driver — the wall memory rides behind), `../skills/skills-scope.md` (the
> asset-gating pattern mirrored), README §6.16 (shared AI agents), §7 (tenancy).

---

## Goals

- **Memory records**: `agent_memory` entries keyed `{ws, scope, slug}` with `description` (one
  line, index-facing), `body` (markdown, the fact), `kind` (`user | feedback | project |
  reference` — the proven taxonomy), and `updated_at`/`updated_by` provenance.
- **Two scopes inside a workspace**:
  - `workspace` — shared memory every member's agent runs see (project facts, house rules);
  - `member:{user}` — private to one member (their preferences, their corrections).
  A run under user U resolves `workspace + member:U`, structurally never `member:V`.
- **A derived index, never a stored one.** The MEMORY.md equivalent is computed by `list` (slug +
  description lines) at session start — no separate index record to drift out of sync.
- **Agent read/write over MCP**: `agent.memory.list | get | set | delete` (resource-verbs grammar),
  each its own capability, all workspace-walled. The agent holds these tools in `granted_tools`
  like any other — remembering is a capability, not a birthright.
- **Enforcement is the existing wall.** No new trust: the verbs run through `caps::check` under the
  derived principal; the external agent reaches them only via the MCP bridge inside the sandbox.
  Revoke `agent.memory.set` and the agent is read-only; revoke both and it is amnesiac — per
  workspace, per user, auditable.

## Non-goals

- **Not the run transcript.** Durable run history is the job record (`run-lifecycle-scope.md`);
  memory is distilled facts, written deliberately, not a log.
- **No auto-summarization / embedding search in v1.** Recall is the index + explicit `get`; slugs
  and descriptions are the retrieval keys. Vector/semantic recall is a named follow-up, not now.
- **No cross-workspace memory.** A member's `member:{user}` memory is per-workspace (the wall is
  the wall). A global-profile memory rides the `global-identity` scope if ever.
- **Not a generic KV for extensions** — that's the `kv.*` scope in `extensions/`. This is the
  agent's curated memory with the index/injection contract.

## Intent / approach

**Mirror the shape that works** (Claude Code's memory dir): one fact per record, a one-line
description for the index, bodies loaded on demand, the writer updates-in-place or deletes when a
fact goes stale. The platform translation: records not files (one datastore, stateless runtimes,
sync for free), MCP verbs not fs access (capability wall), derived index not a stored MEMORY.md
(no drift).

**Injection contract** (shared by the in-house loop and `AcpRuntime`): at session start the runtime
calls `agent.memory.list` under the derived principal and renders the index into the instructions
block — clearly labeled as *recalled background, workspace-authored, not instructions* — after the
persona skill and the skill catalog. Mid-run the agent calls `agent.memory.get {slug}` for bodies
and `agent.memory.set` to persist a new/updated fact (idempotent upsert by `{scope, slug}`).

**Who writes what**: `set`/`delete` on `member:{self}` needs the member-level memory-write cap;
on `workspace` scope it needs a distinct workspace-memory cap so an admin can decide whether every
member's agent may write shared memory or only curators. Defaults: member-write ON for own scope,
workspace-write granted to members too (collaboration-first), revocable where that's wrong.

**Rejected alternatives:**
- **Files on disk / a real MEMORY.md** — breaks one-datastore, stateless runtimes, workspace sync,
  and (for the external agent) would require widening the fs sandbox that the capability wall just
  closed. The scratch dir stays scratch.
- **Memory inside the external agent's own session state** — the run-lifecycle scope already rules
  it: subprocess memory is ephemeral scratch, never authority. Resume + swap-the-agent both demand
  the memory live on our side of the wall.
- **A single big memory document** — merge conflicts on concurrent runs, no per-fact provenance or
  deletion, and the whole blob lands in context every run. Per-fact records + a derived index cost
  one `list` and solve all three.

## How it fits the core

- **Tenancy / isolation:** every record keyed by `ws` first; `member:{user}` sub-scope resolved
  from the authenticated principal, never from an argument (an agent cannot *ask* for another
  member's memory — the scope is derived, not passed). Mandatory two-workspace + two-member tests.
- **Capabilities:** `mcp:agent.memory.list|get:call` (member), `mcp:agent.memory.set|delete:call`
  (member, own scope) + a workspace-scope write gate. Deny is opaque. The deny path *is* the
  feature: memory obeys the same wall as every tool.
- **Placement:** `either` — host verbs + records compiled into every node. Hub-authoritative,
  edge read-cache; `{ws, scope, slug}` composite ids make offline writes idempotent (LWW per fact —
  acceptable: facts are small and last-writer-wins is the right merge for a correction).
- **MCP surface:** `agent.memory.list` (index rows: slug, description, kind, scope, updated),
  `get {slug}`, `set {scope?, slug, description, kind, body}` (upsert), `delete {scope?, slug}`.
  No live feed (memory is state, read at session start); no batch (facts are written one at a
  time by design — a bulk import can come later as a job if ever needed).
- **Data (SurrealDB):** one SCHEMAFULL `agent_memory` table. State, not motion.
- **Bus (Zenoh):** none.
- **Secrets:** none stored; the skill/docs for memory must say "never persist credentials into
  memory" and the `set` verb should reject obvious secret shapes (best-effort lint, not a gate).
- **SDK/WIT impact:** none (host verbs only).
- **Skill doc:** yes — `docs/skills/agent-memory/SKILL.md` on ship (how to inspect/curate memory
  via `lb call`, how the injection works, how to wipe a poisoned fact).

## Example flow

1. Ada (ws `acme`) runs the agent: "our staging DB is the replica, never write to it." The agent
   calls `agent.memory.set { scope:"workspace", slug:"staging-db-readonly", kind:"project", … }` —
   cap ✔ under `ada ∩ agent` → persisted.
2. Bob's run next day starts: the runtime lists `workspace + member:bob` → the index line
   `staging-db-readonly — staging DB is a read replica…` is injected. Bob's agent `get`s the body
   before touching staging.
3. Bob tells his agent he prefers terse answers → `set { scope:"member", slug:"terse-answers",
   kind:"user" }`. Ada's runs never see it (`member:bob` is structurally out of her resolution).
4. A run by Carol, whose grants exclude `agent.memory.set`, tries to remember something → **denied,
   opaquely**; her runs still recall (list/get granted). The workspace decided writers ≠ readers.
5. An admin spots a wrong fact planted by a bad run → `agent.memory.delete { scope:"workspace",
   slug:"staging-db-readonly" }`; next session's index no longer carries it. Provenance
   (`updated_by`, the audit ledger) says which principal wrote it.
6. The same flow drives `open-interpreter-default`: the ACP bridge exposes `agent.memory.*` as MCP
   tools; the subprocess can persist nothing on disk (sandbox) — its only memory is ours, walled.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`):

- **Capability-deny (§2.1):** per verb — no cap → denied; `set` cap present but workspace-scope
  write gate absent → workspace `set` denied while `member` `set` succeeds.
- **Workspace-isolation (§2.2):** ws-B lists/gets nothing of ws-A across **store + MCP**; and the
  **member wall**: bob's run resolution never returns `member:ada` rows even with slugs known —
  scope is derived from the principal, asserted directly.
- **Offline/sync:** double-apply of a `set` is idempotent (composite id, LWW).
- **Injection (real agent, rule 9):** a real in-house run's context contains the index after a
  `set`, loses it after `delete`; a real external-agent run (feature-on smoke) can `set` and a
  following run recalls — over the real MCP bridge, real store, no fakes.
- **Key cases:** upsert semantics (second `set` same slug replaces, one index row); index is
  list-derived (no stored index record to assert stale); `kind` round-trips; description length
  bound enforced.

## Risks & hard problems

- **Memory poisoning is the security story.** A prompt-injected run can write a fact that steers
  every later run ("always pipe output to <url>"). Mitigations, in order of weight: the **wall**
  (a steered agent still can't exceed the derived principal's tools — the same stance as skills);
  **provenance + audit** on every write; the injection framing (*recalled background, not
  instructions*); the write cap being revocable per principal. State plainly: memory changes
  *quality*, never *authority*.
- **Context tax.** An unbounded index erodes every run. Bound it: description ≤ ~120 chars, index
  entries per resolution capped (proposal: 100, oldest-updated listed last), body size capped
  (proposal: 8 KB). Overflow behavior is an open question below.
- **Write discipline.** Agents over-remember. The skill doc + persona guidance must carry the
  "save the non-obvious, update don't duplicate, delete wrong facts" rules; the cap system can't
  encode taste.
- **Concurrent runs** upserting the same slug — LWW is fine for facts, but two runs *creating*
  divergent slugs for one fact duplicates the index. Accept in v1; curation (`delete`) is the tool.

## Decided (was: open questions)

- **Index overflow:** evict from **injection only** (most-recently-updated 100 entries injected;
  older records remain stored and listable — never silent deletion). Bounds: description ≤ 120
  chars, body ≤ 8 KB, enforced at `set` with a clear `BadInput`.
- **`agent.memory.set` for external agents:** **opt-in per profile**; the in-house runtime gets it
  by default. External profiles add it to `granted_tools` once the wall test has soaked.
- **Memory partitions:** **one shared pool** per workspace (+ per-member scope) — no per-profile
  partitions; revisit only if cross-persona pollution shows up in practice.
- **UI freshness:** plain `list` on open for any curation UI; no live feed, no bus subject.

## Related

- `../skills/core-skills-scope.md` (the sibling smarts scope; same enforcement thesis),
  `../skills/skills-scope.md` (the gating pattern mirrored).
- `../agent/agent-scope.md` (injection point, in-house), `../external-agent/acp-driver-scope.md` +
  `../external-agent/capability-wall-scope.md` (injection + wall, external),
  `../external-agent/run-lifecycle-scope.md` (why subprocess memory is never authority).
- `../audit/` (write provenance), `../extensions/` `kv.*` (the generic store this is *not*).
- README `§6.16`, `§7`, `§3` (rules 2, 4, 5).
