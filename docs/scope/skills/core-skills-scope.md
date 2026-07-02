# Skills scope — core (developer) skills + the user skill tier

Status: scope (the ask). Promotes to `public/skills/` once shipped.

The workspace skill asset shipped in S4 (`skills-scope.md`) gives users a grant-gated, versioned
skill they author themselves. This scope adds the **second tier**: **core skills — authored by the
platform developers, shipped with the node, seeded at boot** — alongside the existing **user tier**
(workspace-authored, full CRUD). Both tiers load through the *same* grant gate and the *same*
`load_skill` verb, so an agent (in-house loop or external ACP agent) sees **one catalog** whose
content is exactly what the workspace granted ∩ what the caller may reach. This is half of "make
the agent smarter" (the other half is `../agent-memory/agent-memory-scope.md`): the smarts are
**granted data, never widened authority**.

> Read with: `skills-scope.md` (the shipped substrate), `../agent-run/` (model-activated skills —
> the agent picks from a granted catalog), `../external-agent/acp-driver-scope.md` (persona skill +
> `granted_tools`), `../external-agent/capability-wall-scope.md` (why the agent can't reach past the
> catalog), README §6.12 (load only when granted), §6.4 (versioned assets).

---

## Goals

- **A core-skill corpus, authored in-repo.** The developer-maintained operating manuals in
  `docs/skills/*/SKILL.md` (auth-caps, flows-mcp, lb-cli, external-agent, …) *are* the corpus: each
  is already a runnable how-to for one platform surface, written per the ABOUT-DOCS skill rules.
  This scope makes them **loadable skills on every node**, not just repo docs.
- **Seeded at boot, versioned by release.** The node embeds the corpus at build time and seeds
  `skill:core.<name>@<node-version>` records at boot (idempotent — immutable versions never
  conflict). A node upgrade seeds the new versions; old versions remain for rollback (§6.4).
- **The `core.` namespace is read-only to users.** `put_skill` (and the new deprecate verb) reject
  any `core.*` id — core skills change only by shipping a new node build. Users keep **full CRUD on
  their own tier**: create (`put_skill`, shipped), update (a new version, shipped), read
  (`load_skill`/`list_skills`, shipped), and **delete via a new `assets.deprecate_skill`** (below).
- **Grant-gated, both tiers.** A core skill is *present* on every node but **loads only when the
  workspace granted it** — §6.12 holds with no core-tier exception. Workspace creation applies a
  configurable **default grant set** (e.g. `lb-cli`, `query`, `store-read`) so a fresh workspace's
  agent is useful out of the box; an admin can revoke any of them like any other grant.
- **One agent-facing catalog.** `list_skills` returns `{id, latest, description, tier, granted}`;
  the agent's session context carries the compact **granted** catalog (name + one-line description,
  the Claude-skills shape), and the agent pulls bodies on demand via `load_skill` — the
  model-activated-skills path `agent-run/` already scopes. What the agent can *see and load* is
  computed under the **derived principal** (`caller ∩ agent`), so a user's agent run can never
  browse skills the user couldn't.

## Non-goals

- **A public skill registry / marketplace** — S7 signed distribution (§6.4). Core skills ride the
  node binary; third-party skill packs are later.
- **Skill *execution* semantics** — a skill body stays instructions/recipe text; structured
  tool-recipes + schema validation remain deferred to the agent scopes.
- **Team-scoped grants** — still the open question in `skills-scope.md`; unchanged here.
- **Rewriting the `docs/skills/` authoring flow.** Authors keep writing `SKILL.md` files per
  ABOUT-DOCS; the build embeds them. No second authoring surface.

## Intent / approach

**Two tiers, one substrate, one gate.** Core skills are ordinary `skill:{id}@{version}` records —
same table, same verbs, same grant relation — distinguished only by the reserved `core.` id prefix
and by *who writes them* (the boot seeder, from the embedded corpus, instead of `put_skill`). That
keeps every shipped property (immutability, versioning, grant gate, isolation tests) working on the
new tier for free.

- **Seeding**: a build script embeds `docs/skills/*/SKILL.md` (frontmatter `name`/`description` +
  body) into the node binary; `node` boot runs `seed_core_skills` — for each embedded skill, write
  `skill:core.<name>@<node-version>` if absent (idempotent; immutable versions make re-seeding a
  no-op). Per-workspace default grants are applied at **workspace creation** from node config
  (`default_granted_core_skills`), not at boot for every existing workspace — an existing workspace
  changes only when its admin grants.
- **User delete = deprecate.** Versions are immutable and rollback-bearing, so hard delete is
  wrong. `assets.deprecate_skill(id)` marks the skill id hidden: it disappears from `list_skills`
  and `latest` resolution; explicitly-pinned loads of old versions still work (audit + rollback
  preserved). Requires `store:skill/{id}:write` (the author-tier cap). **Rejected: hard delete** —
  it breaks rollback, breaks audit, and lets a user yank a version an agent run is mid-way through.
  A storage-purge admin verb can follow if bulk matters; not now.
- **Catalog injection**: at session start the runtime (in-house loop and `AcpRuntime` alike) calls
  `list_skills` under the derived principal and renders the granted entries as a compact catalog in
  the agent's instructions, alongside the persona skill. The agent then calls
  `assets.load_skill` mid-run (already in `granted_tools` per the acp-driver scope).

**Rejected: a separate "system skills" table or a grant bypass for core skills.** A second table
forks every verb and test; a grant bypass creates an ungated tier and silently breaks the S4
exit-gate clause ("a skill loads only when granted") the moment a core skill says something a
workspace doesn't want its agents told. Default-grant-at-creation gives the same UX without the
hole — and the admin can see and revoke it, because it is an ordinary grant record.

## How it fits the core

- **Tenancy / isolation:** skill *records* for the core tier are seeded per node into each
  workspace namespace? **No — decided:** core skill records live once in a node-level **system
  namespace**, and `load_skill`/`list_skills` resolve `core.*` ids against it **after** the
  workspace grant check (the grant relation stays workspace-scoped). Isolation holds: ws-B granting
  `core.lb-cli` never affects ws-A, and user-tier skills remain fully workspace-namespaced. (The
  alternative — copying every core skill into every workspace — bloats the store and makes upgrade
  a fan-out write; rejected.)
- **Capabilities:** unchanged gates — `store:skill/*:read` for the surface, `grant:skill/{id}` to
  load, `store:skill/{id}:write` for user-tier put/deprecate. New deny paths: `put_skill("core.*")`
  → rejected regardless of caps; `deprecate_skill` without the write cap → denied.
- **Placement:** `either` — every node embeds the same corpus (symmetric); grants sync like any
  workspace record (hub-authoritative, immutable-version append is the easy case).
- **MCP surface:** existing `assets.put_skill` / `load_skill` / `grant_skill` / `revoke_skill` /
  `list_skills`, plus **one new verb** `assets.deprecate_skill` (CRUD's delete, soft). `list_skills`
  gains `tier` + `description` in its rows (additive). No live feed (skills are state, loaded on
  demand); no batch (grants are singular admin acts).
- **Data (SurrealDB):** `skill:{id}@{version}` unchanged; a `deprecated` flag record (or field on a
  small `skill_meta:{id}`) for the soft delete; system-namespace rows for `core.*`. State only.
- **Bus (Zenoh):** none.
- **Sync / authority:** core records are node-local-derived (re-seeded from the binary — never
  synced); grants and user skills sync as today.
- **Secrets:** none. Skill bodies are instructions; a skill that needs a credential names an env
  var/secret ref, mirroring `agent.config`'s names-only rule.
- **SDK/WIT impact:** none.
- **Skill doc:** yes — `docs/skills/skills/SKILL.md` (managing the catalog: author, grant, deprecate,
  what the agent sees), written by the implementing session from a live run.

## Example flow

1. A node built at `v0.9.0` boots; `seed_core_skills` writes `skill:core.lb-cli@0.9.0`,
   `skill:core.query@0.9.0`, … (no-ops on next boot).
2. A new workspace `acme` is created; the default grant set writes `grant:skill/core.lb-cli` and
   `grant:skill/core.query`.
3. Ada authors her own: `put_skill("acme-runbook", "1.0.0", …)` and an admin grants it.
4. Ada asks the agent (in-house or `open-interpreter-default`) a question. At session start the
   runtime lists granted skills under `ada ∩ agent` → catalog: `core.lb-cli`, `core.query`,
   `acme-runbook` (+ descriptions). Mid-run the agent decides it needs CLI syntax and calls
   `assets.load_skill("core.lb-cli")` → grant ✔ → body loads.
5. Ada tries `put_skill("core.lb-cli", "9.9.9", …)` → **rejected** (reserved namespace), regardless
   of her caps.
6. Ada deprecates `acme-runbook` → it vanishes from `list_skills` and from the next run's catalog;
   a pinned `load_skill("acme-runbook", "1.0.0")` still resolves for rollback/audit.
7. A user whose caps don't include `store:skill/*:read` invokes the agent: the derived principal
   lacks the surface → the catalog is **empty** and every `load_skill` denies. The agent is exactly
   as smart as the user is allowed.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`):

- **Capability-deny (§2.1):** (a) `put_skill` on a `core.*` id → rejected even for an admin;
  (b) `deprecate_skill` without the write cap → denied; (c) an **ungranted core skill** fails
  `load_skill` exactly like an ungranted user skill (no core bypass — the headline deny);
  (d) derived-principal catalog: a caller without `store:skill/*:read` gets an empty catalog and
  denied loads inside an agent run.
- **Workspace-isolation (§2.2):** ws-B's grant of `core.x` invisible in ws-A; ws-B can never list/
  load ws-A user skills (regression on the shipped test, extended to `tier` rows).
- **Seeding idempotence:** boot twice → one record set; upgrade (new version constant) → new
  versions seeded, old intact; default grants applied on workspace-create only.
- **Deprecate:** hidden from list/latest; pinned load still works; re-`put` of a deprecated id as a
  new version un-hides it (or is rejected — decide in-session, test either way explicitly).
- **Catalog injection (real agent, rule 9):** a real in-house run's context contains exactly the
  granted catalog; grant one more skill → next run's catalog grows; revoke → shrinks.

## Risks & hard problems

- **The system namespace is new surface.** Everything else is workspace-namespaced; a node-level
  read path must be provably read-only from workspace principals and must not become a dumping
  ground. Keep it skills-only; one resolver file.
- **Corpus drift.** `docs/skills/` is written for repo readers (relative links, repo paths). Loaded
  into an agent on a customer node, those links dangle. The build step should strip/rewrite the
  frontmatter and flag repo-relative links; a lint in CI keeps authors honest.
- **Catalog bloat.** 17+ core skills × descriptions is fine; 100 is context tax. The catalog is
  name+description only (bodies on demand), and the default grant set is deliberately small —
  granting everything to every workspace is an anti-goal.
- **Prompt-injection via user skills.** A granted skill body goes into the agent's context; the
  wall (caps + sandbox) — not skill text — is what constrains the agent. Already the acp-driver
  stance ("the persona steers, the wall constrains"); restate it in the loader docs.

## Decided (was: open questions)

- **System namespace:** a reserved **system scope in `lb-store`** (one constant + one resolver
  file, skills-only), not a magic workspace id and not a second SurrealDB namespace. Workspace
  principals get read-only resolution of `core.*` through it; only the boot seeder writes.
- **Default grant set:** the read-only core skills — `core.lb-cli`, `core.query`,
  `core.store-read`. Anything that drives writes stays opt-in per workspace admin.
- **Re-publishing a deprecated id:** a new version **un-hides** it (deprecate is a state, not a
  tombstone); the test asserts this explicitly.
- **Persona skill:** yes — `persona_skill` is just a **pinned catalog entry** (same `load_skill`,
  same grant gate); unify when wiring the runtimes so there is one loader path, not two.

## Related

- `skills-scope.md` (the shipped substrate this extends), `../../public/skills/skills.md`.
- `../agent-memory/agent-memory-scope.md` (the sibling "make the agent smarter" scope).
- `../agent/agent-scope.md`, `../agent-run/` (model-activated skills),
  `../external-agent/acp-driver-scope.md` (persona + `granted_tools`),
  `../external-agent/capability-wall-scope.md` (the enforcement wall).
- README `§6.12`, `§6.4`, `§6.16`; `../../ABOUT-DOCS.md` (the `skills/` authoring rules the corpus
  already follows).
