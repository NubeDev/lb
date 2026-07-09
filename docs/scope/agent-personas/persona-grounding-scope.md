# Agent-personas scope — the grounding corpus (persona-grounding)

Status: **SHIPPED** (corpus 24→34: dynamic skills scan + `docs/testing/**` runbooks + `core.mcp`/
`core.acp`/`core.extension-authoring`; anti-rot build gate; grounding test green). Sub-scope #2 of
`agent-personas-scope.md`. Session:
[`sessions/agent-personas/persona-grounding-session.md`](../../sessions/agent-personas/persona-grounding-session.md).
Promoted to [`public/agent-personas/agent-personas.md`](../../public/agent-personas/agent-personas.md).

> **Audit correction:** there was **no code drift** (build.rs already scans all skills dynamically — the
> "17 vs 24" was stale *docs*, now fixed). The delivered work: the `docs/testing/**` scan root, the
> anti-rot build failure, the 3 new skills, and the grounding proof.

Make the platform's own operating knowledge — how to call MCP verbs, how to drive ACP, how to
test, how to build extensions — available to the agent **as granted, pinnable skills**, so a
persona-grounded agent learns Lazybones **from its docs, not from crawling the codebase**. The
observed symptom this kills: the external agent, given whole-repo access, reads source to guess
how the platform works and gets confused. The corpus already exists (24 `docs/skills/*/SKILL.md`
+ the `docs/testing/` runbooks); the seeding pipeline already exists (`seed_core_skills`, the
build-time embed). This slice closes the gaps between them.

## Goals

- **Seed the full skills corpus.** Every `docs/skills/*/SKILL.md` (24 today) is embedded and
  boot-seeded as `skill:core.<name>@<version>` — audit the embed list vs. the on-disk dirs
  (public docs say 17 seeded; the dirs say 24) and close the drift with a **build-time
  assertion**: a `SKILL.md` on disk that is not in the embedded corpus fails the build, so the
  corpus can never silently rot again.
- **Promote `docs/testing/` to skills.** The e2e runbooks (`e2e-backend.md`, `e2e-frontend.md`,
  and the per-area runbooks — charts, dashboard, datasources, nav, system) become
  `core.e2e-backend`, `core.e2e-frontend`, `core.testing-<area>` seeded skills. They already
  carry skill-shaped frontmatter (the testing README anticipated exactly this). **The docs stay
  where they are** — the embed pulls from `docs/testing/` too; no copy that can drift. These are
  what make "the agent tests what it built" real for personas #3/#4.
- **Author the missing grounding skills.** The corpus audit will show gaps the personas (#3)
  need; known already:
  - `core.mcp` — the MCP contract itself: `tools.catalog`, `<ext>.<tool>` resolution, cap
    grammar `mcp:<verb>:call`, error shapes, `lb call` driving (today spread across
    `scope/mcp/` and many skills' preambles — one skill owns it);
  - `core.acp` — the ACP surfaces: our server (`role/acp`, Zed/Cursor drive us) AND the
    external-agent client seam (`agent.runtimes`, profiles) — the agent explaining/operating
    its own runtime story;
  - `core.extension-authoring` — the devkit path: `devkit.templates/scaffold/build/inspect`,
    `ext.publish`, the WIT boundary, `build.sh`, federation widgets (the #4 persona's manual —
    today only the operator-facing `core.extensions` exists).
- **Default grants stay honest.** Fresh-workspace default grants remain the minimal set
  (`core.lb-cli`, `core.query`, `core.store-read`); personas **pin, grants gate** — adopting a
  persona whose skills aren't granted surfaces the named grant error (#1 fail-closed), and the
  Settings picker offers the admin the one-click "grant this persona's skills" batch (small,
  bounded, synchronous — an explicit `assets.grant_skill` per id, no job needed).
- **Both runtimes, one corpus.** No new injection work — #1's pinning rides the shipped
  `render_catalog` / `load_substrate_skill` / goal-fold seams. This slice is **content +
  seeding**, deliberately.

## Non-goals

- New injection mechanics (shipped; #1 applies them).
- Rewriting existing skills' bodies (only the audit's genuine gaps are authored).
- Vector/semantic retrieval over skill bodies (agent-memory v2 territory).
- Workspace-authored grounding docs — already possible today (`assets.put_skill`, user tier);
  nothing new needed.
- Seeding `docs/scope/` or `doc-site/content/public/` wholesale — scope docs describe *asks*, not shipped
  truth; grounding the agent in unshipped designs would *cause* confusion, not cure it. Skills
  are written from live behavior (the skills rule) — that boundary is the point.

## Intent / approach

**Skills are the grounding unit — not repo access, not a RAG side-channel.** The platform
already decided how an agent learns a surface: a granted, versioned, immutable `SKILL.md` whose
frontmatter is advertised and whose body loads on demand. Grounding personas is therefore a
*corpus completeness* problem, not a new mechanism.

**Rejected: giving the agent the repo (the status quo).** Source is the wrong altitude (the
agent needs the contract, not the implementation), unbounded (context burn = the confusion),
and workspace-blind (the repo doesn't know what THIS workspace granted).

**Rejected: a second "docs" pipeline beside skills** (e.g. injecting `docs/testing/*.md` raw at
run time). Two grounding channels with different gating would split the wall; the testing README
already points at the skills loader as the intended path.

## How it fits the core

- **Tenancy / isolation:** core skills are readable-everywhere seeds; the **grant** is the
  workspace wall (`grant:skill/{id}`), unchanged. A ws-B agent can't load a skill ws-B didn't
  adopt — mandatory test re-asserted over the new seeds.
- **Capabilities:** no new caps; rides `mcp:assets.{list,load,grant,revoke}_skill:call` +
  `store:skill/**:read`. Deny: ungranted new seed invisible in catalog and refused on load.
- **Placement:** either — build-time embed + boot seed is node-symmetric.
- **MCP surface:** none new. Batch note: the picker's grant-all is N bounded synchronous
  `assets.grant_skill` calls (N ≤ ~10 per persona), not a job.
- **Data:** `skill:{id}@{version}` records via the existing idempotent seed; version bump per
  content change (immutable-per-version holds).
- **Bus / secrets / WIT:** none. **Stateless:** yes.
- **No mocks (rule 9):** tests grant + load the real seeded records through the real verbs.
- **File layout:** content lives in `docs/skills/` + `docs/testing/` (authored docs); the only
  code is the embed manifest + the build assertion in `lb-assets`' build script.
- **Skill doc:** this slice's deliverables ARE skills; `skills/skills/SKILL.md` (the
  skills-system skill) gains the corpus-assertion note.

## Example flow

1. Build embeds `docs/skills/**` + `docs/testing/**` → boot seeds ~30 `core.*` skills
   (idempotent; version-bumped ones re-seed).
2. Admin adopts the data-analyst persona (#3); the picker shows two of its pinned skills
   ungranted and offers the grant batch; admin confirms (`assets.grant_skill` × 2).
3. A run under the persona starts grounded: identity + `core.datasources`, `core.query`,
   `core.e2e-backend` bodies pinned; the agent answers "how do I verify this datasource?" from
   the runbook, not from reading `rust/crates/host/src/federation/`.
4. A ws-B member with the same persona but no grants gets the fail-closed named error — the
   wall, not a fallback.

## Testing plan

- **Capability-deny (§2.1):** ungranted new seed (`core.e2e-backend`) absent from
  `assets.list_skills` and denied on `load_skill`; the persona pinning it fails the run at
  start (#1's test, re-run over a real new seed).
- **Workspace-isolation (§2.2):** grants are per-workspace — ws-A adopting `core.mcp` leaves it
  invisible in ws-B.
- **Offline/sync:** idempotent re-seed on boot/upgrade (re-run seed, assert no dupes, version
  bump respected) — the shipped core-skills test extended to the testing-sourced seeds.
- **Build assertion:** add an unembedded `SKILL.md` fixture → build fails (the anti-rot gate,
  proven).
- **Content smoke (per new skill):** each authored skill's examples are exercised against a
  live node in the session (the skills rule — grounded in a live run, output in the session
  doc).

## Risks & hard problems

- **Doc drift is now agent-facing.** A stale runbook used to mislead a human; now it misleads
  every persona-grounded run. The build assertion catches *missing*, not *wrong* — the
  discipline stays "skills are written from live behavior", and each version bump re-proves
  examples.
- **Context weight.** Pinning bodies (not just frontmatter) costs tokens per run; a persona
  pinning 6 fat runbooks may spend more grounding than working. #3 keeps pins ≤ ~4 per persona
  (bodies) + filtered catalog for the rest; `agent-close-out` A's real token counts are the
  instrument to tune with.
- **17-vs-24 archaeology.** The drift audit may surface skills that were *deliberately* not
  seeded; resolve each explicitly in the session doc, don't blanket-seed.

## Open questions

1. **Version source for testing-sourced skills:** frontmatter `version:` per file (proposal) vs
   a corpus-wide build stamp? Per-file keeps bumps intentional.
2. **`core.testing-<area>` granularity:** one skill per area README (proposal — matches persona
   pins) vs one fat `core.e2e` skill?
3. **Should the seed include `docs/FILE-LAYOUT.md`-style repo rules for the #4 coding persona?**
   Proposal: yes, folded INTO `core.extension-authoring` (the persona needs the rules where it
   works), not as a standalone repo-rules skill.

## Related

- `agent-personas-scope.md` (umbrella), `persona-model-scope.md` (#1 — the pinning this feeds),
  `persona-catalog-scope.md` (#3 — the pin lists), `persona-coding-scope.md` (#4 —
  `core.extension-authoring`'s consumer).
- `scope/skills/core-skills-scope.md` + `public/skills/skills.md` (seed pipeline, grant gate),
  `docs/testing/README.md` ("Should this be a skill?" — the anticipation this fulfills),
  `scope/mcp/mcp-scope.md` (the `core.mcp` source), `scope/agent-run/agent-run-scope.md` Part 4
  + `scope/external-agent/` (the `core.acp` sources), `scope/extensions/extensions-scope.md`
  (the `core.extension-authoring` source).
- README `§6.5`, `§6.16`.
