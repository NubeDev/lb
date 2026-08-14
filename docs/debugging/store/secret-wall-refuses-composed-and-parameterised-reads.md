# The secret-plane wall refused every composed read and the whole `store-read` node

- **Area:** store / flows / rules
- **Symptom:** `store.query` refused two entirely ordinary shapes with
  `rejected: a table computed at run time (Idiom|Function) cannot be proven not to be a secret table,
  so the query is refused — name the table literally`:
  1. any **composed** read — `SELECT * FROM (SELECT … FROM t WHERE …) …` — i.e. every rules grid
     built by `history(...).filter(...)`;
  2. every **`store-read` / `store-write` / `store-delete` flow node**, which parameterises its table
     as `type::table($tb)` on purpose.
- **Status:** resolved
- **Date:** 2026-08-14 (shipped broken in `9fd99175`, 2026-08-04)

## What was observed

Surfaced as four "pre-existing, unrelated" failures in `flows_platform_nodes_test` and two in
`rules_test` — the kind of noise a session writes off as someone else's problem. They were the
product:

```
[f_r] err step: {"id":"n","outcome":"err",
  "error":"bad input: a table computed at run time (Function) cannot be proven not to be a
           secret table, so the query is refused — name the table literally"}
```

and, from a rule whose body is the documented one-liner
`history("series", "cooler.temp", "24h").filter("value > 5.0")`:

```
Eval("Runtime error: rejected: a table computed at run time (Idiom) cannot be proven not to be a
      secret table, so the query is refused — name the table literally")
```

## Root cause

Both come from one function, `store_query/secret_wall.rs::walk`, and they are two different edges of
the same rule: *a dynamic construct in a **table position** is refused.*

**(1) The table position never closed.** It is entered by key (`what`, `tb`, …) and was inherited for
the *whole* subtree, deliberately, so that a `type::table($t)` nested inside `FROM (…)` is still
caught. But a subquery is not an expression — `FROM (SELECT data.name AS name FROM site ORDER BY
data.name)` puts an entire statement under `what`, and every field reference in its projection,
`WHERE` and `ORDER BY` serialises as an `Idiom`. Each one read as "a table computed at run time".
Since `store_query_run` *itself* wraps every validated `SELECT` in `SELECT * FROM ({sql}) LIMIT …`,
this is not an exotic shape — it is the shape the surface is built from.

**(2) "Cannot be proven" was not true.** `SELECT * FROM type::table($tb)` names no table in the AST,
so the wall refused it outright. But `store.query` takes `{sql, vars}` — the binding that chooses the
table arrives *in the same request*. The information needed for the proof was one function argument
away and simply wasn't passed in. Meanwhile the flow store-CRUD nodes parameterise their table
precisely so that no user text is ever spliced into SQL (their own module doc says so), which made
the safest possible caller the one the wall rejected — a direct collision between two correct
instincts.

## Fix

Both in `secret_wall.rs`, plus threading the bindings to the gate:

- **The table position ends at a nested statement** (`NESTED_STATEMENT = "Select"`). A subquery's own
  `FROM` re-opens the position by its own `what` key, so nothing is lost: `FROM (SELECT * FROM
  secret)`, a correlated `WHERE … IN (SELECT … FROM secret)`, and a doubly-nested variant are all
  still refused. Only the *statement's own* projection/filter/ordering stops being treated as a table
  slot — which it never was.
- **A dynamic table position is resolved against the request's `vars` before being judged.**
  `resolved_table` handles exactly two shapes — `FROM $tb` and `FROM type::table(<literal|$tb>)` —
  and returns a name only when the binding is a plain string. That name is then checked against
  `SECRET_TABLES` like any other. Deliberately narrow: widening the resolver is widening the wall.
- `ensure_read_only_with_vars(sql, vars)` carries the bindings to the gate; `ensure_read_only(sql)`
  stays, delegating with an empty slice. Passing no bindings can only refuse *more*, so no existing
  caller (in or out of tree) became less safe by not being updated.

**Net effect on the wall's strength: it went up.** A binding that chooses the secret plane
(`vars = {tb: "secret"}`) used to produce a generic "cannot be proven" rejection; it is now refused as
`SecretTable("secret")` — named, and by the same rule that catches the literal. What remains refused
as unprovable is what genuinely is: an unbound param, a param bound to a non-string, a computed idiom
(`FROM some.field`), a block, a cast, another function.

## Regression

`crates/host/tests/store_query_secret_wall_test.rs`:

- `a_composed_subquery_read_is_not_refused_for_its_own_field_references` — the composed read runs;
  three nested secret reads (subquery, correlated `IN (…)`, double-nested) still refuse.
- `a_dynamic_table_is_resolved_from_its_binding_and_refused_when_unprovable` — replaces
  `a_dynamic_table_expression_is_refused_even_when_innocent`, which encoded the over-refusal as the
  intended contract. Now: a secret-choosing binding is refused **by name** in four shapes; the
  innocent binding reads a real row; the three genuinely-unprovable shapes still refuse.

Both halves revert-checked **independently**: restoring the inherited position fails only the
composed test; disabling the resolution fails only the dynamic test — and the four
`flows_platform_nodes_test` store-CRUD tests, which is the product-level proof.

## Lesson

**A refusal that says "cannot be proven" is a claim about the information the checker was given, not
about the world.** Here the proof was in the caller's own `vars` and the gate simply wasn't handed
them — so the wall punished the one caller doing the safest thing (parameterising instead of
splicing). When a guard refuses something legitimate, check what it was denied access to before
accepting the false-refusal as the cost of safety.

And the meta-lesson, which is the expensive one: these six failures were carried across sessions as
"pre-existing, unrelated" — true, and worthless. A pre-existing failure is a bug someone else filed
for you; the only safe reading of one is the failure text.
