# Testing — proving a feature works in the real world

Status: scope (the standard). This folder is the **operational home for real-world
end-to-end verification**: the runbooks an AI (or a person) *executes* to drive the actual
running system and observe a feature do what it was designed to do — CRUD, permissions,
access, and functional behavior — against a real backend/frontend/DB.

> **This is not the automated test suite, and not an issue tracker.**
> - **Not the test suite.** The code-level tests (`cargo test`, `pnpm test`) are written
>   and run by the **scope/session** that builds the feature — this folder **assumes they
>   are already green** and does **not** re-run them. Its job is the layer *on top*: stand
>   up the real system and watch it behave. A green unit test proves the code; this proves
>   the *running product*.
> - **Not a bug log.** These runbooks are proactive — "does it work as designed?" A bug
>   you find gets *filed* as a `debugging/` entry (see "The loop"), not written up here.

## Start from the design, not the code

Before you test, know what "correct" *is*. Every run starts by reading the intent:

1. **`scope/<topic>/<name>-scope.md`** — *the ask*. What the feature is supposed to do,
   its constraints, its access rules. This is the spec you test **against**. If you can't
   name the expected behavior, you can't assert it.
2. **`skills/<name>/SKILL.md`** — *the drivable surface*. The exact verbs / routes /
   payloads to exercise the feature, grounded in a live node. This is *how* you drive it.
3. **`public/<topic>/`** — *what shipped*, if it's promoted yet.

Then run the runbook: exercise the surface from (2), assert it matches the design from
(1). The runbook tells you the *mechanics* (real node, seeding, the checklist); scope +
skills tell you *what to expect*.

> This folder answers "**how do I prove it works as designed**". It does **not** replace:
> - `../scope/testing/testing-scope.md` — the **policy**: the no-mocks rule (§0), the
>   test pyramid, the mandatory categories (cap-deny, workspace-isolation). Read that
>   first; it's the constitution. The runbooks here obey it.
> - `../debugging/` — the **post-mortem log**. E2e testing is proactive; a failing e2e
>   run *feeds* debugging but is not stored there. See "The loop" below.

---

## Why this is a separate folder (and not `debugging/`)

A common wrong turn: putting "how AI should e2e-test" into `debugging/`. They are
different jobs and must not be conflated.

| Artifact | Tense | Job |
|---|---|---|
| `scope/testing/testing-scope.md` | policy | What *must* be true of any test (no mocks, cap-deny, isolation). |
| **`testing/` (this folder)** | **imperative** | **The runbook you execute to drive a real e2e flow.** |
| `debugging/<area>/<symptom>.md` | past | The forensic card written *after* something broke. |

E2e testing is a **runbook**, not a bug log. It's repeatable, proactive, and grounded in
a live node — the same shape as a `skills/` guide. So each runbook here is written to be
**agent-runnable**: exact commands, real payloads, real assertions, teardown. An AI asked
to "e2e test the backend" reads `e2e-backend.md` and *does it*.

---

## What to check — the functional dimensions

For any feature, prove these four against the design (scope) by driving the real surface
(skill). The runbooks give the mechanics per stack; this is the shared checklist.

1. **CRUD — the data lifecycle works.** For each entity the feature owns: **create** it
   through the real verb, **read** it back and assert the shape matches scope, **update**
   a field and re-read, then prove **delete** on a *throwaway* record you made for that
   check. Round-trip through the real store — never assert on internal state. A create you
   never read back proves nothing. **Leave the primary artifact in place** for the user to
   inspect (see "Leave it inspectable"); only the delete-check record gets removed.
2. **Permissions — the capability wall holds.** The *negative* path is the point
   (capability-first, README §5). For every verb: **with** the grant it succeeds, **without**
   it is refused. A feature with only happy-path tests is not tested. (testing-scope §2.1)
3. **Access — the workspace wall holds.** Workspace is the hard tenant boundary, checked
   *before* caps. Seed into workspace A; prove workspace B can neither read nor write A's
   data — across store, bus, and MCP. (testing-scope §2.2)
4. **Functional — it behaves as designed.** The feature's *actual job*: the state
   transitions, the workflow, the computed result, the edge cases the scope calls out.
   This is where you test the behavior the scope doc promises — inbox→triage→approval, a
   flow firing, a tag applied. Drive the real end-to-end path and assert the outcome the
   design specifies.

CRUD + permissions + access are the **mandatory** trio for any data-touching feature
(they map to testing-scope §2). Functional is the feature-specific proof on top. Do all
four before calling a feature "works as designed".

---

## The runbooks

| Runbook | Drives the running… | How you stand it up |
|---|---|---|
| [`e2e-backend.md`](e2e-backend.md) | live `node` over its MCP/REST surface — real SurrealDB, real Zenoh, real capability wall. | `make build-wasm && make dev`, then drive the verbs. |
| [`e2e-frontend.md`](e2e-frontend.md) | live UI against a real gateway, seeded with real records — no `*.fake.ts`. | `make build-wasm && make dev`, then click through the app. |
| [`system/`](system/README.md) | live `/system/*` — the read-only admin topology/status console (admin-gated, ws-scoped). | `make dev` → mint a token → drive the four reads + the deny paths. |
| [`dashboard/`](dashboard/README.md) | live `/dashboards` + `/panels` — dashboard CRUD, library-panel reuse (ref cells), and the Grafana-style **variable** system (definition round-trip, the required-var access gate, URL/interpolation). | `make dev` → `make seed-demo-sqlite` → round-trip a dashboard + panel + a query-variable, run `dashboard.access_check`. |
| [`charts/`](charts/README.md) | live `/panels` + the node's **own** series feed a chart renders (in-process, no external DB). | `make dev` → save a panel, ingest a sample, read the series back. |
| [`nav/`](nav/README.md) | live `/navs` + the workspace-default pointer + per-user pref + the `nav.resolve` lens. | `make dev` → CRUD a nav, set default, resolve, confirm cap-strip. |
| [`datasources/`](datasources/README.md) | live datasources + the **charts** that read them — real node **and a real seeded SQLite building dataset** (no Docker). | `make dev` → `make seed-demo-sqlite` (seeds + registers `demo-buildings`), then look at the chart. |
| [`rules/`](rules/README.md) | live `rules.*` — the sandboxed Rhai engine: run/save/get/list, real data reads, the cage + `caller ∩ grant` wall. | `make dev` → `make seed-demo-sqlite` → drive `rules.run`/`rules.save`. |
| [`insights/`](insights/README.md) | live `insight.*` + `/insights` — the durable finding record (raise→dedup→ack→resolve→re-open) **and** the rule producer door. | `make dev` (+ `make seed-demo-sqlite` for the rule path) → drive `insight.*` + a raising rule. |

All are bound by testing-scope §0: **no mocks, no fake backends.** Need data? Seed real
records through the real write path (testing-scope §3.1). The datasources runbook has a
**hard prerequisite**: its external source must be **seeded and registered**
(`make seed-demo-sqlite` — the Docker-free SQLite demo dataset, `datasource.add` on a
running node) or every chart reads empty — that's its Step 0.

---

## The loop (how e2e, testing, and debugging connect)

```
scope/<topic>/…-scope.md  +  skills/<name>/SKILL.md   (the design: what it should do + how to drive it)
            │
            ▼
docs/testing/e2e-*.md            (runbook: drive the real surface, run the CRUD /
            │                     permissions / access / functional checklist)
      pass ─┴─ fail
       │        │
       │        ▼
       │   debugging/<area>/<symptom>.md   (a bug found → FILED HERE, not in this folder)
       │        │
       ▼        ▼
   regression test lands back in the real suite  (bug stays dead)
```

The `cargo test` / vitest suites already ran and passed **before** this loop starts —
that's the scope/session that built the feature. This folder picks up from a green build
and asks the next question the suite can't: *does the real running system do it?* The
runbook's job ends at "pass or fail". A **pass** is an observed result against the design.
A **fail** is not written up in `docs/testing/` — it's a *finding* that gets filed as a
`debugging/` entry with a regression test. That separation is the whole point: this folder
is "prove the running product works", `debugging/` is "here's what broke and how it was
fixed".

- **Green?** Record the observed result in the session doc (`../ABOUT-DOCS.md` — green is
  a claim that must be *shown*), **leave the artifacts in place**, and hand the user the
  URL/page to confirm it themselves (see "Leave it inspectable"). Done.
- **Red and non-trivial?** Open a `debugging/<area>/<symptom>.md` entry per
  `../scope/debugging/debugging-scope.md`, fix at the right layer, add a **regression
  test** that fails-before/passes-after, and cross-link session ↔ debug ↔ test.

That's the whole contract: the runbook *produces* either a green log or a debug entry +
regression test. It never leaves nothing behind.

---

## Leave it inspectable — the user confirms it at the end

Real-world testing isn't done when *you* say it passed — it's done when **the user can look
and confirm it themselves.** So the last step of every run is to **leave the system in the
state you tested it in**, with the evidence still on screen.

- **Do not clean up. Do not delete what you created.** If you created a widget, a
  dashboard, a datasource, a chart — **leave it in place**, in the running system, so the
  user can open the app and see it. Tearing it down at the end erases the very proof the
  user needs to confirm.
- **This is the opposite of the automated suite.** A `cargo test` seeds and tidies up after
  itself (ephemeral `mem://`, fresh per test) — correct there, because no human inspects it.
  Here a human *does* inspect it, so the artifacts must **survive the run**.
- **End with a hand-off, not a teardown.** Your final response tells the user exactly how
  to see it for themselves: the URL / page / verb to open, what they should expect to see,
  and that it was left in place on purpose. Example:
  > ✅ Left a datasource **`demo-buildings`** and a chart on an **Energy kWh** point in place
  > on the running node (`make dev`, http://127.0.0.1:8080). Open **Datasources → demo-buildings**
  > and the **Energy** chart to confirm — I did **not** delete them so you can check.
- **CRUD's `delete` step is the one exception, handled explicitly.** You still *prove*
  delete works — but do it on a throwaway record you created **for** that check, and say so.
  Leave the primary artifact (the one the user should inspect) untouched. Never prove
  "delete works" by deleting the thing you want the user to see.

If a run must leave the system dirty to be inspectable, that's **correct** — note what you
left and where, so the user (or the next session) isn't surprised by it.

---

## Should this be a skill?

Yes — and the two runbooks are written to be **drop-in `.claude/skills/` compatible**
(YAML frontmatter with a `description` that says *when to use it*), so an AI auto-selects
them when asked to e2e-test. Authoring them here in `docs/testing/` keeps them versioned
with the rest of the testing docs; a `skills/e2e-backend/SKILL.md` symlink/copy can expose
them to the agent skill loader once we wire it. Until then, an AI reads them directly.

Do **not** duplicate them into `debugging/` or `public/` — one source, cross-linked.

---

## Open questions

- Do we promote these runbooks into `.claude/skills/e2e-*/SKILL.md` now, or keep them
  doc-only until the skill loader indexes `docs/testing/`?
- Backend e2e: is there a first-class `rust/e2e/` workflow suite yet, or do the driven
  flows live inside `rust/crates/host/tests/`? (Currently the latter — see
  `e2e-backend.md`.)
- Do we add a `pnpm test:e2e` (Playwright, browser) runbook as a third file, or is
  `test:gateway` (jsdom + real node) sufficient for the frontend seam?
