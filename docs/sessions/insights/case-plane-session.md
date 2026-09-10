# Session — the Case plane (lb half)

- **Status:** in-progress
- **Started:** 2026-09-10
- **Branch:** `feat/case-plane` (worktree `~/code/rust/lb-cases-wt`)
- **Scope:** `docs/scope/insights/case-plane-scope.md` (this repo) — derived from the product scope
  in `NubeIO/rubix-ai → docs/scope/insights/case-plane-scope.md`.
- **Downstream:** `NubeIO/rubix-ai` `feat/case-plane` consumes this through a local `[patch]` until
  it is merged and a `node-v*` tag is cut.

## The ask

Build the upstream half of the Case plane: the `lb-cases` crate, the `case.*` verb surface, the
case-group / hold-down / sla-clock reactors, the `service_policy` + business-hours deadline
arithmetic, the contractor `case_request` round trip with its `GET /r/{token}` token principal, and
the changes the insight raise hot path takes (the `tags.*` dispatcher door, the `Human > Producer`
fold, `evidence.subjects[]`, the data-quality caveat stamp, `category` validation, `month_hist` +
`pattern`, the `case_id` echo).

## Baseline

lb master is red on some jobs and some tests race under parallelism, so a failure that is not ours
is only provable against a baseline captured on the untouched branch. `cargo test -p lb-insights -p
lb-host --no-fail-fast` was run on `origin/master` (`f8633b6d`) at the start of the session; the
output is compared against every later run before any failure is treated as a regression.

## Waves

| wave | slice | agent | state |
|---|---|---|---|
| 1 | raise-path changes: tags door, `Human > Producer` fold, backfill, `evidence.subjects`, caveats, `category` vocab, `month_hist`/`pattern`, `case_id` echo | insights-raise | in-progress |
| 1 | `lb-cases` crate, `case.*` verbs, case-group + hold-down reactors, triage migration | cases-core | in-progress |
| 1 | `service_policy`, the calendar type, the pure deadline arithmetic, `policy.sla.*` | sla-policy | in-progress |
| 2 | the sla-clock reactor | sla-clock | pending |
| 2 | `party`, `case_request`, the outbox link, the nudges, the gateway token principal | requests | pending |
| 2 | `rule.scorecard` | scorecard | pending |

## Decisions taken

Recorded as they are made — each with the alternative rejected and why. See also the scope doc's
*Resolved decisions*, which this list extends with anything decided during the build.

1. **The tag door keeps the shipped verb names** (`tags.add`/`tags.remove`/`tags.of`/`tags.find`),
   dispatched by EXACT name rather than a `tags.` prefix. The product scope calls the write
   `tags.set`; `tags.add` has shipped as an upsert on `(entity, tag, source)` since the tags scope,
   and minting a second name for one write forks the surface for a cosmetic reason. Dispatching by
   exact name (not a prefix) follows the `ext.list` precedent: the host does not reserve a whole
   namespace against a hypothetical extension called `tags`. *Rejected: `tags.set` as an alias* —
   two names for one write is how a catalog stops being readable.
2. **`mcp:tags.remove:call` joins the AUTHOR bundle with the door.** It exists in NO role bundle
   today, so a dispatcher entry alone would leave `tags.remove` `Denied` for every caller including
   admins — the shipped-but-unusable shape `tool_gate.rs` already documents four times. `tags.of`
   is aliased onto `tags.find` instead of being granted separately (reading one entity's tags is
   `tags.find` narrowed to one entity, not a second privilege).
3. **Grouping runs inline at the end of `insight_raise`, with a reconcile loop as the backstop.**
   The invariant is "every open insight is in exactly one open case"; a scan-only reactor makes that
   *eventually* true, so the queue under-reports for a tick and a UI that raises-then-lists sees a
   case-less row. Inline is where the tag echo and the subscription matcher already run. The loop
   (`spawn_case_reactors`) is the restart-safe backstop and doubles as the triage backfill.
   *Rejected: a scan-only reactor* (eventual consistency on a completeness invariant) and *a
   synchronous verb the UI calls* (two writers for one fact).

## Test evidence

Pasted below as each wave lands.

## What is not done

Filled in at the end, explicitly.
