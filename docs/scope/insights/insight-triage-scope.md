# Insights scope — triage: ownership + comments

Status: **shipped** (slice 3 of issue #119) — session
[`insight-triage-session.md`](../../sessions/insights/insight-triage-session.md), promoted to
`doc-site/content/public/insights/insights.md`. See "Open questions after building" at the foot.

The shipped record answers *what fired* and *who last moved the status*. It cannot answer
**"who owns this"** or **"what did we find out"**. An operator triaging a roster of open
findings needs to assign one to a person and leave a note for the next person — today they
do it in a spreadsheet beside the app, because `status_by` is the acker (a fact about a
transition) and `body` is producer-owned JSON (a re-raise-stable statement of the machine's
finding, not a place for human prose). This scope adds the **human triage plane** on the
insight: one nullable `assigned_to` axis and an append-only comment thread, each with its
own verb and cap.

## Goals

- Assign an insight to a member (and re-assign / un-assign), visible on the roster.
- Append timestamped, attributed comments — many per insight, never one overwritten field.
- Filter/sort the roster by assignee (`insight.list { assigned_to }`), including the
  "assigned to me" and "unassigned" cases an operator actually opens the page for.
- Survive dedup: a re-raise of an assigned insight must NOT drop the assignment or the thread.
- Keep the roster page cheap — a comment thread never rides `insight.list`.

## Non-goals

- **Not a workflow engine.** No assignment approval, no reassignment routing, no SLA timers,
  no escalation-on-unacked. If a vertical needs those, they compose on top via rules.
- **Not notification.** Assigning does not page the assignee in v1 (resolved decision 1) — the
  shipped subscription/notify ladder is subject-matched, not assignee-matched, and adding an
  assignee arm is the named first follow-up.
- **No comment edit/delete in v1.** The thread is an append-only operational log. Correction =
  another comment. (Unlike the occurrence ring, it does **not** evict — resolved decision 4.)
- **Not the producer's narrative.** Trigger logic / normalised metric / benchmark /
  deviation / estimated impact are the *machine's* statement and belong to
  `insight-evidence-scope.md` (see the sibling extension), not to this human plane.
- **No new dimension columns.** Group / building / asset type / data type / priority /
  category / classification are low-cardinality facets and ride the **shipped tag graph**
  (umbrella §"Tags"); per-asset identity (`WM-CHU-01`) rides `dedup_key`. This scope adds
  nothing for them, deliberately — see "Intent" below.

## Intent / approach

Two asymmetric shapes, because the data is asymmetric:

1. **`assigned_to: Option<String>` — a column on `Insight`.** Single-valued, low-cardinality,
   needed on every row of a filtered roster. A tag would be wrong on all three counts: it's
   mutated constantly (tag edges are provenance-stamped facts, not mutable cells), it's
   per-member (`user:…` values against the 10k tag-node cap is survivable but pointless), and
   `tags.find` intersection is the wrong query for "sort my roster by owner". It sits beside
   `status_by`/`status_ts` as a third small nullable axis, not inside `body`.

2. **Comments — a per-insight append-only child list, reusing the occurrence ring's *storage
   shape* but not its retention policy.** Comments are many and only ever read in the detail
   drawer, so the physical layout is the occurrence ring's (`insight-occurrences-scope.md`): a
   size-capped, seq-numbered child list under the parent, never joined into `list`. A
   `Vec<Comment>` on the parent record would instead re-serialize the whole thread on every
   raise (the hot path) and inflate a record the umbrella keeps deliberately narrow.

   **But they do not evict.** This is the one place this scope diverges from the ring it
   borrows from, and it's deliberate (resolved decision 4): eviction is right for firings
   (machine-generated, individually low-value) and a trust failure for human notes. Instead of
   an eviction cap, a per-insight **count cap that refuses the write** — a thread that long
   means the finding should have become a work item, and the platform should say so rather than
   quietly deleting the oldest note. Comments are purged only **with** their insight.

**The dedup rule is the load-bearing decision.** Assignment and comments are **human facts
about the finding**, so they behave like neither `title`/`body` (first-raise-wins) nor
`evidence` (refreshes on re-raise): they are **untouched by raise, forever**. A raise cannot
set, clear, or read them — there is no `assigned_to` on `RaiseInput`. This is stronger than
"first-raise-wins" and it is the point: a flapping sensor re-raising every 15 minutes must
never silently un-assign the technician who took the job. The re-open arm clears
`status_by`/`status_ts` (a fresh *lifecycle*); it must **not** clear `assigned_to` — the fault
came back and it's still Priya's, and the thread explaining why is the most valuable thing
on the record at that moment.

**Rejected: a generic `insight.update` verb** taking a partial record. It's one verb and one
cap for "change any field", which collapses the distinction the umbrella made on purpose
(§"MCP surface": *no `update`/`delete` in v1 — it's an operational record*). A producer-grade
`raise` grant would then also buy the power to rewrite human triage state, and the deny path
stops being expressible. Two narrow verbs (`assign`, `comment`) keep one cap per capability.

**Rejected: comments as a channel thread.** The messaging plane already threads
(`channels-threading`), and "attach a channel thread to an insight" is tempting reuse. It
fails the workspace-narrowness test in practice: a channel is a *delivery* surface with its
own membership and notification semantics, so an insight comment would become visible/paging
by channel rules rather than by insight read caps, and `insight.get` would need a cross-plane
read to render a drawer. The occurrence-ring precedent keeps the record self-contained.

## How it fits the core

- **Tenancy / isolation:** unchanged — `assigned_to` is a field on the existing
  `insight:{ws}:{id}` record; comments are child rows under the same ws-scoped parent, keyed
  like occurrences. ws-B cannot read/assign/comment on a ws-A insight; the assignee filter is
  a filtered scan inside the workspace namespace. No cross-workspace assignment: the assignee
  must be a member (or team) **of this workspace**, validated at assign time (resolved
  decision 2) — so assignment cannot be used to name a subject from another tenant.
- **Capabilities:** two new per-verb grants, both **member-act grade**, beside
  `mcp:insight.ack|resolve:call`:
  - `mcp:insight.assign:call` — assign / re-assign / un-assign.
  - `mcp:insight.comment:call` — append a comment.
  - Reads ride the existing `mcp:insight.get:call` (thread in the drawer) and
    `mcp:insight.list:call` (the `assigned_to` filter). **No new read cap** — the thread is
    part of the finding's detail, and a reader who may `get` the insight may read its notes.
  - Deny is opaque (the shipped 403 shape). A producer holding only
    `mcp:insight.raise:call` gets **no** triage write power — that separation is the whole
    reason these are distinct verbs, and it is a mandatory deny test.
  - `assigned_to` and comment `author` are **host-stamped-adjacent**: the *author* is always
    the principal's `sub` (never caller-supplied, the `ack` precedent at `ack.rs`); the
    *assignee* is necessarily caller-supplied (you assign to someone else) and therefore
    validated, not trusted.
- **Placement:** either. No reactor, no owner election — both verbs are plain writes on a
  local record, exactly like `ack`/`resolve`.
- **MCP surface (API shape, §6.1):**
  - **CRUD:** `insight.assign { id, assignee?: string|null }` → `{ assigned_to }` —
    one verb for assign/re-assign/un-assign (`assignee: null` clears). Idempotent:
    assigning the current assignee is a no-op success. `insight.comment { id, text }` →
    `{ seq }`. No `update`, no comment edit/delete (non-goals); the umbrella's
    no-`update` stance holds.
  - **Get / list:** no new read verbs. `insight.get` gains the full `comments` thread
    (`get`-only, never `list` — the `evidence` precedent; and since comments don't evict, the
    thread is complete, not a recent window). `insight.list` gains an `assigned_to` filter
    accepting a subject (`user:…` or `team:…`), the literal `"me"` (resolved host-side from the
    principal, **including teams the principal belongs to** — the roster's primary view), or
    `"none"` (unassigned, the triage queue's primary view). `insight.list` also echoes the
    scalar `assigned_to` so the roster renders an owner column without an N+1 — the same
    display-vs-detail boundary [`insight-tag-echo-scope.md`](insight-tag-echo-scope.md) draws
    for facets, and the reason the thread stays out.
  - **Live feed:** no new subject. Both verbs emit on the **existing**
    `ws/{ws}/insight/events` fire-and-forget subject so an open roster re-renders — a new
    subject for triage would fragment a stream the page already holds open (and see the
    SSE-pool lesson: one stream per surface, not per feature).
  - **Batch:** **yes, bounded-synchronous** — `insight.assign` accepts `ids: [..]` (bulk
    "assign these 12 to me" is the actual triage gesture; a roster with checkboxes is
    useless without it). Contract: **per-item results**, not all-or-nothing —
    `{ results: [{ id, ok, error? }] }`, capped at 100 ids per call, no I/O fan-out, so it
    stays a synchronous call and never becomes a job. Bulk comment: N/A (no caller).
- **Data (SurrealDB):** one new nullable field on the existing `insight` table; one child
  comment list per insight reusing the occurrence ring's storage shape (**but not its
  eviction** — resolved decision 4). **No new table, no new store.** Comments are bounded
  per-insight by a refuse-on-exceed count cap and per-comment size cap, and are purged **with**
  their parent insight — so the fleet-scale exposure is the parent table's, already tracked in
  the umbrella's "unbounded growth" risk, and the retention follow-up must delete comments as
  part of deleting an insight (not on their own schedule).
- **Bus (Zenoh):** the existing insight event subject only; fire-and-forget. State (who owns
  it, what was said) is SurrealDB's; the event is motion telling a page to re-read.
- **Sync / authority:** ordinary workspace data, node-local authoritative like the rest of the
  record. Two operators assigning concurrently is last-write-wins on a single scalar — no
  merge semantics needed, and the comment thread is append-only so it never conflicts.
- **Secrets:** none.
- **SDK/WIT impact:** **none.** No ABI change — an extension reaches these verbs through the
  existing host-callback MCP path under `caller ∩ install-grant`, and the new fields are
  additive/optional so an old client deserializes unaffected (the `evidence` rollout
  precedent).
- **Skill doc:** **YES** — extend `skills/insights/SKILL.md` with the triage walkthrough
  (assign → comment → filter `assigned_to: "me"` → bulk assign), grounded in a live run. Not a
  new skill: it's the same drivable surface, and a second doc would rot out of step.

## Example flow

The Chullora water-meter finding, triaged end to end:

1. A saved rule fires nightly and raises with the machine's facts only —
   `insight.raise { dedup_key: "rule:no-water-1d:WM-CHU-01", severity: "warning",
   title: "Chullora — no water usage in 1 day", tags: { building: "chullora-dc",
   asset_type: "water-meter", data_type: "water", priority: "medium",
   classification: "plumbing" }, evidence: { … }, analysis: { … } }`. Note where the 22
   columns went: identity in `dedup_key`, dimensions in `tags` (rendered as columns via the
   tag echo), the data binding in `evidence`, the producer's reasoning in `analysis`.
   Nothing human is set — `assigned_to` is `None` and the thread is empty.
2. An operator opens the roster filtered `{ status: "open", assigned_to: "none" }` — the
   triage queue. The Chullora row is there, unassigned.
3. They call `insight.assign { id, assignee: "user:priya" }`. Host checks
   `mcp:insight.assign:call`, validates `user:priya` is a member of this workspace, writes
   `assigned_to`, emits the insight event. The roster re-renders for everyone watching.
4. They add context: `insight.comment { id, text: "Site was shut for the long weekend —
   confirming with facilities before we roll a truck." }`. Author is stamped from the
   principal; `seq` is the next in the thread.
5. Overnight the rule fires again — same `dedup_key`, so `raise` bumps `count`/`last_ts`
   and refreshes `evidence`. **`assigned_to` is still `user:priya` and the comment is still
   there** (the dedup rule above). This is the case the regression test must pin.
6. Priya confirms the shutdown, comments the outcome, and calls `insight.resolve`. Two weeks
   later the meter genuinely fails: the same key re-raises, status goes back to `open`,
   `status_by`/`status_ts` clear — and the thread explaining last time's false alarm is
   exactly what the next responder reads first.

## Testing plan

Per `scope/testing/testing-scope.md`, against the **real** store (`mem://`) and a real
spawned gateway — no mocks, no fake backend (rule 9). Mandatory categories:

- **Capability deny (mandatory).** A token with `mcp:insight.get:call` but not
  `assign`/`comment` gets an opaque 403 on both writes. Critically: a **producer** token
  holding `mcp:insight.raise:call` and nothing else is denied both — the separation this
  scope exists to create. Per the deny-test lesson: assert the property only the outer gate
  has (a real id and a fictional id must produce **identical** errors, so a deny can't be
  distinguished from a not-found), and **revert-check** the gate — gut it and watch the test
  go red, or it's testing an inner layer.
- **Workspace isolation (mandatory).** ws-B cannot assign, comment on, read the thread of, or
  see in `list` a ws-A insight — with **identical** error shapes for "other workspace's real
  id" and "id that doesn't exist anywhere".
- **Offline / sync:** N/A beyond the record's existing behaviour (node-local writes, no
  cross-node authority).
- **Hot-reload:** N/A (core verbs, no extension instance state).

Key cases beyond the mandatory set:

1. **Dedup preservation (the load-bearing regression test).** Assign + comment, then re-raise
   the same `dedup_key` twice → `assigned_to` and every comment survive, `count` advanced.
   Then the harder arm: resolve → re-raise (the re-open path) → `status_by`/`status_ts`
   cleared **but** `assigned_to` and the thread intact. Revert-check by making the re-open
   arm clear `assigned_to` and confirming red.
2. **`raise` cannot write triage state.** A raise body carrying `assigned_to` /`comments`
   (a caller trying it on) is ignored — the field is absent from `RaiseInput`, so this is a
   serde-level assertion that a hostile producer can't reach the plane.
3. **Assign semantics.** Assign → re-assign → un-assign (`null`) round-trips; assigning the
   current assignee is an idempotent no-op; assigning a nonexistent insight errors like `ack`
   does. **Membership validation** (resolved decision 2): a `user:` non-member is refused, a
   `team:` of this workspace is **accepted**, and a member/team of *another* workspace is refused
   with the same opaque error as a subject that doesn't exist (a probe must not confirm that
   `user:ada` exists in ws-B).
4. **Author is un-forgeable.** A comment body supplying `author: "user:someone-else"` stores
   the principal's `sub` instead (the `ack.rs` host-stamp precedent).
5. **List filter.** `assigned_to: "me"` resolves to the calling principal **and their teams**
   (a team-assigned insight appears in the member's "mine" view — the case a naive sub-equality
   check silently drops); `"none"` returns exactly the unassigned; an explicit subject returns
   only that subject's; the filter composes with `status`/`severity`/tags and keyset paging
   without breaking the cursor.
6. **`get`/`list` boundary.** `insight.list` returns the scalar `assigned_to` (the owner column)
   but **never** `comments`; `insight.get` returns the full thread, newest-first.
7. **Comment caps refuse, never evict.** An oversize `text` rejects the whole call before any
   write (never silent truncation — the `validate_occurrence_size`/`validate_evidence_size`
   contract). Appending past the per-insight count cap **errors**, and the pre-existing thread is
   unchanged — assert the oldest comment is **still there** after the refused write. Revert-check
   by swapping in ring eviction and confirming red: this is the decision most likely to be
   "helpfully" reverted to match the occurrence ring it sits beside.
8. **Comments die with their insight, not before.** Delete the insight → its comments are gone
   (no orphan rows accumulating outside any retention sweep); a long-lived insight keeps its
   oldest comment indefinitely.
9. **Live verification in the product** (not just the suite — `cargo test` has historically
   not caught the real bugs here): assign and comment from the running node, confirm the
   roster re-renders off the existing SSE stream, shows the owner column, and the drawer
   shows the thread.

## Risks & hard problems

- **The dedup interaction is the whole risk.** Every other field on this record is either
  producer-owned or transition-owned; these two are neither, and the raise path is the hot
  path a future contributor will edit. Mitigation: the test in §1 above, plus a `SCOPE:`
  comment on the re-open arm in `raise.rs` naming this doc — the field is easy to clear by
  accident while "cleaning up the lifecycle", and nothing else in the record punishes that.
- **`assigned_to` is a subject string, and subjects outlive membership.** A member removed
  from the workspace leaves insights assigned to a `user:…` that can no longer read them —
  a silently-orphaned queue. Validation at assign time doesn't fix it (removal is a later
  event). Accepted (resolved decision 3): the record keeps the stale subject and the UI must
  render it as "unknown (removed)" rather than blank, so the orphaned queue is visible. The
  residual risk is real — nobody is *notified* that work lost its owner.
- **Not evicting trades a trust bug for a growth bug** (resolved decision 4, and the trade is
  deliberate). Notes are now durable for the life of the insight, so the exposure moves to
  volume: a long-lived flapping insight on a chatty site accumulates a thread nothing prunes
  until the parent is purged. The count cap bounds each thread and the retention follow-up must
  delete comments *with* their insight — but that follow-up "must land before any production
  fleet" (umbrella risk) is now load-bearing for a second reason. Getting a stale note deleted
  is a support request; getting a note the operator wrote silently deleted is a lost customer.
- **Bulk assign's partial failure.** 100 ids where 12 fail must not read as success. Per-item
  results are specified above; the UI must surface the failures rather than a green toast, or
  it silently drops work — and per the no-silent-caps rule, a call truncated at 100 must say so.
- **Roster width, and where the boundary now sits.** The record can express ~22 fields and the
  roster must not render all of them. The rule across the three scopes is **"does a column need
  it"**: `list` carries the scalars and facets (severity, status, `assigned_to`, tags) and
  **never** the payloads (comments, `evidence`, `analysis`). Assignee is the 6th column
  operators want; the thread is the 1st thing that would make every page expensive.
- **Three scopes must land together to finish the roster.** The dimension columns come from
  [`insight-tag-echo-scope.md`](insight-tag-echo-scope.md), the drawer content from
  [`insight-analysis-scope.md`](insight-analysis-scope.md), the owner column from here. Any one
  alone makes the roster look almost-done while a visible column stays blank — worth sequencing
  deliberately (tag echo first: it's the one the other two assume).

## Resolved decisions

Stated here rather than as open questions, so the implementing session has no ambiguity.

1. **Assigning does NOT notify in v1 — and the ladder gains an assignee match arm as the named
   follow-up.** Wiring notification through the shipped subject-matched ladder means a new match
   dimension (`assignee`), which is a change to `match_subs` and the subscription grammar — real
   work with its own deny-test surface, not a flag. Keeping it out means v1 assignment is a
   roster fact, which is honest and useful on its own. But "assigned and nobody told them" is the
   same trust bug the umbrella flags for "0 subscribers", so this is filed as the **first
   follow-up**, not an open musing. The UI must not imply a notification was sent.
2. **The assignee must be a workspace member, validated at assign time, and `team:` subjects are
   legal from v1.** Two parts, both long-term calls:
   - *Validated:* assigning to a subject that cannot read the insight is never intentional. One
     cheap membership read on a low-frequency verb.
   - *Teams allowed:* queue-style ownership ("the mechanical crew owns this") is how real triage
     works, and adding `team:` later would mean every consumer that parsed `assigned_to` as a
     user sub gets it wrong retroactively. Accepting both shapes from day one costs one validation
     branch now and avoids a breaking read-side change later. `assigned_to` is therefore
     documented as **a subject, not a user id** — the same discipline `status_by` already has.
3. **An insight assigned to a removed member keeps the stale subject, and the removal path is
   not touched.** A sweep needs a member-removal hook that doesn't exist, and inventing one for
   this is scope creep into identity. The record keeps what it was told; the UI renders an
   unresolvable assignee as "unknown (removed)" rather than blank, so an orphaned queue is
   *visible* instead of silently empty. Re-assignment is a human action, which is correct — the
   platform should not guess who inherits someone's work.
4. **Comments are NOT a ring — they are retained for the life of the insight.** This reverses the
   occurrence-ring reuse for storage *policy* while keeping it for storage *shape*. Eviction is
   right for firings (machine-generated, individually low-value, unbounded) and wrong for human
   notes: "we wrote it down and the platform deleted it" is a trust failure, and the note
   explaining last quarter's false alarm is the single most valuable thing on a re-opened
   finding. Bounding is instead:
   - a **per-comment size cap** (rejects loudly, never truncates), and
   - a **per-insight comment count cap** that **refuses the write** when exceeded rather than
     evicting silently — a thread that long means the insight should have become a work item, and
     failing loudly says so.
   Fleet-scale growth is then the parent table's retention problem, already tracked in the
   umbrella's "unbounded growth" risk, and comments are purged **with** their insight — never
   before it.
5. **`assigned_to` becomes subscription-filterable in the same follow-up as decision 1** — it's
   the same missing match arm, and "notify me about anything assigned to me" is the version of
   that feature people actually want.
6. **Bulk assign is capped at 100 ids with per-item results, and the cap is reported.** Never a
   silent truncation: a call passing more than 100 is an explicit error, and per-item failures
   are returned rather than folded into a success. The UI must surface partial failure — a green
   toast over 12 silent failures is the no-silent-caps rule broken at the last mile.

## Open questions after building

Written by the implementing session. Everything above is the ask as scoped; this is what shipping it
exposed.

1. **Assignment still notifies nobody, and that gap is now real rather than theoretical.** Resolved
   decision 1 deferred it knowingly, but v1 now ships the ability to *give someone work* with no way
   for them to hear about it — the same trust shape the umbrella flags for "0 subscribers". The
   follow-up (an `assignee` match arm in `match_subs` + the subscription grammar, decisions 1 and 5)
   should be sequenced next, before a vertical builds a workflow on top that assumes people are told.
2. **The `unknown (removed)` rendering is unenforceable from this repo.** Resolved decision 3 keeps a
   stale subject deliberately, and the visibility of the orphaned queue rests entirely on a
   downstream UI choice lb cannot check (the shell is out-of-tree). If `rubix-ai` renders an
   unresolvable assignee as blank, the record is honest and the product silently isn't. Worth an
   explicit consumer contract test there.
3. **A member who leaves can still be *newly* assigned to, briefly.** Validation is at assign time
   and membership is read then; nothing re-validates later, which is the accepted trade — but a bulk
   assign racing a removal writes a subject that was legal microseconds earlier. Harmless today
   (the record keeps what it was told, by design), worth naming before anyone builds auto-assignment.
4. **The comment count cap is per-insight, not per-workspace.** 200 comments × an unbounded insight
   table is still unbounded; the cap bounds a *thread*, not the fleet. The umbrella's retention
   follow-up now carries a second load-bearing requirement: it must delete comments **with** their
   parent and must never sweep them independently.
5. **Bulk assign is O(n) sequential store writes.** Fine at the 100-id cap on a human verb, and
   deliberately not a job (the scope requires it stay synchronous) — but it is the shape that will
   hurt first if the cap is ever raised. Raise the cap only with a batched write.

## Related

- Parent: [`insights-scope.md`](insights-scope.md) (the record, the tag-cardinality rule, the
  no-`update` stance, §"Tags" and §"MCP surface")
- Filed by the implementing session:
  [`insight-tag-precedence-scope.md`](insight-tag-precedence-scope.md) — the resolution of slice 1's
  multi-source tag question (`Human` > `Producer`), decided there because triage is where a human
  first disagrees with the machine
- Siblings: [`insight-occurrences-scope.md`](insight-occurrences-scope.md) (the child-list
  storage shape this reuses — **but not its eviction**, resolved decision 4);
  [`insight-analysis-scope.md`](insight-analysis-scope.md) (the producer's reasoning — where
  trigger logic / suspected cause / normalised metric / benchmark / deviation / impact belong,
  **not** here); [`insight-tag-echo-scope.md`](insight-tag-echo-scope.md) (the dimension
  columns; the display-vs-detail boundary this scope reuses for `assigned_to`);
  [`insight-evidence-scope.md`](insight-evidence-scope.md) (the data binding);
  [`insight-subscriptions-scope.md`](insight-subscriptions-scope.md) and
  [`insight-notify-scope.md`](insight-notify-scope.md) — **resolved decisions 1 and 5 land
  there** as the named first follow-up (an `assignee` match arm)
- `scope/tags/tags-scope.md` — the facet plane the dimension fields ride (10k-node cap)
- `scope/datasources/page-cursor-scope.md` — the keyset paging the `assigned_to` filter composes with
- `scope/testing/testing-scope.md` §0 (no mocks), `scope/debugging/debugging-scope.md`
- `skills/insights/SKILL.md` — extended by the implementing session with the triage walkthrough
- README §3 (rules 5–7: capability-first, workspace wall, MCP contract), §6.5
