# Skills — versioned, grant-gated workspace assets (S4) + the core tier (2026-07-03)

The trimmed truth of what shipped. Full design: `../../scope/skills/skills-scope.md` +
`../../scope/skills/core-skills-scope.md`; sessions: `../../sessions/files/shared-assets-session.md`
+ `../../sessions/skills/core-skills-session.md`. Operating manual: `../../skills/skills/SKILL.md`.

A **skill** is a reusable instruction/recipe asset that an AI agent **loads only when the workspace
has granted it** (README §6.12). Same asset substrate as docs (`../files/files.md`), plus two
skill-specific notions: **versioning** and the **grant gate**. There are **two tiers**, both behind
the same grant gate and the same `load_skill` (see "Core tier" below).

## Versioning

A skill is addressed `skill:{id}@{version}` and is **immutable per version** — a change is a new
version (`put_skill` rejects republishing an existing `{id}@{version}`). Rollback (§6.4) is loading
a prior version, whose record never went away. `load_skill(id, None)` resolves the latest published
version; `load_skill(id, Some(v))` pins.

## The grant gate (load only when granted)

Beyond the workspace + `store:skill/{id}:read` capability, `load_skill` checks a
**`grant:skill/{id}`** relation: the workspace saying "our agents may use this skill." No grant →
**denied**, even with the read capability — the §6.12 "load only when granted" rule. Granting is a
revocable relation, not a capability minted into a token: `grant_skill`/`revoke_skill` write/delete
one record, and the next `load_skill` reflects it immediately.

## Core tier — developer skills, shipped with the node (2026-07-03)

A **second tier** alongside the workspace-authored user tier: **core skills** are authored in-repo
(`docs/skills/*/SKILL.md`), **embedded into the node binary at build time**, and **seeded at boot**
as immutable `skill:core.<name>@<node-version>` records in a **reserved system namespace**
(`_lb_skills` — the `_lb_identity`/`_lb_workspaces` precedent). Ids are `core.<name>`. 17 ship today.

- **One substrate, one gate.** Core skills are ordinary `skill:{id}@{version}` records; the only
  differences are *where* they live (the reserved namespace) and *who* writes them (the boot seeder,
  the sole writer). `load_skill`/`list_skills` resolve `core.*` against the reserved namespace **after
  the same workspace grant check** — no core bypass. An ungranted core skill denies exactly like an
  ungranted user skill.
- **Read-only to users.** `put_skill`/`deprecate_skill` on any `core.*` id are **rejected regardless
  of caps** (a clear, non-opaque error) — core skills change only by shipping a new node build. Boot
  is idempotent (immutable version = no-op); an upgrade seeds new versions and keeps old for rollback.
- **Default grant set at workspace creation.** A fresh workspace is granted the read-only core skills
  `core.lb-cli`, `core.query`, `core.store-read` (node config `LB_DEFAULT_CORE_SKILLS`; empty = none)
  so its agent is useful out of the box — each a revocable `grant:skill/{id}` edge.

## The agent catalog (`assets.list_skills`) + injection

`list_skills` returns the **granted** skills as `{id, latest, description, tier, granted}` rows —
never the body (bodies load on demand). The runtime injects this compact catalog (name + description
only) into the agent's context at session start under the derived principal (`caller ∩ agent`), for
**both** runtimes: the in-house loop injects once per run (and the model can `skill.activate`
mid-run); the external ACP runtime folds it into the goal (its only channel). A caller lacking
`store:skill/**:read` gets an **empty catalog** and denied loads. The persona rides the same
grant-gated `load_skill` loader — one loader, not two.

## Deprecate = soft delete (user tier)

`assets.deprecate_skill(id)` hides a user skill's id from `list_skills`/latest resolution while a
**pinned** load of an old version still resolves (audit + rollback). Re-publishing a new version
**un-hides** it (deprecate is a state, not a tombstone). Backed by a `skill_meta:{id}` flag.
Requires `store:skill/{id}:write`; rejected on `core.*`.

## Verbs (host)

- `put_skill(store, principal, ws, id, version, description, body, ts)` — publish an immutable
  version. Requires `store:skill/{id}:write`; **rejects `core.*`**.
- `grant_skill` / `revoke_skill(store, principal, ws, id)` — the workspace grant relation.
- `deprecate_skill(store, principal, ws, id)` — soft-hide a user skill. Rejects `core.*`.
- `load_skill(store, principal, ws, id, version?)` — the grant-gated load (cap + grant); resolves
  `core.*` against the reserved namespace.
- `list_granted_skills(store, principal, ws)` — the tiered catalog (id + latest + description + tier).
- `seed_core_skills(store, version, ts)` — the boot seeder (node binary only).

Reachable over MCP as `assets.put_skill` / `assets.grant_skill` / `assets.revoke_skill` /
`assets.deprecate_skill` / `assets.load_skill` / `assets.list_skills` via `call_asset_tool`, each
behind its own `mcp:assets.<verb>:call` gate.

> **Cap grammar note:** the skill surface uses `store:skill/**` (recursive tail), not `store:skill/*`.
> A core id contains a `.` (`core.lb-cli`), and the caps grammar splits a resource on `/` AND `.`, so
> `*` (one segment) under-matches a dotted id. See `../../debugging/auth/skill-star-cap-misses-dotted-core-id.md`.

## Tested

S4: capability-deny (no cap, and **cap-but-no-grant**), workspace-isolation, grant→load→revoke,
latest + rollback — `host/assets_skill_test`, `host/assets_isolation_test`,
`assets/tests/skill_version_test`. Core tier: `assets/tests/core_skill_seed_test` (seed idempotence +
versioning + reserved-namespace isolation), `host/tests/core_skills_test` (core.* put/deprecate
rejected even for admin, ungranted-core deny, empty-catalog-without-read-cap, tier rows, deprecate
hide/pin/un-hide, default grants at creation, ws isolation, **real-run catalog injection tracks
grant/revoke**), `host/tests/core_skills_mcp_test` (per-verb MCP deny + tier rows over the bridge).

**Exit-gate clause met:** a skill loads only when granted — for both tiers.
