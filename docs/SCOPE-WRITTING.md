# Scope writing — the setup playbook

Hand this file to an agent along with a feature idea. The agent turns that idea into a
**complete, consistent scope setup**: the scope doc, the matching session/public stubs,
the testing plan, debug readiness, and all the index/cross-reference updates — following
this project's conventions instead of inventing new ones.

> **Read first (don't duplicate):** `ABOUT-DOCS.md` (the three-stage flow + AI-session
> rules + session template), `FILE-LAYOUT.md` (one responsibility per file),
> `scope/testing/testing-scope.md` (how to test), `scope/debugging/debugging-scope.md` (how to
> debug). This playbook is the *procedure*; those are the *standards*.

---

## 1. What you give the agent

A feature idea, at any fidelity — one sentence to a page. Examples:

- "We need a per-workspace rate limiter for MCP tool calls."
- "Add a Slack-style threading model to channels."
- "Scope the document-sharing permissions for the coding workplace."

The agent does the rest. If the idea is too vague to scope (the core decisions can't be
made), the agent asks **2–3 sharp questions first**, then proceeds.

---

## 2. What the agent produces

For a feature with topic name `<topic>` (kebab-case) and a specific ask `<name>`:

| Artifact | Path | When | State after setup |
|---|---|---|---|
| **Scope doc** | `scope/<topic>/<name>-scope.md` | now | Fully written (the deliverable). |
| **Session doc** | `sessions/<topic>/<name>-session.md` | when work starts | Created from the ABOUT-DOCS template, status `in-progress`. |
| **Public stub** | `doc-site/content/public/<topic>/<topic>.md` | now, if missing | A `TODO` placeholder, filled on ship. |
| **Skill doc** | `skills/<name>/SKILL.md` | on ship, **if** it exposes a drivable surface | Written/maintained by the implementing session, grounded in a live run. Scope names it; §6 flags whether one is needed. |
| **Debug area** | `debugging/<topic>/` | on first bug | Created lazily; entries per `debugging-scope.md`. |
| **Index updates** | see §7 | now | Cross-links wired both ways. |

The scope doc is the thing you actually review. The rest is scaffolding that keeps the
feature readable top-to-bottom (scope → session → public) and searchable.

---

## 3. Naming — one topic, used everywhere

Pick **one kebab-case `<topic>`** and reuse it across every folder so a feature reads
top-to-bottom. Match an existing topic if one fits (`caps`, `sync`, `mcp`, `jobs`,
`inbox-outbox`, …); only coin a new one for genuinely new surface.

- `scope/<topic>/<name>-scope.md`
- `sessions/<topic>/<name>-session.md`
- `doc-site/content/public/<topic>/<topic>.md`
- `debugging/<topic>/<symptom>.md`

`<name>` is the specific ask within the topic (often equal to `<topic>` for the first
scope, then more specific later — e.g. `channels-threading`).

---

## 4. Procedure

1. **Clarify if needed.** If the core decisions can't be made from the idea, ask 2–3
   questions. Otherwise proceed and state assumptions in the doc.
2. **Choose the topic** (§3). Reuse before coining.
3. **Read the neighbours.** Skim the existing `scope/<topic>/` and `README.md` sections it
   touches, so the new doc is consistent and references the right `§` numbers.
4. **Write the scope doc** from the template in §5, working through the platform
   checklist in §6. This is where the thinking goes.
5. **Create the public stub** if `doc-site/content/public/<topic>/<topic>.md` doesn't exist (TODO
   placeholder — it gets filled when the feature ships).
6. **Wire the indexes and cross-references** (§7).
7. **Run the self-check** (§8) before handing back.

The session doc and debug entries come later, *during* implementation — not at scope
time — but the scope doc must already name the testing plan and the risks so the
implementing session knows what "done" means.

---

## 5. Scope doc template

```markdown
# <Topic> scope — <short title>

Status: scope (the ask). Promotes to `doc-site/content/public/<topic>/` once shipped.

One-paragraph statement of what we want and why. No implementation detail — the brief.

## Goals
- The outcomes this must achieve.

## Non-goals
- What this explicitly does NOT cover (defer-list).

## Intent / approach
The shape of the solution at architecture altitude: the key idea and why it fits the
core principles (README §3). Name the alternative considered and why it was rejected.

## How it fits the core
Address each that applies (delete the rest) — see the checklist in SCOPE-WRITTING §6:
- **Tenancy / isolation:** how workspace-first isolation holds.
- **Capabilities:** what grants gate this; the deny path.
- **Placement:** local-only | cloud-only | either, and why.
- **MCP surface:** tools exposed/consumed — the **API shape** (§6.1): which of CRUD /
  get-list / live-feed (SSE/watch) / batch apply, and long batches as **jobs**.
- **Data (SurrealDB):** records/tables/buckets touched; state vs motion.
- **Bus (Zenoh):** subjects, message class (fire-and-forget | must-deliver | replay).
- **Sync / authority:** node-local vs cloud-authoritative; offline behavior.
- **Secrets:** any secret material and how it's mediated.

## Example flow
A concrete walkthrough (numbered) of the main path, like the worked examples.

## Testing plan
Which mandatory categories from `scope/testing/testing-scope.md` apply here
(capability deny-tests, workspace-isolation, offline/sync, hot-reload), plus the key
unit/integration/E2E cases. The implementing session must satisfy these.

## Risks & hard problems
The parts most likely to be underestimated (mirror README §11 style for this feature).

## Open questions
The decisions to resolve during implementation. Be specific and answerable.

## Related
Links: README `§x.y`, sibling scope docs, vision notes, key-stack rows.
```

---

## 6. Platform checklist (work through while scoping)

A scope for *this* platform isn't done until it has considered the principles in
`README.md` §3. For each, either address it in "How it fits the core" or state it's N/A:

- [ ] **Workspace is the hard wall** — every key scoped by workspace; isolation tested.
- [ ] **Capability-first** — what grant is required; what the deny looks like.
- [ ] **Symmetric nodes** — no `if cloud {…}`; differences are config/role only.
- [ ] **One datastore** — SurrealDB only; no new persistence layer sneaking in.
- [ ] **No mocks / no fake backend** (CLAUDE §9, `scope/testing/testing-scope.md` §0) —
  the testing plan proves this feature against the **real** store/bus/gateway, seeded
  with real records. No `*.fake.ts` or in-memory re-implementation. A fake is allowed
  only for a true external (provider HTTP, GitHub), behind one trait in one named file —
  if the scope needs one, name it here.
- [ ] **State vs motion** — SurrealDB for state, Zenoh for messages; not mixed.
- [ ] **Stateless extensions** — no durable state in an instance (hot-reload safe).
- [ ] **MCP is the contract** — capabilities exposed as MCP tools.
- [ ] **API shape** — the right verbs for *this* feature (see §6.1): CRUD + get, a live
  feed (SSE/watch), and/or batch — with batch backed by a **job** when it can run long.
- [ ] **Durability** — must-deliver effects go through the outbox, not raw pub/sub.
- [ ] **One responsibility per file** — the implementation plan respects FILE-LAYOUT.
- [ ] **SDK/WIT impact** — does this touch the stable plugin boundary? Flag it loudly.
- [ ] **Skill doc** — does this add or change an **agent-/API-drivable surface** (MCP verbs, gateway
  routes, an automatable task)? If so, name the `skills/<name>/SKILL.md` the implementing session must
  write and maintain (a runnable how-to grounded in a live run — see `ABOUT-DOCS.md` → "`skills/`").
  If not, state N/A. A stale/missing skill for a drivable surface is a finding, not a nicety.

If a scope violates a principle, that's a finding to surface — not something to paper over.

### 6.1 API shape — use judgment, don't default to one verb

Most features expose an API. Decide *which shapes it actually needs* — don't reflexively
ship a single `get`, and don't ship CRUD you have no caller for. Walk the four, name the
ones that apply in the scope's **MCP surface** section, and say why the rest are N/A:

- **CRUD (create / update / delete)** — the write verbs. Include only the mutations a
  caller really makes; a read-only roster (e.g. fleet presence) has *no* write verbs and
  should say so. Each write is its own MCP tool + capability (`...:create`, `...:update`,
  `...:delete`), one responsibility per file (FILE-LAYOUT).
- **Get / list** — the read verbs. Single-`get` by id and a `list` (with filter/paging)
  are usually distinct tools. State the workspace-scoped query and the read capability.
- **Live feed (SSE / watch)** — when a caller needs *changes*, not a snapshot. Prefer the
  bus (Zenoh sub / liveliness) for motion and surface it as a `watch` tool + the gateway's
  SSE route (§6.13); don't poll `list` on a timer. State vs motion (§3 rule 3) decides
  whether something is a record read or a stream.
- **Batch** — one call acting on many items (bulk create/update/delete, an import/export,
  a re-index). Decide the **partial-failure** contract (all-or-nothing vs. per-item
  results) up front.
  - **A batch that can run long MUST be a job, not a blocking call.** Enqueue a durable
    job (README §6.10, `jobs/jobs-scope.md`); the API returns a **job id**, and the caller
    watches progress via the job's status/feed. This keeps the request fast, survives a
    node restart mid-batch, and gives retries/backoff for free. A blocking loop over N
    items in a tool handler is a smell — it ties up the call, has no resume, and silently
    breaks past some N.
  - Small, bounded, always-fast batches (a handful of items, no I/O fan-out) may stay
    synchronous — but say so explicitly, with the bound.

If a write verb has a must-deliver side effect (it has to reach another node/service), it
goes through the **outbox** (§3 rule "Durability"), not raw pub/sub — the batch-job and
the outbox compose: the job does the work, the outbox delivers the effects.

---

## 7. Indexes & cross-references to update

- **`scope/README.md`** — add/confirm the topic is listed.
- **`doc-site/content/public/<topic>/<topic>.md`** — create the TODO stub if missing.
- **`skills/<name>/SKILL.md`** — if the feature exposes a drivable surface (§6), note that the
  implementing session owns writing/maintaining it; link it from the scope's "Related".
- **`README.md`** — if this adds a core component or changes one, update the relevant
  `§` and keep section numbers stable (many docs cross-reference them).
- **`key-stack.md`** — if it introduces a new library/crate/role, add the row.
- **`vision/README.md`** — if the artifact is a vision note, link it from the index.
- **Back-links** — the scope doc's "Related" section links out; the things it references
  should be reachable back. Cross-link scope ↔ public ↔ (later) session.

---

## 8. Definition of done for a scope setup

Hand back only when all are true:

- [ ] `scope/<topic>/<name>-scope.md` exists and is fully written (not a stub).
- [ ] The platform checklist (§6) is addressed or explicitly N/A.
- [ ] A **testing plan** names the mandatory categories that apply.
- [ ] The **skill doc** is decided: either the scope names the `skills/<name>/SKILL.md` the build will
  write, or it states N/A (no agent-/API-drivable surface) with the reason (§6 checklist).
- [ ] Open questions are specific and answerable.
- [ ] `doc-site/content/public/<topic>/<topic>.md` exists (at least as a TODO stub).
- [ ] Indexes and cross-references (§7) are updated.
- [ ] The doc matches the house voice: practical, decisive, recommends rather than surveys.

The session doc, tests, debug entries, and (when the feature exposes a drivable surface) the
`skills/<name>/SKILL.md` are produced **during implementation** per the AI-session rules in
`ABOUT-DOCS.md` — this playbook gets the feature ready to build.

---

## 9. Ready-to-use prompt

Copy this, paste your scope idea into it, and hand it to an agent:

```
Read docs/SCOPE-WRITTING.md and follow it.

Here is the scope idea:
<your feature idea — one sentence to a page>

Set up everything as needed: choose the topic, write the scope doc, create the public
stub, name the testing plan (and the skills/<name>/SKILL.md if this exposes a drivable
surface), work through the platform checklist, and update the indexes and cross-references.
Ask me 2–3 questions first only if you genuinely can't make the core decisions. Then show
me the scope doc.
```
