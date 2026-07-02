# About these docs

How `docs/` is organized and which folder a given note belongs in.

The docs track a feature through three stages — **what we want → how we built it → what shipped**.
Each top-level folder is one stage. Inside each, group by topic (e.g. `tracing/`, `dashboards/`).

```
docs/
├── scope/      ← "hey, we want this"   — the ask, before any work
├── sessions/   ← the AI-agent sessions — what was done, while doing it
├── public/     ← production / shipped  — what actually got done
├── skills/     ← runnable how-to guides — drive a shipped surface over MCP/REST
├── vision/     ← the north star        — the big-picture framework direction
└── debugging/  ← the working history   — every issue and how it became working
```

Three areas sit outside the three-stage flow: `vision/` (why the platform exists),
`debugging/` (the append-only debugging memory — see "Testing & debugging" below), and
`skills/` (agent-runnable operational guides — see "`skills/`" below).

Top-level guides tie it together: `STATUS.md` (the live "where are we" dashboard),
`SCOPE-WRITTING.md` (how to write a scope), `HOW-TO-CODE.md` (how to build one), and
`FILE-LAYOUT.md` (how to lay out the code).

## The three stages

### `scope/` — what we want

The **intent**, written *before* the work. A scope doc says "we want X": the goal,
rough requirements, constraints, open questions. No implementation detail — it's the brief.

- One file per feature/ask, under a topic subfolder.
- Example: `scope/dashboards/multiple-dashboards-scope.md`
- Write this first. It's the thing you hand to an agent (or yourself) to go build.
- **Don't write it from scratch — follow [`SCOPE-WRITTING.md`](SCOPE-WRITTING.md)**, the
  playbook that turns a raw idea into a complete scope setup (doc + stubs + testing plan
  + index updates).

### `sessions/` — the AI agent session

The **working log** of an AI-agent (Claude Code) session — what was explored, decided,
and changed *while doing the work*. This is the messy middle: findings, dead ends,
the actual diffs and reasoning. Tied to a scope doc above it.

- One file per session, under the matching topic subfolder.
- Example: `sessions/dashboards/multiple-dashboards-session.md`
- Captures *how* it got done, not just that it did.

### `public/` — production, what got done

The **shipped, durable** documentation — the source of truth for what exists in
production now. Trimmed of session noise: deployment, architecture, the final scope
as built. This is what a new person reads.

- `public/SCOPE.md` — the project scope as actually built.
- `public/DEPLOYMENT.md` — runtime, ports, how to run it.
- `public/DIAGRAMS.md` — visual reference.
- Topic subfolders (`public/tracing/`, `public/ce/`) hold per-area shipped docs.

## `skills/` — runnable operational guides

Separate from the three-stage flow. A **skill** is a self-contained, agent-runnable how-to for
*driving a shipped surface* — the exact verbs, routes, payloads, and gotchas needed to operate a
capability over MCP / REST (or the CLI), grounded in the **real** API. Where `public/` explains *what
exists and why*, a skill is the *operating manual* an agent (or a person) follows to actually do the
thing, end to end, with copy-pasteable calls.

- One folder per skill: `skills/<name>/SKILL.md` (YAML frontmatter — `name` + a `description` that
  says *when to use it* — then the body). It is drop-in compatible with `.claude/skills/`.
- Write one when a feature ships (or changes) an **agent- or API-drivable surface**: a set of MCP
  verbs / gateway routes, a workflow someone drives programmatically, an automatable admin task.
  Example: `skills/dashboard-mcp/SKILL.md` — manage dashboards over `dashboard.*` + `/dashboards`.
- **Ground it in a live run.** Exercise the real gateway/node while writing it; paste real request/
  response shapes, not remembered ones. No `*.fake.ts` prose either (rule 9 applies to docs too).
- **Maintain it.** A skill is durable like `public/`: any later change to the surface it documents
  (a new verb, a changed payload, a renamed route) updates the skill in the SAME session — a stale
  skill mis-drives an agent, which is worse than none.

Not every feature needs a skill — a pure internal refactor with no external surface doesn't. But any
change to *how the platform is driven* must leave the relevant skill current.

## `vision/` — the north star

Separate from the three-stage flow. The **big-picture direction**: what the platform
*is* and where it's going (`vision/README.md` + numbered design notes). Read this to
understand *why*; read `scope → sessions → public` to follow a specific feature.

## How to use it

- **Starting something new?** Write a `scope/<topic>/<name>.md` — the ask.
- **Building it (esp. with an agent)?** Log it in `sessions/<topic>/<name>.md`.
- **Shipped it?** Promote the durable parts into `public/` (and update `public/SCOPE.md`).
- **Shipped a drivable surface (MCP verbs / REST routes / an automatable task)?** Write or update the
  matching `skills/<name>/SKILL.md` so an agent can operate it.
- **Want the big picture?** Read `vision/`.

Keep topic names consistent across folders (`tracing`, `dashboards`, `ce`) so a feature
reads top-to-bottom: scope → session → public.

---

## Rules for AI sessions (required)

**Documentation is part of every task, not an afterthought.** Each AI session must leave
the docs better than it found them. A session that changed code or decisions but wrote no
docs is **incomplete** — treat the doc as a deliverable equal to the code.

### At the start of a session

1. Read `README.md` (the core scope) and any `scope/<topic>/` doc covering the work.
2. Read `docs/FILE-LAYOUT.md` before writing code, and `CLAUDE.md` for the project rules.
3. **Create the session doc** `sessions/<topic>/<name>-session.md` from the template below.
   Use the *same topic name* as the scope/public folders so the feature reads top-to-bottom.

### While working

- Keep the session doc updated as you go — it is the working log, not a final report.
  Record what you explored, what you decided, dead ends, and *why*. The "why" is the value;
  the diff already shows the "what".
- When you make an architectural decision, write down the alternative you rejected and why.
- If you discover the scope was wrong or incomplete, update the `scope/` doc (or note the
  gap in it) — don't silently diverge.

### At the end of a session

- **Resolve or update open questions** in the relevant `scope/` doc.
- **Promote durable truth to `public/`** if something shipped: update the per-area
  `public/<topic>/` doc and `public/SCOPE.md`. The session log stays as the messy history;
  `public/` is the trimmed source of truth a new person reads.
- **Write/maintain the skill** if the change adds or alters an agent- or API-drivable surface:
  create or update `skills/<name>/SKILL.md`, grounded in a live run against the real node (paste real
  payloads). If the surface it documents changed, update it in this same session — a stale skill
  mis-drives agents. (N/A for changes with no external/drivable surface — say so.)
- Keep `README.md` section numbers stable — many docs cross-reference them (`§6.5`, etc.).
- Cross-link: scope ↔ session ↔ public for the same topic.

### Testing & debugging are part of the session

Every session also **tests its work and records its debugging.** These are deliverables,
not optional extras:

- **Test it.** Ship tests in the same session as the behavior change, following
  `scope/testing/testing-scope.md` — including the mandatory categories (capability
  deny-tests, workspace-isolation tests). "Tests later" is not allowed. Show the green
  output in the session doc.
- **Log the debugging.** When something doesn't work, open a `debugging/<area>/<symptom>.md`
  entry and keep it as you investigate, per `scope/debugging/debugging-scope.md`. On resolution,
  fill in root cause + fix, add a regression test, and update `debugging/README.md`.
- **Close the loop:** every fixed bug leaves behind a regression test and a completed
  debug entry, cross-linked to the session doc.

### Definition of done for a session

A session is done when: the work is complete **and** the session doc exists and is filled
in **and** the change is tested (mandatory categories included) **and** any debugging is
captured in `debugging/` with a regression test **and** any shipped change is reflected in
`public/` **and** any agent-/API-drivable surface it touched has a current
`skills/<name>/SKILL.md` **and** the scope doc's open questions are current. If any of those is
missing, the session is not done.

### Session doc template

The sections mirror the **definition of done** above — fill every one (write "n/a" with a
reason rather than deleting, so a reader knows it was considered). Driving a coding session
end to end? Follow [`HOW-TO-CODE.md`](HOW-TO-CODE.md), the execution playbook.

```markdown
# <Topic> — <short title> (session)

- Date: YYYY-MM-DD
- Scope: ../../scope/<topic>/<name>-scope.md
- Stage: <e.g. S1 — the spine> (STAGES.md)
- Status: in-progress | done | blocked

## Goal
One or two sentences: what this session set out to do, and the stage exit gate it targets.

## What changed
The concrete edits/decisions (link files as path:line where useful).

## Decisions & alternatives
- Chose X over Y because …
- Rejected Z because …

## Tests
What was tested, which mandatory categories apply (capability-deny, workspace-isolation,
offline/sync, hot-reload), and the **green command output pasted here** — green is a claim
that must be shown.

## Debugging
Links to any `debugging/<area>/<symptom>.md` entries opened this session, each with its
regression test. "None" if nothing broke.

## Public / scope updates
What was promoted to `public/<topic>/` + `public/SCOPE.md`, and which scope open questions
were resolved or refreshed.

## Skill docs
Which `skills/<name>/SKILL.md` was written or updated (the drivable surface this touched), and the
live run it was grounded in — or "n/a: no agent-/API-drivable surface" with the reason.

## Dead ends / surprises
What didn't work, and what was learned.

## Follow-ups
- Open questions pushed back to the scope doc
- TODOs for a future session
- STATUS.md updated? (slice/stage state)
```

---

## Publishing (Nextra)

The reader-facing docs are rendered by a [Nextra](https://nextra.site/) site in
[`../doc-site/`](../doc-site/README.md). It publishes `public/`, `skills/`, and `vision/`; `scope/`
and `sessions/` are working material and are not published by default. **Author here in
`docs/`** — never fork content into `doc-site/`.
