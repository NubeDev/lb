# How to code — the coding-session playbook

Hand this file to an agent along with a **scope doc** and the **current stage**. It turns
that ask into shipped, tested, documented work — following this project's conventions
instead of inventing new ones. It is the execution counterpart to `SCOPE-WRITTING.md`:

| Phase | Playbook | Produces |
|---|---|---|
| Plan the ask | `SCOPE-WRITTING.md` | `scope/<topic>/<name>-scope.md` |
| **Build the ask** | **this file** | code + tests + `sessions/…` + `debugging/…` + `doc-site/content/public/…` |

> **Read first (don't duplicate):** `STAGES.md` (which stage we're in + its exit gate),
> `FILE-LAYOUT.md` (one responsibility per file — read before writing any code),
> `scope/testing/testing-scope.md` (how to test + the mandatory categories),
> `scope/debugging/debugging-scope.md` (how to debug + the working history),
> `ABOUT-DOCS.md` (the session doc rules + template). This playbook is the *procedure*;
> those are the *standards*.

---

## 1. What you give the agent

- A **scope doc** (`scope/<topic>/<name>-scope.md`) — the ask, already written.
- The **current stage** from `STAGES.md` (or "figure it out from STATUS.md").

If there is no scope doc yet, stop and run `SCOPE-WRITTING.md` first. Code without a
scope is how the contracts come out wrong.

---

## 2. What the agent produces (the deliverables)

A coding session is **not done** until all of these exist. This is the same checklist as
`ABOUT-DOCS.md` "Definition of done", made concrete:

| Deliverable | Path | Always? |
|---|---|---|
| **Code** | `rust/<crate>/…`, `packages/<lib>/…` or `app/…`, within FILE-LAYOUT limits (**never `ui/` — it is deleted, see CLAUDE.md**) | yes |
| **Tests** | beside the source / `rust/<crate>/tests/` — incl. mandatory categories | yes |
| **Green output** | pasted into the session doc | yes |
| **Session doc** | `sessions/<topic>/<name>-session.md` | yes |
| **Debug entries** | `debugging/<area>/<symptom>.md` + `debugging/README.md` row | only if something broke |
| **Public promotion** | `doc-site/content/public/<topic>/<topic>.md` + `doc-site/content/public/SCOPE.mdx` | only if something shipped |
| **Scope updates** | open questions in `scope/<topic>/…` resolved or refreshed | yes |
| **STATUS.md** | mark the slice/stage state | yes |

The "only if" rows are not loopholes: a slice that touched no `doc-site/content/public/` and hit no bugs is
the exception, not the norm. If you wrote code, you wrote tests and you moved STATUS.

---

## 3. Procedure

1. **Locate yourself.** Read `STATUS.md` and `STAGES.md`: which stage, which slice, what
   is the **exit gate**? Restate the exit gate in your own words — it is your acceptance
   criterion. (`STAGES.md` rule: a stage is not done until its exit gate passes *and* its
   docs exist.)
2. **Read the scope.** The scope doc's "How it fits the core", "Testing plan", and "Open
   questions" *are* your task list and acceptance criteria. The platform checklist it
   addressed (workspace wall, capability deny, symmetric nodes, …) tells you which tests
   are mandatory here.
3. **Open the session doc** `sessions/<topic>/<name>-session.md` from the template in
   `ABOUT-DOCS.md`, status `in-progress`. Keep it updated *as you work* — it is the log,
   not a final report. The "why" is the value; the diff shows the "what".
4. **Slice vertically.** Build one capability through all layers (store → caps → bus → MCP
   → UI), not one crate in isolation (`STAGES.md` cross-cutting rule). Respect FILE-LAYOUT
   as you write: one verb per file, ≤400 lines hard, no `utils`/`helpers`/`common`.
4a. **Build the whole contract, not the easy half.** The scope's **MCP surface** (the full
   CRUD + get/list + live-feed + batch it named — `SCOPE-WRITTING §6.1`) is the deliverable.
   Ship **every verb the scope named**, wired end to end (store → cap → MCP → `http.ts` → UI),
   each with its own deny-test. Do **not** ship `get` and call it done because update/delete/
   list were "more work" — a half-wired surface is the thing that *looks* finished, then
   doesn't work and confuses everyone later. If a verb genuinely shouldn't exist yet, it is an
   explicit **scope non-goal** with a reason — never a silent gap. When in doubt, **do what's
   best long-term: the complete surface**, not the easiest subset. If building it all reveals
   the scope was wrong, fix the scope (step 8) — don't quietly trim it.
5. **Test in the same session — backend AND frontend.** Ship tests with the behavior
   change, never "tests later". **Both sides get tested, no exceptions:** Rust crate +
   integration tests for the backend, Vitest for the frontend. Add the mandatory
   categories that apply (capability deny, workspace isolation, and offline/sync or
   hot-reload where relevant) — and a deny-test **per verb** you built in step 4a. Run
   them; **paste the green output** into the session doc.
   - **Real infra, seeded data — never mock data** (`testing-scope.md` §0/§3.1, CLAUDE §9).
     Tests run against the real store (`mem://`), real bus, real caps, real gateway. When a
     test needs existing data, **seed real records through the real write path** — a DB seed,
     not a mocked response and not a `*.fake.ts`. This holds for frontend tests too: drive the
     UI against a real in-process node seeded with real rows, not a hand-written fake backend.
6. **Debug in the open.** If something non-trivially breaks, open a
   `debugging/<area>/<symptom>.md` entry and keep it as you investigate. On resolution:
   root cause + fix + a regression test that fails-before/passes-after, then update
   `debugging/README.md`. Cross-link session ↔ debug ↔ test.
7. **Promote what shipped.** Move durable truth into `doc-site/content/public/<topic>/<topic>.md`
   and update `doc-site/content/public/SCOPE.mdx`. (Public docs live in the doc-site now — MDX,
   authored directly under `doc-site/content/public/`; the old `docs/public/` is gone.) The session
   log stays as the messy history; `public/` is the trimmed truth a new person reads.
8. **Close the scope.** Resolve or refresh the scope doc's open questions; if you
   discovered the scope was wrong, fix it there — don't silently diverge.
9. **Move STATUS.** Update `STATUS.md`: the slice's state, and the stage's exit-gate
   status if you crossed it.
10. **Run the self-check** (§5) before handing back.

---

## 4. From scope to code — the bridge

The scope doc and the code map one-to-one. Use it as a worksheet, not background reading:

| Scope section | Becomes |
|---|---|
| Goals / Non-goals | The slice boundary — what you build now vs defer. |
| How it fits the core → Capabilities | A capability **deny-test** (mandatory). |
| How it fits the core → Tenancy/isolation | A **workspace-isolation** test (mandatory). |
| How it fits the core → Data / Bus | The SurrealDB records + Zenoh subjects to implement. |
| Extension surface (if any) | Reached only via generic seams — no core branch on the ext id (CLAUDE §10). If a core crate would name it, that's a leak: re-route through MCP/caps/outbox `Target`/`ext.list`. |
| How it fits the core → Sync/authority | An offline/sync test, if synced. |
| MCP surface | **Every** MCP tool the scope named (full CRUD + get/list + batch) — built end to end, not a subset (§3 step 4a) + their contract snapshot. |
| Testing plan | The test files to write (backend **and** frontend, real infra + DB seed) — start here, not last. |
| Open questions | Decisions to make *and record* in the session doc. |
| SDK/WIT impact (if flagged) | Stop and confirm before touching the stable boundary. |

---

## 5. Definition of done for a coding session

Hand back only when **all** are true (this consolidates the scattered checklists in
`ABOUT-DOCS.md`, `testing-scope.md` §5, `debugging-scope.md` §5 into one):

- [ ] The work satisfies the **scope** and the stage's **exit gate**.
- [ ] The **full API surface** the scope named is built end to end (every CRUD/get-list/batch
      verb wired store→cap→MCP→UI) — no easy-subset, no silently-dropped verb (§3 step 4a).
- [ ] Code respects **FILE-LAYOUT** (one verb/file, ≤400 lines, named concepts).
- [ ] No `if cloud {…}` — role differences are config only (symmetric nodes).
- [ ] **No core knowledge of any extension** — no core crate or core UI shell branches on
      an extension id (`if ext == "github"`, `match id`, static import, hardcoded
      route/nav/cap); extensions are reached only through the generic mediated seams (MCP
      `<id>.<tool>` dispatch, cap grammar, outbox `Target`, `ext.list` discovery), which
      treat the id as opaque data. A "built-in" ext takes the same caps/auth path as any
      other. Swapping an equivalent extension must need **zero** core-crate change
      (CLAUDE §10). Test fixtures / doc-comment examples don't count as branches.
- [ ] **No mock data / no fake backend** — tests run on real infra, seeded via the real write
      path (CLAUDE §9, `testing-scope.md` §0).
- [ ] Tests exist on **both backend and frontend**, including the **mandatory categories** and
      a **deny-test per verb**, and the **green output is pasted** in the session doc.
- [ ] Every bug fixed this session has a **regression test** and a closed **debug entry**.
- [ ] `sessions/<topic>/<name>-session.md` is filled in (not a stub).
- [ ] Anything shipped is in **`doc-site/content/public/`** and `doc-site/content/public/SCOPE.mdx`.
- [ ] The scope doc's **open questions** are current.
- [ ] **`STATUS.md`** reflects the new state.
- [ ] scope ↔ session ↔ public ↔ debug are **cross-linked**.

If any box is unchecked, the session is **incomplete** — say so, don't claim done.

---

## 6. Ready-to-use prompt

Copy this, fill the two blanks, and hand it to an agent:

```
Read docs/HOW-TO-CODE.md and follow it.

Scope: docs/scope/<topic>/<name>-scope.md
Stage: <e.g. S1 — the spine> (or: "read STATUS.md to find where we are")

Build this slice end to end and COMPLETE — every verb the scope's MCP surface named
(full CRUD + get/list + batch), wired store→cap→MCP→http.ts→UI, not just the easy
subset. Write the code within FILE-LAYOUT. Ship tests on BOTH backend and frontend
(including the mandatory capability-deny and workspace-isolation tests, plus a deny-test
per verb) — real infra seeded via the real write path, NO mock data and NO fake backend
— and paste the green output. Log any debugging, promote what shipped to doc-site/content/public/, update
the scope's open questions, and move STATUS.md. Then show me the session doc and the test
output. Do what's best long-term, not what's easiest now; if you must defer a verb, say so
explicitly as a scope non-goal — never leave a silent gap.
```
