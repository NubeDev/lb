# `filterByValue` — an "is one of" (array membership) matcher

## Ask

The **Filter data by values** transform needs a matcher that tests MEMBERSHIP of a field value in a
LIST — include (or exclude) a row when `meter` is one of `["MSB-2 Main", "MSB-01"]`. The list is meant
to come from an array-typed dashboard variable, i.e. the selection a table's multi-select row click
accumulates (`RowClickConfig.mode: "multi"`, shipped downstream in rubix-ai).

Filtering executes **entirely server-side** — the client posts `panel.transformations` to `viz.query` and
runs no matcher of its own — so the matcher itself is an **lb** change. This is the lb half; the editor
affordance is the rubix-ai half (`NubeIO/rubix-ai`, branch
`157-add-includes-in-array-matcher-to-filter-data-by-values-transform-for-use-with-array-dashboard-variables`).

## What shipped

`rust/crates/viz/src/transforms/filter_by_value.rs` — two new `ValueMatcherID`s:

| id | operand | test |
|---|---|---|
| `oneOf` | `options.value`: an array (or a scalar) | the cell value equals ANY list member |
| `notOneOf` | same | the per-row complement of `oneOf` |

Grafana has no equivalent id, so these are **lb's own** — the first deliberate departure from the
verbatim-Grafana matcher set this transformer otherwise mirrors. Noted in the module header so a future
Phase-4 Grafana import doesn't mistake them for parity ids it can round-trip.

`equal`'s comparison body was extracted to `equal_to(value, target)` and `oneOf` tests each list member
with it. This is the point of the refactor: membership is *equality, N times*, and one shared helper is
what keeps it from drifting into a second, subtly different equality semantic.

## The three judgement calls (and why)

**A scalar right-hand side is a one-element list, not a non-match.** This is not defensive slop. The
client carries a multi-value selection as repeated `?var-<name>=` params, and its URL round-trip
(`ui/src/features/routing/search.ts` in rubix-ai) **collapses a single-element array back to a plain
string**. So a one-row selection genuinely arrives here as `"MSB-01"`, not `["MSB-01"]`. Rejecting a
scalar would make the filter work at two selected rows and silently empty the panel at exactly one —
precisely the class of quiet wrongness this transformer exists to refuse. Pinned by
`one_of_treats_a_scalar_operand_as_a_one_element_list`.

**An empty list matches nothing, not everything.** "Nothing is selected" must not read as "no filter", or
clearing a selection would silently *widen* the result instead of narrowing it. An author who means "all
rows" removes the condition. Pinned by `one_of_empty_list_matches_nothing`.

**`notOneOf` with an absent operand keeps every row**, because it is `!one_of` and an absent operand is
an honest `false`. That mirrors the shipped `notEqual` exactly; diverging here would have made two
negated matchers behave differently for the same half-authored config.

## Known gap (named, not silent)

A dashboard variable's values are **strings**, so `oneOf` over a **numeric** column with a variable
operand compares `5` against `"5"` and matches nothing. `equal` has had this same gap since Phase 3 (it
is documented downstream in rubix-ai's `builder/numericOperands.ts`, which coerces the *ordered* matchers
on the wire precisely because they are numeric-only and therefore safe to coerce).

The fix is one deliberate change to `equal_to` — try a numeric parse when one side is a numeric string —
covering `equal`, `notEqual`, `oneOf`, `notOneOf` at once. It is **not** done here because it also makes a
text column holding `"007"` match a target of `7`: one silent wrongness traded for another, and worth its
own decision rather than riding along with a new matcher. What this session did guarantee is that
`oneOf` does not fork its own coercion in the meantime.

String columns — the actual ask (`meter` names) — are unaffected.

## Tests

`cargo test -p lb-viz` → **86 passed** (79 before; +7 here), `cargo fmt -p lb-viz --check` clean.

New units in `filter_by_value.rs`:

- `one_of_keeps_rows_whose_value_is_in_the_list`
- `one_of_treats_a_scalar_operand_as_a_one_element_list` — the URL-collapse case above
- `one_of_compares_numerically_on_a_numeric_column` — a numeric list vs a numeric column, via the shared
  `equal_to`
- `not_one_of_is_the_per_row_complement`
- `one_of_empty_list_matches_nothing`
- `one_of_unresolved_variable_literal_matches_nothing` — an unknown `$var` is left literal by the client
  (the vars library's shared-link behavior), so it must reach here as a string no row equals
- `one_of_without_an_operand_never_matches`

No capability or workspace surface changed: `lb-viz` is a pure crate with no store, bus, or I/O, and
`viz.query`'s existing `mcp:viz.query:call` gate and workspace wall are untouched. The mandatory
deny/isolation coverage for that verb is unchanged in `rust/crates/host/tests/viz_query_test.rs`.

## Downstream

rubix-ai must bump its `lb-node` pin past this change to expose the matcher — it is currently
`node-v0.20.0` (`Cargo.toml`). Until then the rubix-ai editor can author a `oneOf` condition that the
embedded node does not know, and an unknown `ValueMatcherID` never matches (the module's stated honesty
rule) — so the failure is an empty panel, not a crash. Sequence per `WORKFLOW-LB.md` §4: merge here → tag
→ bump the pin there.
