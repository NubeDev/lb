# Session — insights: the tag echo (the record carries its own facets)

Status: **shipped** (slice 1 of GitHub issue #119 — the tag echo only).
Scope: [`../../scope/insights/insight-tag-echo-scope.md`](../../scope/insights/insight-tag-echo-scope.md).
Branch: `updates-for-reports` (no commits made — the human owns git).
Date: 2026-07-30.

## The ask, restated

Tags are the insight's dimension plane and they are persisted to the tag graph, but they are not
**on the record** — so a roster gets rows back and then needs an N+1 `tags.find` per row (plus a tag
capability a read-only viewer shouldn't need) before it can render a single dimension column. Echo
the materialized facets onto the `Insight` record, on **both** `insight.get` and `insight.list`.

Deferred deliberately (scope §Backfill): the backfill job for records that never fire again. See
"Next step" below.

## What shipped

| Layer | Change |
|---|---|
| Record | `Insight.tags: BTreeMap<String,String>` — `#[serde(default)]` + `skip_serializing_if` empty, so a pre-field row still decodes (`rust/crates/insights/src/insight.rs`) |
| Verb | `set_tags_echo` + `validate_tags_echo_size` + `MAX_TAG_ECHO_BYTES` (`rust/crates/insights/src/tags_echo.rs`, new file) |
| Raise path | materialize from the graph **unconditionally**, persist the echo, reuse the record it returns for the matcher's `origin_ref` (`rust/crates/host/src/insight/raise.rs`) |
| Read path | none — `get`/`list` return the typed record and the gateway routes pass it through; `list`'s strip loop touches `evidence` only |
| Filter path | **unchanged on purpose** — `insight.list { tags }` still resolves through `lb_tags::find` against the graph |
| Frontend | `Insight.tags?: Record<string,string>` on the canonical TS shape (`packages/insights/src/types.ts`) |

No new verb, no new capability, no new table — as the scope requires. The echo is host-computed and
not caller-writable (the `producer` host-stamp precedent).

## Decisions made while building

**1. The materialize read is raw (`lb_tags::of`), not the `mcp:tags.of:call`-gated host verb.**
The old code called `crate::tags::tags_of(.., principal, ..)`, which authorizes `tags.of`. That
would have made the echo *silently* fall back to `RaiseInput.tags` for any producer lacking a tag
capability — the union-vs-declaration bug the whole scope exists to prevent, in its least visible
form. Live evidence that this is the common case, not a corner: the real member token minted by the
running product (`/auth/login` as `user:test`) carries `mcp:tags.add:call` and `mcp:tags.find:call`
and **not** `mcp:tags.of:call`. Reading raw follows the existing precedent one file over —
`insight_list` resolves its facet filter through the raw `lb_tags::find` because
`mcp:insight.list:call` already authorized the workspace read.

**2. Tag *application* at raise is also raw now (`lb_tags::add`, still under the per-workspace
tag-node cap).** This was a real bug found here, not a refactor — see the debug entry below.

**3. The size guard's contract is a loud skip, not a rejected raise.** `validate_evidence_size`
rejects the whole raise because it runs *before* any write. The echo is computed *after* the durable
record landed and projects tags that are already in the graph, so failing the raise would be wrong
and truncating would be silent. Over the cap: `tracing::warn!` + the previous echo left in place
(visibly stale), never a partial map.

**4. `set_tags_echo` skips the write when the echo already matches.** The steady-state re-raise of a
flapping producer costs one graph read and no record write. Net cost of this scope on the raise hot
path: **one indexed `tags.of`**, and one conditional record write — the extra record *read* is free
because `set_tags_echo` returns the post-write record and the matcher's `origin_ref` now comes from
that instead of the second `lb_insights::get` it used to do.

## Two things found that this slice did not fix

- **`tags.*` is not reachable over `/mcp/call`.** `call_tags_tool` exists, is capability-gated, and
  has **no caller in the dispatcher** — `is_host_native` has no `tags.` entry, so a live
  `{"tool":"tags.add"}` returns `no such tool` (verified against the running node). Every tags
  caller today is in-process (nav, the raise path). This does not affect the echo (the graph is
  written by the raise path and read by the same), but it means the scope's "an admin re-classifies
  through the existing `tags.*` verb" story has no wire door yet. Left alone: wiring a verb family
  into the dispatcher is its own scope + per-verb deny tests, not a tag-echo change. Recorded in the
  scope's open questions.
- **Same-key, multi-source tag edges are ambiguous in a flat echo.** Edge identity is
  `(entity, tag, source)`, so a `Producer` edge `classification=plumbing` and a `Human` edge
  `classification=mechanical` coexist, and a flat `{k: v}` map keeps whichever `tags.of` returns
  last. The scope says the echo is not provenance (read `tags.of` for that), so this is in-contract
  — but "last edge wins" is undefined, not chosen. The self-heal test therefore asserts on an
  unambiguous *added* key rather than a contested one. Worth a rule (newest edge wins? producer
  loses to human?) before the triage slice lets humans re-classify.

## Tests

`rust/crates/host/tests/insight_tag_echo_test.rs` (new, 11 cases, real booted `Node`: real store
`mem://`, real tag graph, real caps, the real `call_tool` bridge) + 2 unit tests on the size guard in
`rust/crates/insights/src/tags_echo.rs` + 3 Vitest cases in `packages/insights/src/tagEcho.test.tsx`
(real `memoryClient` transport + the real `useInsights` hook).

Mandatory categories: capability-deny (three cases, incl. the narrowing this scope buys) and
workspace-isolation (ws-A and ws-B carrying the **same tag key and value**).

### Revert-checks — the suite is only worth what it catches

Every one was run by breaking the shipped code on purpose and confirming **red**, then restoring.

| Break | Expected | Result |
|---|---|---|
| Echo written from `RaiseInput.tags` instead of the graph | union test red | **red** — `union`, `self_heals`, `not_caller_writable` (3 tests) |
| `insight.list` facet filter "simplified" to scan the echo | filter test red | **red** — `list_filtering_reads_the_graph_not_the_stale_echo` only |
| Restore `if !subs.is_empty()` around materialization | zero-subs test red | **red** — 8 tests, incl. `echo_is_written_with_zero_subscriptions` |
| Remove the `insight.get` service gate only | deny test red | **still green** — the dispatcher gate catches it |
| Remove the dispatcher gate for `insight.get` only | deny test red | **still green** — the service gate catches it |
| Remove **both** gates | deny test red | **red** — the record (facets and all) leaks |

The last three are the point: the deny is enforced at two independent layers, so a single-layer
revert proves nothing — exactly the "deny test passes on reverted code" trap. The test is
non-vacuous only against both.

### Green output

```
$ cargo test -p lb-host --test insight_tag_echo_test --no-fail-fast
running 11 tests
test a_denied_list_leaks_no_facet_data ... ok
test echo_is_written_with_zero_subscriptions_in_the_workspace ... ok
test the_echo_is_not_caller_writable ... ok
test the_echo_never_crosses_the_workspace_wall ... ok
test echo_lands_on_raise_and_appears_in_both_get_and_list ... ok
test echo_is_the_union_across_raises_not_this_raises_declaration ... ok
test echo_self_heals_on_the_next_raise_after_an_out_of_band_tag ... ok
test a_denied_get_cannot_distinguish_a_real_id_from_a_fictional_one ... ok
test a_lister_with_no_tag_caps_at_all_still_receives_the_echo ... ok
test list_filtering_reads_the_graph_not_the_stale_echo ... ok
test an_oversize_facet_set_skips_the_echo_instead_of_bloating_the_record ... ok
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.62s

$ cargo test -p lb-insights --lib
test tags_echo::tests::a_dimension_sized_facet_map_passes_the_guard ... ok
test tags_echo::tests::an_absurd_facet_map_is_rejected_whole_and_says_why ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s

$ cargo test -p lb-host --test insights_test --test insight_evidence_test --test tags_test --no-fail-fast
test result: ok. 10 passed; 0 failed  (insight_evidence_test)
test result: ok. 22 passed; 0 failed  (insights_test)
test result: ok.  3 passed; 0 failed  (tags_test)

$ cargo test -p lb-rules --test insight_test          → ok. 12 passed; 0 failed
$ cargo test -p lb-role-gateway --test insight_routes_test → ok.  6 passed; 0 failed

$ cd packages/insights && pnpm test
 ✓ src/model.test.ts (5 tests)
 ✓ src/InsightsWidget.test.tsx (4 tests)
 ✓ src/tagEcho.test.tsx (3 tests)
 Test Files  3 passed (3)   Tests  12 passed (12)
$ pnpm typecheck   → clean
$ cargo fmt        → clean;  cargo clippy -p lb-insights -p lb-host --tests → no new warnings
```

## Live verification (not just the suite)

Against the **running product** — `rubix-ai` on `127.0.0.1:8099`, whose `.cargo/config.toml`
`[patch]` points `lb-node` at this working tree; rebuilt and restarted (`make kill && make dev`) so
the binary actually carried the change.

```
# two raises of the same dedup_key with DISJOINT tag declarations
insight.raise {tags:{building:chullora-dc, asset_type:water-meter}} → count 1, created
insight.raise {tags:{priority:medium}}                             → count 2, same id

# ONE insight.list call — the whole roster page:
"tags": { "asset_type": "water-meter", "building": "chullora-dc", "priority": "medium" }
```

The union across both raises, on the **list** row, from a single call, under a token holding no
`tags.of` capability. The REST route (`GET /insights`) returns the same. Facet filtering still
resolves through the graph live (`insight.list {tags:{building:"chullora-dc"}}` → the row).

Two honest caveats on the "network panel" half of the check:

- The browser could not be driven from this session (the Chrome extension is not connected), so the
  N+1 claim is evidenced from the payload above plus the code path: the rubix-ai insights feature
  (`ui/src/features/insights/*`, `ui/src/lib/insights/*`) calls `insight.list` / `insight.get` /
  ack / resolve / occurrences and **no `tags.*` verb at all** — and could not, since `tags.*` is not
  dispatchable (above). There is no fan-out to remove because the roster never had the data; it now
  arrives with the list.
- **The dimension *columns* themselves are a downstream change.** rubix-ai's roster
  (`ui/src/features/insights/InsightsList.tsx`) has a fixed 5-column header and reads only
  `dedup_key`/`severity`/`status`/`last_ts`; its own copy of the `Insight` type
  (`ui/packages/insights/src/types.ts`) needs the `tags?` field too, and `insightsExport.ts`
  (`CSV_COLUMNS`) + `insightsSearch.ts` are hardcoded field lists that will silently omit tags. All
  out-of-tree, none touched here.

## Docs / follow-ups

- Public: `doc-site/content/public/insights/insights.md` (the placeholder is now a real page for
  this field).
- Skill: `docs/skills/insights/SKILL.md` — the verb table + walkthrough show `tags` coming back on
  `list`, and state that the graph is the write path.
- Scope: "Resolved decisions" unchanged (all six held); open questions refreshed with the two
  findings above.
- **Next step (deferred by the scope, sequenced after this):** the backfill **job** — walk the
  workspace's insights, `tags.of` → `set_tags_echo` (the verb is already idempotent, resumable and
  re-runnable, and takes the map directly, so the job is a table walk plus a call). Shape it on
  `host/src/insight/heal_ts.rs`, but **not** on a boot driver — the scope is explicit that a large
  table makes this a deliberate run, not a boot walk. Until it lands, a resolved insight that never
  fires again keeps a blank echo; the roster must render that as "no dimensions", not "no data".
- The other two thirds of the "more fields on an insight" ask (`insight-analysis-scope.md`,
  `insight-triage-scope.md`) are unblocked for their dimension columns and were not touched.

## Debugging

- [`../../debugging/insights/producer-tags-never-reached-the-graph.md`](../../debugging/insights/producer-tags-never-reached-the-graph.md)
  — declared tags silently swallowed unless the producer also held `mcp:tags.add:call`.
