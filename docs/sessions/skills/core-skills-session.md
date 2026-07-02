# Skills — core (developer) skill tier + agent-facing catalog (session)

- Date: 2026-07-03
- Scope: ../../scope/skills/core-skills-scope.md
- Stage: post-S8 (building on the shipped S4 skill substrate) — branch `ce-node-wiring-v2`
- Status: done

## Goal

Add the **second skill tier** — developer-authored **core skills**, shipped with the node and
seeded at boot — alongside the existing workspace-authored user tier, both loading through the SAME
grant gate and the SAME `load_skill`. Ship the full contract the scope named: embed the
`docs/skills/*/SKILL.md` corpus at build time, seed immutable `skill:core.<name>@<node-version>`
records via a reserved system scope, reject `put_skill`/`deprecate` on any `core.*` id, add
`assets.deprecate_skill` (soft delete), enrich `list_skills` with `{tier, description}`, apply a
default grant set at workspace creation, and inject the granted catalog into BOTH runtimes at
session start (persona unified onto the same loader).

## What changed

**`lb-assets` (store side):**
- `crates/assets/build.rs` — embeds `docs/skills/*/SKILL.md` at build time: parses each frontmatter
  (`name`/`description`), strips it, flags repo-relative links (`](../…)`→`](repo-relative: …)`),
  emits `$OUT_DIR/core_skills_corpus.rs` as `CORE_SKILLS: &[(name, description, body)]`. 17 skills
  embedded, verified.
- `crates/assets/src/skill/corpus.rs` — `include!`s the generated corpus + a typed iterator.
- `crates/assets/src/skill/core.rs` — the ONE skills-only resolver: reserved namespace
  `CORE_SKILLS_NS = "_lb_skills"` (the `_lb_identity`/`_lb_workspaces` precedent), `is_core()`, the
  seeder-only `seed_core_skill` (idempotent — immutable version = no-op), and the read verbs
  `get_core_skill`/`list_core_skill_versions`.
- `crates/assets/src/skill/seed.rs` — `seed_core_skills(store, version, ts)` writes the whole corpus
  as `skill:core.<name>@<version>`; the boot seeder is the only writer.
- `crates/assets/src/skill/meta.rs` — `skill_meta:{id}` soft-delete flag (per id, workspace-scoped)
  backing deprecate; `set_deprecated`/`is_deprecated`.
- `crates/assets/src/skill/put.rs` — a new version **un-hides** a deprecated id (clears the flag).

**`lb-host` (gated side):**
- `assets/load_skill.rs` — resolves `core.*` against the reserved namespace after the SAME
  workspace grant check (no bypass); honors deprecation on latest resolution (pinned still loads).
- `assets/put_skill.rs` + `assets/deprecate_skill.rs` — reject any `core.*` id (`AssetError::Reserved`,
  a NON-opaque `BadInput`) BEFORE the caps gate; deprecate then needs `store:skill/{id}:write`.
- `assets/list_granted_skills.rs` — catalog entries gain `tier` (Core|User) + `latest`; core rows
  resolve from the reserved namespace, deprecated user ids are skipped.
- `assets/tool.rs` — wired `list_skills` (the agent catalog: `{id, latest, description, tier,
  granted}`), `deprecate_skill`, `revoke_skill` into the MCP dispatch.
- `agent/dispatch.rs` — the EXTERNAL runtime now injects the granted catalog into the goal (its only
  channel; it can't call the loop-internal `skill.activate`); the in-house loop keeps its own
  once-per-run injection (`run.rs`). Persona rides the same grant-gated `load_skill` loader.
- `workspaces/default_skills.rs` + `workspaces/create.rs` — default core-skill grant set
  (`core.lb-cli`/`core.query`/`core.store-read`) applied on genuine workspace creation; env override
  `LB_DEFAULT_CORE_SKILLS`.
- `node/src/main.rs` — boot seeds the corpus at `env!("CARGO_PKG_VERSION")` and grants the resolved
  default set to the boot workspace.
- `role/gateway/src/session/credentials.rs` — dev-login gains the per-verb `mcp:assets.*_skill:call`
  caps AND switches the skill surface to `store:skill/**` (see Debugging).

## Decisions & alternatives

- **Reserved namespace, not a magic ws / second SurrealDB namespace** (scope "Decided"): `_lb_skills`
  mirrors the shipped `_lb_identity` pattern — one constant + one resolver file. Core records live
  once per node; grants stay workspace-scoped. Rejected copying every core skill into every
  workspace (store bloat + upgrade fan-out).
- **Two tiers, one table + one gate**: core skills are ordinary `skill:{id}@{version}` records; the
  ONLY differences are WHERE they live (reserved ns) and WHO writes them (the boot seeder). Rejected
  a separate "system skills" table (forks every verb/test) and a grant bypass (creates an ungated
  tier that breaks the "loads only when granted" invariant).
- **Deprecate = soft delete**, hard delete rejected (breaks rollback/audit/in-flight runs). A new
  version un-hides (deprecate is a state, not a tombstone) — asserted explicitly.
- **`Reserved` is NON-opaque** (`BadInput`, not `Denied`): the `core.` namespace is a public,
  deliberate reservation, so the caller is told plainly — unlike a caps deny.
- **External-runtime catalog injection into the goal**: the ACP runtime drives `ctx.goal` verbatim
  over `exec --json` and has no separate system-prompt/`skill.activate` channel, so the catalog folds
  into the goal in `invoke_via_runtime` (the one seam both paths flow through) under the derived
  principal. Bodies stay on-demand via the profile's granted `load_skill`.

## Tests

Mandatory categories (testing §2), rule 9 (real `mem://` store, real loop, real gateway; only the
model provider is stubbed):

- `crates/assets/tests/core_skill_seed_test.rs` (3): `is_core`; seed idempotence + versioning
  (re-seed = no-op, upgrade coexists, rollback resolves); core records live outside any ws namespace.
- `crates/host/tests/core_skills_test.rs` (11): **put_skill("core.*") rejected even for an admin**;
  deprecate("core.*") rejected; deprecate without write cap → **Denied**; **ungranted core skill
  denied like a user skill (no bypass)**; **empty catalog + denied loads without the read cap**;
  catalog carries core+user tiers; deprecate hides from list/latest but pinned still loads +
  republish un-hides; **default grants applied at workspace creation**; the default set + env
  override; **workspace isolation (ws-B's core grant invisible in ws-A)**; **real in-house run
  injects exactly the granted catalog and tracks grant→grow, revoke→shrink** (capturing Provider).
- `crates/host/tests/core_skills_mcp_test.rs` (4): `list_skills` tier rows over the bridge (no body
  leak); deprecate+revoke over the bridge; put_core → BadInput (non-opaque); **per-verb MCP deny**
  (each of the 6 verbs refused at the MCP gate without its cap).

Green output (the three new suites):

```
core_skill_seed_test:  ok. 3 passed; 0 failed
core_skills_test:      ok. 11 passed; 0 failed
core_skills_mcp_test:  ok. 4 passed; 0 failed
```

Full `cargo test --workspace` + `cargo fmt` green — see the STATUS/paste below.

## Debugging

- [auth/skill-star-cap-misses-dotted-core-id.md](../../debugging/auth/skill-star-cap-misses-dotted-core-id.md)
  — a `store:skill/*` grant can't reach a core skill because the `.` in `core.lb-cli` is a caps-grammar
  segment boundary; fixed by granting the skill surface with the recursive-tail `**`. Regression: the
  `core_skills_*` suites (all use `store:skill/**`). This was a real fix to the shipped dev-login.

## Public / scope updates

- `docs/public/skills/skills.md` updated with the core tier + deprecate + catalog+tier.
- Scope open questions were already "Decided" — all realized as built (system namespace, default
  grant set, republish-un-hides, persona-as-pinned-catalog-entry).

## Skill docs

- `docs/skills/skills/SKILL.md` written — managing the catalog (author/grant/deprecate, what the
  agent sees, the core tier), grounded in `lb call` examples over the verbs.

## Dead ends / surprises

- The caps grammar splitting on `.` (not just `/`) meant every dotted core id under-matched a
  `skill/*` grant — invisible until core ids (which inherently contain `.`) existed. Logged + fixed.
- **Pre-existing red test, unrelated to this slice, fixed in passing:** `flows_nodes_test.rs`'s
  `BUILTINS` list was missing `flipflop` (added by commit `a4128e7` "flip/flop node" without updating
  the test) — it was failing on the branch before my changes. Added `"flipflop"` after `"trigger"` to
  match the registry order, so the workspace is green. Not caused by core-skills; noted for honesty.

## Follow-ups

- A CI lint that flags un-rewritten repo-relative links in `docs/skills/*` (the build rewrites them
  at embed time; a lint keeps authors honest) — scope risk "corpus drift".
- STATUS.md updated: yes (core-skills slice shipped).
