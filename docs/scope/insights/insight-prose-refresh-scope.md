# Insights scope — `title`/`body` refresh on re-raise

Status: scope (the ask) — tracked as issue
[#124](https://github.com/NubeDev/lb/issues/124). Filed 2026-07-30 by the `analysis` implementing session
([`sessions/insights/insight-analysis-session.md`](../../sessions/insights/insight-analysis-session.md))
per [`insight-analysis-scope.md`](insight-analysis-scope.md) resolved decision 6 — **filed, not fixed
inline**, because it changes shipped semantics.

Re-raising an existing `dedup_key` bumps `count`/`last_ts` and refreshes `severity`, `evidence`,
`analysis`, and the `tags` echo — but **`title` and `body` stay first-raise-wins**. That was
defensible while the record carried only prose: the narrative was roughly stable, and overwriting it
lost the operator's original framing for no gain.

`analysis` shipping is what makes it indefensible. The record now carries *three* dedup behaviours,
and the drawer renders the refreshing ones directly above the frozen ones:

| Field class | On re-raise | Why |
|---|---|---|
| `title`, `body` | **first-raise-wins** | ← the subject of this scope |
| `severity`, `evidence`, `analysis`, `tags` | refresh / recompute | bindings + projections of *current* truth |
| `assigned_to`, comments | untouched | human facts a machine must never overwrite ([triage](insight-triage-scope.md)) |

So a finding on its 47th firing shows a freshly-computed `deviation: -100%` beside a `body` describing
firing #1's reading, under a `title` naming a condition that may have changed severity twice since.
The reasoning is right, the narrative is stale, and nothing on the record tells a reader which is
which. That is worse than either behaviour applied consistently.

## Goals

- `title` and `body` **refresh on supply** on the dedup arm — the rule `evidence`/`analysis` already
  hold: a raise that supplies one overwrites, a raise that omits it leaves the stored value alone.
- The record's dedup story becomes **two** classes, not three: *producer-owned fields refresh*,
  *human-owned fields don't*. That is a rule a contributor can hold in their head; the current
  three-way split is why each scope has had to restate it.
- No new verb, no new capability, no new field, no migration.

## Non-goals

- **Not a history of prose.** No `title_history`, no per-firing title on the occurrence ring. If the
  narrative's evolution matters, that is the ring's `data` or the (human) comment thread, not a
  fourth copy of the title.
- **Not touching `origin`.** Provenance is a fact about the *first* raise's producer and stays
  first-raise-wins. A re-raise from a different door is a real question, but not this one.
- **Not touching the human plane.** `assigned_to` and comments stay untouched by raise
  ([triage](insight-triage-scope.md)) — that boundary is the whole point of splitting the classes.

## Intent / approach

Two lines in `crates/insights/src/raise.rs`'s dedup arm, beside the existing `evidence`/`analysis`
arms. `title` is a required field on `RaiseInput` (so "omitted" needs a decision — see the open
question), `body` is already `Option`-shaped via its `Value::is_null` skip.

**Why refresh rather than keep the first title.** The counter-argument is real: an operator who
recognises a finding by its title will see it change under them, and the *original* framing —
possibly the one they wrote a comment about — is gone with no history. That is a genuine loss. It
loses to the consistency argument for one reason: the record already refreshes everything else, so
the current behaviour doesn't *preserve* a coherent firing-#1 snapshot either. It preserves two of
eight fields, which reads to a consumer as a bug rather than as a design. If prose history is wanted,
it should be wanted explicitly, on the ring, and not achieved by accident through a frozen field.

## How it fits the core

- **Tenancy / isolation, capabilities, placement, bus, secrets:** all unchanged. Same gated verb
  (`mcp:insight.raise:call`), same record, same workspace key.
- **MCP surface:** no new verb and no new field. `insight.raise` behaviour changes; `get`/`list`
  shapes do not.
- **Data:** no migration. Existing records keep their stored prose until their next raise, then heal.
- **SDK/WIT:** none.
- **Skill doc:** `skills/insights/SKILL.md` currently states first-raise-wins in the dedup
  walkthrough — it must be corrected in the same session, or it becomes the stale-skill finding.

## Testing plan

Per `scope/testing/testing-scope.md`: real store (`mem://`), real spawned gateway, no mocks. The
mandatory capability-deny and workspace-isolation tests are already covered for `insight.raise` and
need re-pinning only if this scope touches the gate (it does not).

The load-bearing case is a **behaviour-change** test, so it must assert the new semantics explicitly
rather than by omission:

1. **Refresh.** Raise `{title: A, body: X}` → re-raise same key `{title: B, body: Y}` → stored record
   is `B`/`Y`, `count: 2`. This is the assertion that currently exists **inverted**, in
   `crates/host/tests/insight_evidence_test.rs`
   (`re_raise_refreshes_evidence_but_leaves_title_and_body_first_raise_wins`) — that test's title and
   its two `first-raise-wins` assertions must be **updated, not deleted**, and the same session should
   say so, because a silently-removed assertion is indistinguishable from a lost requirement.
2. **Omit means unchanged.** A re-raise with a null/absent `body` leaves the stored `body` alone
   (the `evidence` precedent). Revert-check: make the arm unconditional → red.
3. **The human plane is still untouched.** Once [triage](insight-triage-scope.md) ships, a re-raise
   that refreshes `title`/`body` must still leave `assigned_to` and the comment thread alone — the
   two classes must not be collapsed by a contributor who reads "raise refreshes prose" too broadly.
4. **Live verification in the product**, per the area's history of the suite missing the real bug.

## Risks & hard problems

- **It is a semantics change to a shipped verb**, and the consumer most affected is a human's memory
  of a roster row rather than any code. Nothing will fail; a title will just differ from what someone
  remembers. That is precisely the kind of change that needs to be in a release note, and the kind
  no test can flag.
- **A required `title` has no "omitted" state.** `RaiseInput.title: String` means a producer that
  wants to bump a count without restating the title must send the same string back, and one that
  sends `""` would blank a real title under a naive refresh. See the open question.
- **The frozen title is load-bearing for someone.** A downstream consumer may key a saved view, an
  export column, or an operator's mental model on a title that has never moved. Unknowable from here;
  worth asking before shipping rather than after.

## Open questions

1. **Does an empty `title` refresh or preserve?** `title` is required on `RaiseInput`, so unlike
   `body` there is no honest "absent". The candidates: treat `""` as "omitted" (preserve), or refresh
   faithfully (a producer sending `""` gets a blank title). Preserving is the safer default and makes
   `""` a de-facto sentinel, which is the kind of thing that later needs undoing. **Recommend**
   treating `""` as omitted and saying so in the doc comment — but this is the one decision this scope
   should not make unilaterally, since it is the difference between a producer bug being visible and
   being silently swallowed.
2. **Is `severity`'s existing refresh worth restating here?** It already refreshes (and drives the
   escalation breakthrough), so it belongs in the "producer-owned refreshes" class — but it is not
   prose and this scope shouldn't touch it. Mentioned so the two-class table in the docs is complete.

## Related

- **Parent / why this exists now:** [`insight-analysis-scope.md`](insight-analysis-scope.md) —
  resolved decision 6 filed this scope; its dedup table is the one this simplifies to two classes.
- [`insight-evidence-scope.md`](insight-evidence-scope.md) — its **Q1** is the original musing
  ("should `title`/`body` refresh?"), closed as "decided elsewhere" by the analysis scope and
  answered here. Its test file holds the assertion this scope inverts.
- [`insight-tag-echo-scope.md`](insight-tag-echo-scope.md) — the third refreshing field.
- [`insight-triage-scope.md`](insight-triage-scope.md) — the *untouched* class; the boundary this
  scope must not blur.
- `insights-scope.md` §"Dedup / flap suppression" — the umbrella statement of the dedup contract,
  which this scope updates.
