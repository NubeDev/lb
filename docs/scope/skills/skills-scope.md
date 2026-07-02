# Skills scope — versioned, grant-gated workspace assets

Status: scope (the ask). Promotes to `public/skills/` once the S4 slice ships.

A **skill** is a reusable instruction/recipe asset (a prompt, a checklist, a tool recipe) that a
central or local AI agent **loads only when the workspace has granted it** (README §6.12). Skills
are the same workspace-asset shape as docs (`../files/files-scope.md`) with two additions: they
are **versioned** (a skill is `{id}@{version}`, rollback = a prior version), and they "load" only
behind an explicit **grant** — a skill that exists but is not granted is invisible to the agent.

> Read with: `../../README.md` §6.12 (docs/skills as assets), §6.16 (shared AI agents load skills),
> §6.4 (versioned assets + rollback), `../files/files-scope.md` (the asset/sharing substrate this
> reuses), `../auth-caps/auth-caps-scope.md` (the grammar), `../mcp/mcp-scope.md` (the tool surface).

---

## Goals

- A **skill asset**: workspace-scoped, addressed by `(id, version)`, with a body + metadata
  (author, ts, description). Immutable per version (a change is a new version).
- **Grant-gated load**: `load_skill(id)` returns the skill **only if the workspace has granted
  that skill** — a `grant:skill/{id}` relation record. An ungranted skill is denied, even to a
  principal holding the skill-read capability. This is the §6.12 "load only when granted" rule.
- **A skill loads only when granted** is the S4 exit-gate clause for skills — the mandatory deny
  test at the grant layer.
- **Workspace isolation**: workspace B can never read/list/load workspace A's skills.

## Non-goals (S4)

- **Executing** a skill (running its recipe/tool-loop) — that's the AI agent's job at S5
  (§6.16). S4 ships *load/grant/version*; the agent that consumes a loaded skill is later.
- **Skill sharing to a team / linking to a channel** — skills reuse the doc sharing machinery if
  needed, but the S4 skill story is **workspace-grant** (the whole workspace either granted the
  skill to its agents or didn't). Team-scoped skills are a follow-up; not built now.
- **A registry / signed distribution of public skills** — S7 (§6.4). S4 skills are authored in
  the workspace.
- **Skill compatibility / dependency resolution** — version is a label + rollback target at S4;
  semver compatibility checks are deferred.

## Intent / approach

Reuse the doc-asset substrate (`../files/`) and add the two skill-specific notions:

- **Versioning** — the record id is `skill:{id}@{version}` (immutable per version). `latest`
  resolution is a `list` over `{id}@*` picking the highest version, or an explicit pinned version.
  Rollback (§6.4) is granting/loading a prior version — no special machinery, the prior version's
  record still exists.
- **The grant gate** — instead of the doc's three-mode visibility, a skill has one gate beyond the
  workspace+capability check: a **`grant:skill/{id}`** relation record in the workspace. `load_skill`
  resolves it: no grant → **denied**. Granting is an admin act (a host verb / MCP tool with its own
  capability); it is the workspace saying "our agents may use this skill."

So the gate stack mirrors docs, with the membership gate replaced by a grant gate:

1. **Workspace wall (gate 1)** — the skill record lives in the workspace namespace.
2. **Capability gate (gate 2)** — `store:skill/*:read` to use the skill surface at all.
3. **Grant gate (gate 3)** — `grant:skill/{id}` must exist for `load_skill` to return the body.

**Why a grant *relation*, not a granted-skills list in the token.** Same reasoning as docs: a
revocable relation beats a grant baked into a JWT you must re-mint. The workspace grants/revokes by
writing/deleting one record; the next `load_skill` reflects it immediately. **Rejected:** a
`skill:{id}:load` capability minted per grant — that conflates "may use the skill surface" (a
capability) with "this workspace has adopted this skill" (a workspace policy fact); keeping them
separate means an agent's token is stable while the workspace's skill set evolves underneath it.

## How it fits the core

- **Tenancy / isolation:** skill records are workspace-namespaced (gate 1, structural). The grant
  gate (gate 3) is a workspace-internal policy, not a cross-workspace concern — a grant in ws A
  never affects ws B.
- **Capabilities (deny path):** `store:skill/*:read` gates the surface. Deny tests: (a) no cap →
  denied; (b) cap but **no grant** for the skill → `load_skill` denied (the §6.12 "only when
  granted" rule — the mandatory skill deny).
- **Placement:** `either`. Skills are shared workspace data → hub-authoritative, edge read-cache,
  §6.8 append-style sync (immutable versioned records never conflict — the easiest sync case).
- **MCP surface:** `assets.put_skill`, `assets.load_skill`, `assets.grant_skill`,
  `assets.list_skills` — reached identically by UI/agent/extension.
- **Data (SurrealDB):** `skill:{id}@{version}` (immutable body+meta) ; `grant:skill/{id}` (the
  workspace grant relation). Workspace namespace, existing store primitives. State only.
- **Bus (Zenoh):** none — a skill is state, loaded on demand.
- **Sync / authority:** hub-authoritative; immutable versioned records → idempotent apply, never
  contested. (Same mechanism as channel items; not re-proven at S4.)
- **State vs motion / one datastore / stateless extensions:** ✔ — body in SurrealDB, host verbs,
  no extension state.

## Example flow

1. Ada authors a skill: `put_skill("coding-scope-writer", version="1.0.0", body="…")`. It exists
   in ws `acme` but no agent can load it yet — there is no grant.
2. A workspace admin calls `grant_skill("coding-scope-writer")` → writes `grant:skill/coding-scope-writer`.
3. The central AI agent (holding `store:skill/*:read`) calls `load_skill("coding-scope-writer")`.
   Gate 1 ws ✔, gate 2 cap ✔, gate 3 grant exists ✔ → **the body loads.**
4. Before the grant existed (or after a revoke), the same `load_skill` is **denied** — the skill is
   invisible until/unless the workspace grants it (§6.12).
5. Ada publishes `coding-scope-writer@1.1.0`. The grant resolves to latest; rollback is loading
   `@1.0.0` explicitly (its record never went away).

## Testing plan (mandatory categories apply)

- **Capability-deny (mandatory, §2.1):** `assets/tests/skill_deny_test` —
  (a) no `store:skill/*:read` cap → `load_skill` denied; (b) cap but **no grant** → `load_skill`
  denied (the §6.12 / S4 exit-gate "a skill loads only when granted").
- **Workspace-isolation (mandatory, §2.2):** `assets/tests/skill_isolation_test` — ws-B cannot
  `load_skill`/`list_skills` a ws-A skill, across **store + MCP**.
- **Grant→load happy path + versioning:** grant then load returns the body; a second version is
  loadable; rollback loads the prior version; revoke makes load deny again.

## Risks & hard problems

- **Grant vs capability confusion.** The clearest trap: making "granted" a capability. Keep them
  distinct (capability = may use the surface; grant = workspace adopted this skill). The deny test
  with-cap-but-no-grant is what guards the distinction.
- **Latest-version resolution** is a `list` + max; cheap now, but if skills proliferate a pinned-id
  index is the scale answer. Flagged, not built.
- **Immutability enforcement.** A `put_skill` to an existing `{id}@{version}` should be rejected or
  be a no-op (versions are immutable). The verb must not silently overwrite a published version.

## Open questions

- **Team-scoped skills** — reuse the doc team-share path so a skill can be granted to a team rather
  than the whole workspace? Decide when the first multi-team skill case appears (S5 agents).
- **Who may grant** — `grant_skill` needs its own capability/role (workspace-admin). S4 gates it
  behind a cap; the role-grant flow is the auth-caps delegation open question (S5).
- **Skill body schema** — free text at S4. When skills carry structured tool-recipes (§6.16), a
  schema + validation lands with the agent that executes them (S5).
- **Latest vs pinned default** — does `load_skill(id)` default to latest-granted or require a
  version? S4: latest-granted, with an optional explicit version for rollback.

## Related

- README `§6.12` (docs/skills as assets), `§6.16` (agents load skills), `§6.4` (versioned assets +
  rollback).
- Follow-up scope: [`core-skills-scope.md`](core-skills-scope.md) — the developer-authored core
  tier + user CRUD (deprecate) + the agent-facing catalog.
- Sibling scopes: `../files/files-scope.md` (the asset substrate skills reuse),
  `../auth-caps/auth-caps-scope.md`, `../mcp/mcp-scope.md`, `../extensions/extensions-scope.md`.
- Public (on ship): `../../public/skills/skills.md`.
