---
name: skills
description: >-
  Manage the two-tier skill catalog an AI agent draws on — developer-authored CORE skills (shipped
  with the node, seeded at boot, read-only) and workspace-authored USER skills (full CRUD). Author a
  user skill (`assets.put_skill`), adopt one for the workspace's agents (`assets.grant_skill`), list
  the agent-facing catalog with tiers (`assets.list_skills`), pull a body (`assets.load_skill`), soft-
  delete a user skill (`assets.deprecate_skill`), and revoke a grant (`assets.revoke_skill`). Use when
  a task says "what skills can the agent use", "grant/revoke a skill", "publish a skill version",
  "deprecate a skill", "why can't the agent load a skill", or "add a core skill". A skill loads ONLY
  when the workspace granted it — the grant is the wall, identical for both tiers.
---

# Managing the skill catalog (`assets.*_skill`, two tiers, one gate)

A **skill** is an instruction/recipe asset an AI agent loads to do a job. Lazybones has **two tiers**,
both reached through the *same* grant gate and the *same* `load_skill`:

- **Core skills** — developer-authored, shipped with the node, seeded at boot as immutable
  `skill:core.<name>@<node-version>` records in a reserved system namespace. Ids start `core.`
  (`core.lb-cli`, `core.query`, …). **Read-only to users** — they change only by shipping a new node
  build. There are 17 today (the `docs/skills/*/SKILL.md` corpus embedded at build time).
- **User skills** — workspace-authored, full CRUD (`put`/`grant`/`load`/`deprecate`/`revoke`),
  versioned, live in the workspace namespace.

**The grant is the wall (both tiers).** A skill that exists — even a core skill present on every
node — is invisible to the agent until the workspace **granted** it (`grant:skill/{id}`). No core
bypass: an ungranted `core.lb-cli` denies exactly like an ungranted user skill. Granting is a
workspace policy act (revocable); the capability (`store:skill/{id}:*`) says "may use the skill
surface", the grant says "this workspace adopted this skill". They are separate on purpose.

All verbs funnel through the one MCP contract (`lb call assets.<verb>` / `POST /mcp/call`); the
server is the enforcement, the CLI holds no authority.

## Capabilities you need

| Verb | MCP gate | Store gate |
|---|---|---|
| `assets.list_skills` | `mcp:assets.list_skills:call` | `store:skill/**:read` |
| `assets.load_skill` | `mcp:assets.load_skill:call` | `store:skill/**:read` + the **grant** |
| `assets.put_skill` | `mcp:assets.put_skill:call` | `store:skill/**:write` (rejects `core.*`) |
| `assets.grant_skill` | `mcp:assets.grant_skill:call` | `store:skill/**:write` |
| `assets.revoke_skill` | `mcp:assets.revoke_skill:call` | `store:skill/**:write` |
| `assets.deprecate_skill` | `mcp:assets.deprecate_skill:call` | `store:skill/**:write` (rejects `core.*`) |

> **Use `**`, not `*`.** A core id contains a `.` (`core.lb-cli`), and the caps grammar splits a
> resource on `/` **and** `.`. So `store:skill/*` (one segment) does NOT cover `skill/core.lb-cli` —
> grant the recursive tail `store:skill/**` to reach core (and it still covers flat user ids). The dev
> login grants `store:skill/**`.

## Core skills are seeded at boot

A node seeds the embedded corpus once at boot (idempotent — a re-seed of an immutable version is a
no-op; a node upgrade seeds the new versions and keeps the old for rollback). The boot log
(grounded live, `cargo run -p node`):

```
boot: seeded N core skills @0.1.0 (["core.auth-caps", "core.channels-inbox-outbox", …,
      "core.lb-cli", "core.mcp", "core.prefs", "core.query", "core.e2e-backend", "core.store-read",
      "core.tags"])
boot: default core-skill grants for ws=acme: ["core.lb-cli", "core.query", "core.store-read"]
```

> **The corpus is the whole `docs/skills/` tree + the `docs/testing/**` e2e runbooks — no allow-list.**
> The build script scans **every** `docs/skills/<name>/SKILL.md` (a new dir auto-seeds as
> `core.<name>`) and every frontmatter-bearing `docs/testing/**/*.md` runbook (seeds as `core.e2e-*`).
> **Anti-rot gate (agent-personas #2):** a `docs/skills/` subdir missing its `SKILL.md` **fails the
> build** — a half-authored skill can never silently ship as "absent" (which would fail-close a persona
> that pins it at run time with no build-time signal). So the seeded count is exactly the on-disk
> corpus; don't hardcode it.

A **fresh workspace** gets a small **default grant set** — the read-only core skills `core.lb-cli`,
`core.query`, `core.store-read` — so its agent is useful out of the box. An admin can revoke any of
them like any other grant (`assets.revoke_skill`), or widen the boot set with the node config
`LB_DEFAULT_CORE_SKILLS` (comma-separated ids; empty = none).

## The agent-facing catalog — `assets.list_skills`

The one catalog the agent sees at session start: **id + latest + description + tier + granted**, never
the body (bodies load on demand). Only granted skills appear.

```bash
lb call assets.list_skills '{}' -o json
```
```json
{ "skills": [
  { "id": "core.lb-cli",     "latest": "0.1.0", "tier": "core", "granted": true,
    "description": "Operating a node from the terminal (lb) …" },
  { "id": "core.query",      "latest": "0.1.0", "tier": "core", "granted": true, "description": "…" },
  { "id": "core.store-read", "latest": "0.1.0", "tier": "core", "granted": true, "description": "…" },
  { "id": "acme-runbook",    "latest": "1.1.0", "tier": "user", "granted": true, "description": "…" }
] }
```

The agent's run context is seeded with exactly this (name + description only). What it can *see and
load* is computed under the **derived principal** (`caller ∩ agent`), so a user's run can never browse
skills the user couldn't. A caller lacking `store:skill/**:read` gets an **empty catalog** and every
`load_skill` denies — the agent is exactly as smart as the caller is allowed.

## Author + grant a user skill

```bash
# 1. Publish a version (immutable — re-publishing the same {id}@{version} is rejected).
lb call assets.put_skill \
  '{"id":"acme-runbook","version":"1.0.0","description":"Acme deploy runbook","body":"1. …","ts":1}'
# → { "id": "acme-runbook", "version": "1.0.0" }

# 2. Adopt it for the workspace's agents.
lb call assets.grant_skill '{"id":"acme-runbook"}'      # → { "ok": true }

# 3. The agent (or you) pulls the body on demand.
lb call assets.load_skill '{"id":"acme-runbook"}' -o json
# → { "id":"acme-runbook", "version":"1.1.0", "body":"…" }   # latest granted version
lb call assets.load_skill '{"id":"acme-runbook","version":"1.0.0"}'   # pinned (rollback)
```

## Deprecate (soft delete) + un-hide

Versions are immutable and rollback-bearing, so a skill is never hard-deleted. `deprecate_skill`
**hides** the id from `list_skills`/latest resolution — but a **pinned** load of an old version still
resolves (audit + rollback preserved). **Re-publishing a new version un-hides** it.

```bash
lb call assets.deprecate_skill '{"id":"acme-runbook"}'   # → { "ok": true }; gone from list/latest
lb call assets.load_skill '{"id":"acme-runbook","version":"1.0.0"}'   # still resolves (pinned)
lb call assets.put_skill '{"id":"acme-runbook","version":"1.1.0", …}'  # a new version un-hides it
```

Revoke drops the grant entirely (the id vanishes from the catalog until re-granted):

```bash
lb call assets.revoke_skill '{"id":"acme-runbook"}'      # → { "ok": true }
```

## Core skills are read-only — rejected regardless of caps

`put_skill` / `deprecate_skill` on any `core.*` id are **rejected even for a workspace admin** holding
`store:skill/**:write` — a clear, non-opaque error (not a caps deny), grounded live:

```bash
lb call assets.put_skill '{"id":"core.lb-cli","version":"9.9.9","description":"x","body":"x","ts":1}'
# DENIED / error: core skills are read-only to users (reserved namespace)
```

To change a core skill, edit its `docs/skills/<name>/SKILL.md` and ship a new node build — the boot
seeder writes the new immutable version.

## Gotchas

- **`load_skill` denies but the skill exists?** It isn't granted (or the caller lacks
  `store:skill/**:read`). Grant it (`assets.grant_skill`) — the grant is the wall for both tiers.
- **`store:skill/*` doesn't reach a core skill.** Use `**` (see the cap note above). This is the one
  non-obvious trap; a `*` grant silently under-matches a dotted core id.
- **`list_skills` empty in `lb local`?** The CLI's `local` mode boots its own node and does NOT run
  the node binary's boot seeder, so core skills aren't seeded there. List/load against a running
  `node` (remote), where boot seeded them.
- **The catalog never carries a body.** Bodies load on demand via `load_skill`; `list_skills` is the
  cheap descriptor surface (it pays a per-run token cost, so it stays small).
- **Prompt-injection stance:** a granted skill body enters the agent's context, but the **wall** (caps
  + sandbox), not the skill text, is what constrains the agent. The persona steers; the wall
  constrains. Skills change *quality*, never *authority*.

## Related

- Public: `../../public/skills/skills.md`. Scope: `../../scope/skills/core-skills-scope.md`,
  `../../scope/skills/skills-scope.md`.
- Sibling: `../agent-memory/SKILL.md` (learned knowledge; same enforcement thesis).
- The agent that consumes the catalog: `../external-agent/SKILL.md`, `../../scope/agent/agent-scope.md`.
