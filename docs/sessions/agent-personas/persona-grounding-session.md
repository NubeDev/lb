# Persona-grounding (agent-personas #2) — session log

Status: **SHIPPED**. Scope: [`scope/agent-personas/persona-grounding-scope.md`](../../scope/agent-personas/persona-grounding-scope.md)
(umbrella: [`agent-personas-scope.md`](../../scope/agent-personas/agent-personas-scope.md)).
Depends on #1 (the pinning seam, shipped).

## The ask, restated

Make the platform's own operating knowledge available to the agent **as granted, pinnable skills**, so a
persona-grounded run learns Lazybones from its **docs, not from crawling the codebase** (the observed
confusion cure). Seed the full skills corpus + the `docs/testing/` runbooks, add a build assertion so the
corpus can't silently rot, and author the missing grounding skills (`core.mcp`, `core.acp`,
`core.extension-authoring`). This slice is **content + seeding** — #1 already shipped the injection.

## The audit finding (corrects the scope's premise)

The scope assumed a 17-vs-24 embed drift to reconcile. **There is no code drift** — `crates/assets/build.rs`
already **dynamically scans** every `docs/skills/*/SKILL.md` with **no allow-list**, so all 24 on-disk
skills were already embedded. The "17" is stale *documentation* (STATUS.md, the skills session doc, a
sample boot log) — fixed, and replaced with "the corpus is the whole tree, don't hardcode a count".

So the real #2 work became: (a) add `docs/testing/**` as a second scan root, (b) make the anti-rot gate a
hard build failure, (c) author the 3 new skills (auto-embed), (d) fix the stale docs, (e) prove it.

## What shipped

**`crates/assets/build.rs` — two changes:**
1. **Second scan root `docs/testing/**`.** A recursive `collect_markdown` walk pulls every
   frontmatter-bearing `.md` runbook (the top-level `e2e-backend.md`/`e2e-frontend.md` + the per-area
   `<area>/README.md`), each seeding as `core.<name>` (their frontmatter names are already `e2e-*`, so
   `core.e2e-backend`, `core.e2e-nav`, …). The frontmatter-less `docs/testing/README.md` index is
   skipped. Pulled from where they live — **no copy that can drift** (the scope's requirement).
2. **The anti-rot assertion (a hard build failure).** A `docs/skills/<dir>/` missing its `SKILL.md` now
   **panics the build** (was: silently skipped). A half-authored skill can never ship as "absent" — which
   would fail-close a persona pinning it at run time with no build-time signal. Plus a duplicate-name
   guard across the two scan roots.

**Corpus: 24 → 34** — the 24 `docs/skills/*` + 7 `docs/testing/**` runbooks + the 3 new skills.

**The 3 authored skills (grounded in live source, each reviewed against real code before accepting):**
- `docs/skills/mcp/SKILL.md` → **`core.mcp`** — the MCP contract: `call(principal, "<ext>.<tool>", json)
  -> Result<json, ToolError>`, the `mcp:<ext>.<tool>:call` cap grammar (workspace-first, re-checked
  every call), the four `ToolError` variants, the **honest deny** (a denial leaks no tool existence —
  never retry it), `tools.catalog`. Cited from `crates/mcp/src/call/{authorize,error,resolve}.rs` +
  `tool_call.rs::is_host_native` + the proving `tools_catalog_test`.
- `docs/skills/acp/SKILL.md` → **`core.acp`** — the two ACP surfaces: **we-as-ACP-server** (`role/acp`,
  honest that it's **partial ACP v1** — the 5 implemented JSON-RPC methods from `session.rs`, and that
  `session/update`/`request_permission` are *emitted, not handled*) and **we-driving-an-external-agent**
  (the runtime seam, `agent.runtimes`/`agent.config`, feature-gated). Run lifecycle caps grepped from
  `credentials.rs`.
- `docs/skills/extension-authoring/SKILL.md` → **`core.extension-authoring`** — the devkit *developer*
  manual (distinct from the operator-view `core.extensions`): the `templates → scaffold → code → build →
  inspect → publish → prove-it` loop, real `devkit.*` shapes from `crates/devkit/src/model.rs`,
  FILE-LAYOUT folded in, `ext.publish`/`native.install` as Ask-gated proposals, "built ≠ done" paired
  with `core.e2e-backend`. The #4 extension-builder persona's grounding.

**Default grants stay honest:** the fresh-workspace default set is unchanged (`core.lb-cli`,
`core.query`, `core.store-read`); personas *pin*, grants *gate* — an ungranted pin fail-closes (#1).

## Tests (green)

- `crates/assets/tests/core_skill_seed_test.rs`: `the_testing_runbooks_seed_as_core_skills`
  (`core.e2e-backend`/`core.e2e-frontend` seed; the README index does NOT) + `every_skills_dir_carries_a_skill_md`
  (the anti-rot invariant as a runtime mirror of the build gate) + the existing idempotent/versioned/
  ws-isolation tests — **5 green**.
- `crates/host/tests/core_skills_test.rs` — the grant-gate + ws-isolation suite stays green over the
  bigger corpus (**11 green**).
- **The GROUNDING TEST (umbrella exit gate)** in `agent_persona_test.rs`:
  `a_persona_grounded_run_is_fed_its_pinned_skill_body_not_the_repo` — seeds the REAL corpus, grants
  `core.e2e-backend`, a persona pins it, drives the in-house loop, and asserts the runbook body (`make
  dev`, a distinctive phrase from `docs/testing/e2e-backend.md`) reached the model's context **and** the
  menu is the persona's one data verb (no fs/shell/repo tool) — the confusion cure proven. Plus
  `a_persona_pinning_an_ungranted_real_seed_fails_closed` (the grant is the wall, over a real seed).

```
$ cargo test -p lb-assets --test core_skill_seed_test        # 5 passed
$ cargo test -p lb-host   --test core_skills_test            # 11 passed
$ cargo test -p lb-host   --test agent_persona_test          # 21 passed (incl. the 2 grounding tests)
```

## Content smoke — grounded in a live corpus

Each new skill's frontmatter parses (a bad key **panics** the build — so a green `cargo build -p
lb-assets` IS the frontmatter check) and embeds in the generated `core_skills_corpus.rs` (verified:
`core.mcp`, `core.acp`, `core.extension-authoring` all present; total 34). Their bodies were reviewed
against the cited live source before acceptance (the skills rule — written from live behavior, honest
about partial impls like ACP v1).

## Open-question resolutions (scope §Open questions)

1. **Version source for testing-sourced skills** → the corpus-wide **node build version** (the shipped
   `seed_core_skills(store, CARGO_PKG_VERSION, ts)` mechanism), NOT per-file frontmatter — there is no
   `version:` key in any SKILL.md, and the runbooks join the same versioned corpus. Consistent + already
   idempotent/rollback-safe.
2. **`core.testing-<area>` granularity** → **one skill per area README** (the existing on-disk layout:
   `e2e-backend`, `e2e-frontend`, `e2e-{nav,system,dashboard,charts,datasources}`) — matches the persona
   pins, and they already carry per-file `e2e-*` frontmatter. Not one fat `core.e2e`.
3. **Repo-layout rules for the #4 coding persona** → folded INTO `core.extension-authoring` (FILE-LAYOUT
   rules where the persona works), not a standalone repo-rules skill. Done.

## Front line / next

#3 (persona-catalog: the 7 built-ins as `personas.toml` data, pinning these new skills) and #4
(extension-builder persona, grounded in `core.extension-authoring`).
