# Session — insights: triage (`assigned_to` + the comment thread)

Status: **shipped** (slice 3 of GitHub issue #119 — the human triage plane).
Scope: [`../../scope/insights/insight-triage-scope.md`](../../scope/insights/insight-triage-scope.md).
Branch: `master` (no commits made — the human owns git).
Date: 2026-07-30.

## The ask, restated

The shipped record answered *what fired* and *who last moved the status*. It could not answer **"who
owns this"** or **"what did we find out"** — so operators triaged in a spreadsheet beside the app.
`status_by` is the acker (a fact about a transition) and `body` is producer-owned JSON (a
re-raise-stable statement of the machine's finding), so neither could host human prose. This slice
adds the human plane: one nullable `assigned_to` axis and an append-only comment thread, each with
its **own verb and its own cap**.

Sequencing note: slices 1 (tag echo → dimension columns) and 2 (analysis → the drawer's reasoning)
shipped first; this is the third of the three the scope says must land together to finish the roster.
The owner column was the last blank one.

## What shipped

| Layer | Change |
|---|---|
| Type | `Comment` (`cseq`/`text`/`author`/`ts`) + `MAX_COMMENT_BYTES` (4 KB) + `MAX_COMMENTS_PER_INSIGHT` (200) + `validate_comment` — `rust/crates/insights/src/comment.rs` (new; the shape + its guards) |
| Record | `Insight.assigned_to: Option<String>` — `#[serde(default)]` + `skip_serializing_if`, so a pre-field row still decodes (`insight.rs`) |
| Verbs (crate) | `assign.rs` (set/re-assign/clear, idempotent), `comment_append.rs` (the refuse-don't-evict append), `comments.rs` (the whole thread, newest-first) |
| Raise | **Nothing added** — and that is the feature. No `assigned_to` on `RaiseInput`; the create arm sets `None`; the dedup arm carries the prior through untouched. A `SCOPE:` comment now guards the re-open arm (`raise.rs`) |
| Read | `list.rs` gains an `assigned_to` filter + the new `AssigneeFilter` resolution seam; `assigned_to` is deliberately **not** stripped (owner column), `comments` never rides `list` |
| Delete | `delete.rs` cascades the comment thread beside the occurrence ring |
| Caps | `mcp:insight.assign:call` + `mcp:insight.comment:call`, member-act grade beside `ack`/`resolve` (`builtin_roles.rs`) |
| Host | `assign.rs` (bulk + cap), `assignee.rs` (membership validation + `"me"` resolution), `comment.rs`, `comments.rs`, `triage_event.rs`; `insight.get` composes the thread in the MCP bridge |
| Bus | `EventKind::Assign` / `EventKind::Comment` on the **existing** `ws/{ws}/insight/events` subject |
| Frontend | `Comment`, `Insight.assigned_to`/`comments`, `ListFilter.assigned_to`, the two client methods, and the two new event kinds on the canonical TS shape (`packages/insights`) — plus the reference `memoryClient` gained real `assign`/`comment` implementations |

No generic `insight.update`, no new table, no new read cap — as the scope requires.

## Decisions made while building

**1. `AssigneeFilter` is a resolved enum, not a string set.** `"me"` cannot be resolved in
`lb_insights` (team membership lives in `lb_authz`/`lb_assets`, which the crate is deliberately
agnostic of), so the host resolves the wire string and passes a typed filter — the same seam
`tag_allow` already uses. It is an enum rather than a `HashSet` because *"unassigned" is not a
subject*: folding it in as a sentinel `""` is exactly how an empty-string assignee silently becomes a
legal owner.

**2. The membership refusal is one message for three cases.** A nonexistent subject, a non-member,
and a **real member of another workspace** all produce the identical `BadInput`. Both reads are
workspace-namespaced, so the third case is structurally indistinguishable from the first — assign
cannot become a cross-tenant existence oracle. The test asserts the three errors are equal rather
than merely that each fails, because "each fails" passes even when the messages differ.

**3. An invalid assignee fails the whole bulk call; a missing insight is a per-item result.** The
assignee is the same subject for all 100 ids, so validating per item would be 100 identical reads for
one answer — and an unknown assignee is a caller error about the *request*, not an outcome for each
row. Per-item results carry only genuinely per-item failures.

**4. `validate_comment` rejects empty/whitespace text.** Not named in the scope, but an empty note is
indistinguishable from a mis-click, and the thread's whole value is that every row says something.

**5. `insight.get` composes the thread in the MCP bridge, not in `insight_get`.** `insight_get`
returns `Option<Insight>` and is called directly by `rules_test`; widening its signature would have
churned a neighbouring suite for no gain. The bridge merges `comments` into the response JSON, so the
wire shape is additive and every existing field stays where it was.

**6. Comment rows are `write`-based, unlike occurrence rows.** The ring uses `capped_insert` (flat
rows, store-injected `seq`) *because* it evicts. Comments don't, so they use plain `write` with a
`{insight_id}:{seq}` id — which also means the delete cascade filters `data.insight_id`, not
`insight_id`. Getting that envelope wrong is silent (the delete matches nothing), so it is called out
in `delete.rs`.

## Testing

`rust/crates/host/tests/insight_triage_test.rs` — **17 cases**, real booted `Node`, real store, real
bus, real caps, the real `call_tool` MCP bridge. No mocks (CLAUDE §9): every insight is raised through
the verb under test, and memberships/teams are seeded as real rows through the real `lb_authz` /
`lb_assets` writers.

```
cargo test -p lb-host --test insight_triage_test  → 17 passed; 0 failed
                      --test insight_analysis_test → 16 passed   (slice 2, unbroken)
                      --test insight_tag_echo_test → 11 passed   (slice 1, unbroken)
                      --test insight_evidence_test → 10 passed
                      --test insights_test         → 22 passed
cargo test -p lb-insights                          → 3 + 10 ok
cargo build --workspace → green;  cargo fmt --all --check → clean
packages/insights: pnpm typecheck → clean;  pnpm test → 12 passed (3 files)
```

Mandatory categories, both present:

- **Capability deny** — including the one this scope exists to create: a **producer** token holding
  `mcp:insight.raise:call` and nothing else is denied *both* writes, and the record is unchanged
  after. Asserted on the property only the outer gate has: a real id and a fictional id produce
  **identical** errors.
- **Workspace isolation** — ws-B cannot assign, comment on, read the thread of, or list a ws-A
  insight, with identical error shapes for "another workspace's real id" and "an id that exists
  nowhere".

Scope cases covered: dedup preservation **including the re-open arm** (§1), `raise` cannot write
triage state (§2), assign/re-assign/un-assign + idempotence + `team:` + membership validation +
missing insight (§3), un-forgeable author (§4), the list filter incl. `"me"` resolving **through
teams** and composition with `status` + keyset paging (§5), the get/list boundary (§6), both comment
caps refusing (§7), and comments dying with their insight (§8). Bulk assign's per-item results and
reported cap are covered too (resolved decision 6).

### Revert-checks performed (both the ones the scope names)

| Revert | Result |
|---|---|
| Made the re-open arm clear `assigned_to` (`prior.assigned_to = None`) | `the_re_open_arm_clears_the_lifecycle_but_not_the_human_facts` went red, **and nothing else did** — the test is pinned to exactly the decision it guards. Restored. |
| Swapped the count cap for ring eviction (delete oldest, then append) | `the_count_cap_refuses_the_write_it_does_not_evict_the_oldest` went red, nothing else. Restored. |

Both are the reverts the scope predicted a future contributor would make while "cleaning up" — the
first beside `status_by`, the second to match the occurrence ring it sits next to.

### One test I had to correct (worth recording)

My first workspace-isolation assertion required the two errors to be **byte-identical** and failed:
both are `no such insight: <id>`, differing only in echoing back the id the caller itself passed. That
echo leaks nothing (ws-B already knows the id it typed), so the honest property is that the error
*template* matches. The test now compares with the caller's own id redacted. Worth naming because the
lazy fix — deleting the assertion — would have removed the isolation check entirely.

### Pre-existing failures, verified unrelated

- `lb-cli`'s `sign_test` and `lb-role-gateway`'s `publish_install_test` don't **compile** here (both
  read a wasm artifact under `extensions/`, a tree absent from this working copy) — structurally
  unrunnable, not failing.
- `lb-host`'s `rules_test::registered_datasource_is_in_the_rule_allowlist` fails (**21/22 pass**) —
  red before this slice, owned by the rule-allowlist path, untouched here. Confirmed by running the
  suite rather than by stashing (concurrent sessions hold staged work in this tree).
- `federation::direct_path_pg_test` is gated on a real Postgres and runs 0 tests here.

## Live verification (not just the suite)

Against a real booted node from **this** working tree — `cargo run -p lb-role-gateway --features
test-harness --bin test_gateway`, `LB_DEV_LOGIN=1 PORT=8137` — over the real `POST /mcp/call` wire
with real minted tokens. (Note the envelope is `{tool, args}`, not `{tool, input}`.)

```
token caps → mcp:insight.assign:call + mcp:insight.comment:call present  ← the caps reach a real mint
raise (nothing human set)          → {"created":true}
list {assigned_to:"none"}          → [the Chullora row, UNASSIGNED]      ← the triage queue
assign {assignee:"user:priya"}     → refused: "assignee is not a member of this workspace…"
assign {assignee:"user:does-not-exist-anywhere"}
                                   → BYTE-IDENTICAL refusal              ← no existence oracle
teams.create team:mechanical; assign {assignee:"team:mechanical"}
                                   → {"assigned_to":"team:mechanical"}   ← team: legal from v1
list                               → row carries assigned_to, NO comments key
comment {text:…, author:"user:someone-else"}
                                   → {"seq":1};  get → author "user:ada" ← host-stamped, un-forgeable
assign {assignee:"user:ada"} ×2    → idempotent
list {assigned_to:"me"}            → the row                             ← "me" resolved host-side

resolve → get: status resolved, status_by user:ada, assigned_to user:ada
re-raise same dedup_key
  → status open | status_by CLEARED | status_ts CLEARED
    assigned_to user:ada | comments 1 | count 2
    ^^^ THE load-bearing rule, on the real wire: lifecycle clears, human facts survive

assign {ids:[real, real, "not-real"]}
  → {"results":[{ok:true},{ok:true},{"id":"not-real","ok":false,"error":"no such insight: not-real"}]}
assign 101 ids  → "101 ids exceeds the 100-id bulk cap — nothing was assigned…"   ← reported, not truncated
comment 5000 bytes → refused; get → thread still 1 comment, no truncated row landed
comment "   "      → "comment text is empty — a note must say something"
comment on "nope"  → "no such insight: nope"

other-ws token → assign on acme's real id  → "no such insight: <id>"
               → assign on a fictional id  → same shape;  list → 0 items
```

Every behaviour the scope specifies, confirmed end to end — including the two `cargo test` alone is
weakest evidence for: the refusals leaving no partial write, and the re-open arm preserving human
state on a real store rather than an in-test one.

**Honest gaps in the live run.** Two things I could not drive from the wire and did not pretend to:

1. **`membership.add` was denied** to the dev-login token (it lacks `members.manage`), so the live
   `user:` assign was exercised against `user:ada` — a genuine member via the bootstrap — rather than
   a second member I added. The `user:` non-member refusal and the `team:` acceptance were both
   verified live; a second seeded member adds nothing the suite doesn't already cover with real rows.
2. **The roster re-render and the drawer are downstream.** lb is a library and the UI shell is
   out-of-tree (`MIGRATION.md`), so scope §9's "confirm the roster re-renders off the SSE stream and
   the drawer shows the thread" cannot be checked in this repo. What is proven here is that the
   events publish on the existing subject and the fields arrive at the consumer boundary with the
   right shape. Rendering the owner column, showing an unresolvable assignee as **"unknown
   (removed)"** (resolved decision 3), and **not** implying a notification was sent (decision 1) are
   `rubix-ai` changes — and all three are places the decision can quietly evaporate.

## The inherited open question, resolved

Slice 1 left this open ([`insight-tag-echo-scope.md`](../../scope/insights/insight-tag-echo-scope.md)
"Open questions after building" #3): tag edge identity is `(entity, tag, source)`, so a `Producer`
`classification=plumbing` and a `Human` `classification=mechanical` can coexist, and the flat echo
map keeps whichever `tags.of` returns last — non-deterministically.

**Resolved: precedence by source, `Human` > `Producer`, and it belongs to the echo — not here.**

The rule is that a human correction wins over a machine assertion for the same key. It is the only
rule that makes the feature humans are about to use — re-classifying a finding they've been assigned
— actually stick: under last-write-wins, an operator re-tags a finding, the rule fires overnight, and
their correction silently reverts. That is the same class of trust bug as evicting a comment, and
this slice is what makes it *reachable*, because triage is where a human first disagrees with the
machine in the UI.

**Scoped out of slice 3 deliberately, and filed rather than fixed.** Implementing it means changing
`materialize_facets` in `host/src/insight/raise.rs` to fold by source precedence instead of
collecting, which changes what every existing echoed row means — a shipped-semantics change to the
raise hot path, on the slice-1 surface, with its own deny/regression surface. That does not belong in
a slice whose load-bearing property is "the raise path does not touch the triage plane". Filed as
[`insight-tag-precedence-scope.md`](../../scope/insights/insight-tag-precedence-scope.md), with this
session's argument for why the rule must be source precedence.

Until it lands, **a multi-source tag key is non-deterministic in the echo** — so a UI must not offer
re-classification of a producer-set key as if it will stick. Named in the public doc.

## Filed, not fixed

- [`insight-tag-precedence-scope.md`](../../scope/insights/insight-tag-precedence-scope.md) — the
  above.
- **Assignee notification** (resolved decisions 1 + 5): assigning does not page the assignee in v1.
  The ladder is subject-matched, not assignee-matched, so this needs a new match arm in `match_subs`
  + the subscription grammar. This is the scope's own named first follow-up, and it is now more
  pressing than when it was written: v1 ships the ability to *give someone work* with no way for them
  to hear about it, which is the same trust shape as the umbrella's "0 subscribers" flag.
- **The retention follow-up is now load-bearing for a second reason.** Comments are purged only with
  their parent, so "delete an insight" must stay the only path that removes them; a retention sweep
  that deletes parents without cascading would strand human notes permanently.

## Still open from earlier slices

- The tag-echo **backfill job** (slice 1) — a resolved insight that never fires again keeps a blank
  echo, and now also a blank owner column context, until it re-raises.
- **`tags.*` has no dispatcher entry** (slice 1) — no wire door, so the out-of-band tag edit that the
  precedence rule above is *about* is not yet reachable over MCP.

## Related

- Scope: [`insight-triage-scope.md`](../../scope/insights/insight-triage-scope.md) (status → shipped;
  "Open questions after building" added)
- Slice 1: [`insight-tag-echo-session.md`](insight-tag-echo-session.md) ·
  Slice 2: [`insight-analysis-session.md`](insight-analysis-session.md)
- Skill doc: `docs/skills/insights/SKILL.md` — the triage walkthrough (assign → comment → filter →
  bulk), grounded in the live run above
- Testing runbook: `docs/testing/insights/README.md`
- Public: `doc-site/content/public/insights/insights.md`
