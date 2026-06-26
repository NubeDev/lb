# How to code — the coding-session playbook

Hand this file to an agent along with a **scope doc** and the **current stage**. It turns
that ask into shipped, tested, documented work — following this project's conventions
instead of inventing new ones. It is the execution counterpart to `SCOPE-WRITTING.md`:

| Phase | Playbook | Produces |
|---|---|---|
| Plan the ask | `SCOPE-WRITTING.md` | `scope/<topic>/<name>-scope.md` |
| **Build the ask** | **this file** | code + tests + `sessions/…` + `debugging/…` + `public/…` |

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
| **Code** | `rust/<crate>/…` or `ui/…`, within FILE-LAYOUT limits | yes |
| **Tests** | beside the source / `rust/<crate>/tests/` — incl. mandatory categories | yes |
| **Green output** | pasted into the session doc | yes |
| **Session doc** | `sessions/<topic>/<name>-session.md` | yes |
| **Debug entries** | `debugging/<area>/<symptom>.md` + `debugging/README.md` row | only if something broke |
| **Public promotion** | `public/<topic>/<topic>.md` + `public/SCOPE.md` | only if something shipped |
| **Scope updates** | open questions in `scope/<topic>/…` resolved or refreshed | yes |
| **STATUS.md** | mark the slice/stage state | yes |

The "only if" rows are not loopholes: a slice that touched no `public/` and hit no bugs is
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
5. **Test in the same session.** Ship tests with the behavior change — never "tests
   later". Add the mandatory categories that apply (capability deny, workspace isolation,
   and offline/sync or hot-reload where relevant). Run them; **paste the green output**
   into the session doc.
6. **Debug in the open.** If something non-trivially breaks, open a
   `debugging/<area>/<symptom>.md` entry and keep it as you investigate. On resolution:
   root cause + fix + a regression test that fails-before/passes-after, then update
   `debugging/README.md`. Cross-link session ↔ debug ↔ test.
7. **Promote what shipped.** Move durable truth into `public/<topic>/<topic>.md` and
   update `public/SCOPE.md`. The session log stays as the messy history; `public/` is the
   trimmed truth a new person reads.
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
| How it fits the core → Sync/authority | An offline/sync test, if synced. |
| MCP surface | The MCP tool(s) to expose + their contract snapshot. |
| Testing plan | The test files to write — start here, not last. |
| Open questions | Decisions to make *and record* in the session doc. |
| SDK/WIT impact (if flagged) | Stop and confirm before touching the stable boundary. |

---

## 5. Definition of done for a coding session

Hand back only when **all** are true (this consolidates the scattered checklists in
`ABOUT-DOCS.md`, `testing-scope.md` §5, `debugging-scope.md` §5 into one):

- [ ] The work satisfies the **scope** and the stage's **exit gate**.
- [ ] Code respects **FILE-LAYOUT** (one verb/file, ≤400 lines, named concepts).
- [ ] No `if cloud {…}` — role differences are config only (symmetric nodes).
- [ ] Tests exist, including the **mandatory categories** that apply, and the **green
      output is pasted** in the session doc.
- [ ] Every bug fixed this session has a **regression test** and a closed **debug entry**.
- [ ] `sessions/<topic>/<name>-session.md` is filled in (not a stub).
- [ ] Anything shipped is in **`public/`** and `public/SCOPE.md`.
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

Build this slice end to end: write the code within FILE-LAYOUT, ship the tests
(including the mandatory capability-deny and workspace-isolation tests) and paste the
green output, log any debugging, promote what shipped to public/, update the scope's
open questions, and move STATUS.md. Then show me the session doc and the test output.
```
