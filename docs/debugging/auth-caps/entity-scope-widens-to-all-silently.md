# Entity-scoped grants silently widened to `Scope::All` (two paths)

- **Date:** 2026-07-11
- **Area:** auth-caps (entity-scoped grants)
- **Status:** fixed
- **Found by:** peer review of the entity-scoped-grants slice (branch `updates-to-core`)

## Symptom

Two independent code paths turned a *narrowing* selector into full-table reach — the exact
privilege-widening the scope doc forbids ("only ever subtractive"):

1. **Malformed selector → `All`.** `host/src/authz/tool.rs::scope_arg` parsed the optional
   `scope` argument with `serde_json::from_value(...).unwrap_or(Scope::All)`. A caller who sent
   a scoped `grants.assign` with a typo (`"kind": "idz"`, missing `ids`, a string instead of an
   object …) silently wrote an **unscoped** grant: the admin asked for `child:[leo]`, the store
   got *every row of every table*. Fail-open at the security chokepoint.
2. **Cross-table union → `All`.** `authz/src/scope.rs::Scope::union` handled two `Ids`
   selectors for *different* tables with `if t1 != t2 { return Scope::All; }` — mislabelled
   "conservative" in the comment. A principal granted `child:[leo]` and `site:[north]` under the
   same cap resolved to `All`: reach to every child, every site, and every other table. The
   test `union_ids_different_table_widens_to_all` enshrined the wrong behaviour.

## Root cause

Both are the same class of defect: an "impossible/edge" input at a narrowing seam defaulted to
the **widest** value instead of failing or accumulating. In an authorization union, the safe
direction is *less* reach, never more — "widen to safe" was exactly backwards.

## Fix

1. `scope_arg` now returns `Result<Scope, ToolError>`: absent/null `scope` → `All` (additive
   default, unchanged); a **present-but-malformed** selector → `ToolError::BadInput`, no grant
   written (`rust/crates/host/src/authz/tool.rs`).
2. Additive `Scope::Tables { tables: BTreeMap<String, BTreeSet<String>> }` variant
   (`rust/crates/authz/src/scope.rs`): the union of `Ids` for different tables accumulates
   per-table id-sets instead of collapsing to `All`. `contains`/`filter_for`/`key` handle it
   per-table. Single-table unions still collapse back to `Ids`, so existing `grant_id` keys and
   stored records are byte-stable (zero migration holds); `Tables` normally only arises inside
   resolution.
3. Rode along: the gateway REST body (`role/gateway/src/routes/admin_grants.rs`) previously
   hardcoded `Scope::All` on assign/revoke — it now passes an optional additive `scope` field
   (absent = `All`; malformed = 422, consistent with fix 1).

## Regression tests

- `crates/host/tests/authz_scoped_test.rs::malformed_scope_selector_is_bad_input_and_writes_no_grant`
  (five malformed shapes → `BadInput`, `grants.list` stays empty)
- `crates/host/tests/authz_scoped_test.rs::multi_table_scoped_grants_reach_only_their_rows_not_everything`
- `crates/authz/src/scope.rs::tests::union_ids_different_table_accumulates_without_widening`
  (replaces `union_ids_different_table_widens_to_all`), `union_multi_table_collapses_back_to_ids_when_one_table`,
  `tables_key_is_deterministic`
- `crates/authz/tests/scoped_grants_test.rs::union_across_tables_reaches_only_granted_rows_not_everything`
- `role/gateway/tests/entity_scoped_grants_routes_test.rs::malformed_scope_in_body_is_rejected_not_widened`
  (+ scope passthrough, cap-deny, ws-isolation)

## Lesson

At a capability-narrowing seam, every unexpected input must resolve toward **less** authority:
parse failures are hard errors, and a union of selectors is the sum of what was granted — an
`All` that nobody granted is an escalation, not a fallback.
