# Session — the secret wall's false refusals (2026-08-14)

Follow-on from the lb#167 session. That work reported six test failures as "pre-existing, unrelated"
— verified by stashing to a clean tree, which is true and, on its own, worthless. Reading the failure
text instead of the failure count found two live product bugs and one real test gap.

## What the failures actually were

**Six failures, three causes:**

| Suite | Count | Cause |
|---|---|---|
| `flows_platform_nodes_test` | 4 | secret wall refuses `type::table($tb)` → every store-CRUD flow node |
| `rules_test` | 2 | secret wall refuses composed subquery reads → every rules grid |
| `rules_test` | 1 | test gap: `datasource.add` needs a federation install record |

### 1. The table position never closed (composed reads)

`secret_wall.rs::walk` enters a **table position** by key (`what`, `tb`, …) and inherited it for the
whole subtree — deliberately, so a `type::table($t)` nested inside `FROM (…)` is still caught. But a
subquery is a *statement*: `FROM (SELECT data.name AS name FROM site ORDER BY data.name)` puts an
entire statement under `what`, and every field reference in its projection / `WHERE` / `ORDER BY`
serialises as an `Idiom` — read as "a table computed at run time".

Not an exotic shape. `store_query_run` **itself** wraps every validated SELECT in
`SELECT * FROM ({sql}) LIMIT …`, and every rules grid — `history("series", …).filter("value > 5.0")`,
the documented one-liner — compiles to exactly it.

Fix: the position **ends at a nested `Select`**. The subquery re-opens it through its own `what` key,
so nothing is given up.

### 2. "Cannot be proven" was not true (parameterised reads)

`SELECT * FROM type::table($tb)` names no table in the AST, so the wall refused it as unprovable. But
`store.query` takes `{sql, vars}` — the binding that chooses the table arrives **in the same
request**. The gate was simply never handed it.

The consequence is the sharp part: the flow `store-read`/`store-write`/`store-delete` nodes
parameterise their table *precisely so that no user text is ever spliced into SQL* (their module doc
says so). The wall rejected the caller doing the safest possible thing — two correct instincts in
direct collision.

Fix: `resolved_table` resolves exactly two shapes (`FROM $tb`, `FROM type::table(<literal|$tb>)`,
string bindings only) and the resolved name is judged like any other. `ensure_read_only_with_vars`
threads the bindings; the var-less `ensure_read_only` stays and delegates with an empty slice —
which can only refuse *more*, so nothing out of tree got less safe by not being updated.

**The wall got stronger.** `vars = {tb: "secret"}` used to be a generic "cannot be proven"
rejection; it is now `SecretTable("secret")` — named, by the same rule that catches the literal.
What stays refused as unprovable is what genuinely is: an unbound param, a param bound to a
non-string, a computed idiom, a block, a cast, another function.

### 3. The one real test gap

`registered_datasource_is_in_the_rule_allowlist` booted a bare node, and `datasource.add`
self-approves its endpoint by appending `net:tls:{host}:{port}:connect` to the **federation install
grant** — absent install → `EndpointRefused`, before the allowlist under test is reached. Seeded a
real install record through the real `record_install` (rule 9), mirroring
`datasource_crud_ownership_test`. No sidecar: `add` never touches the child.

## A test that had to change

`a_dynamic_table_expression_is_refused_even_when_innocent` encoded the over-refusal as the *intended*
contract ("the deliberate false-refusal edge the wall accepts"). It could not survive the fix, so it
was replaced by `a_dynamic_table_is_resolved_from_its_binding_and_refused_when_unprovable`, which
asserts the stronger contract: four secret-choosing shapes refused **by name**, the innocent binding
reading a real row, three genuinely-unprovable shapes still refused.

Deleting a security test to make a change pass is exactly the move that should be viewed with
suspicion, so the replacement is deliberately wider than the original, and every refusal case it
drops is covered somewhere in the same file.

## Tests

- `store_query_secret_wall_test` — 7 passed (2 new).
- `flows_platform_nodes_test` — 10 passed (was 6/4).
- `rules_test` — 22 passed (was 19/3).

**Both halves of the wall fix revert-checked independently:** restoring the inherited table position
fails only the composed test; disabling the resolution fails only the dynamic test *and* the four
`flows_platform_nodes_test` store-CRUD tests — the product-level proof, not just the unit one.

## Cross-links
- Debug: `debugging/store/secret-wall-refuses-composed-and-parameterised-reads.md`.
- The wall's original scope: node-update scope, decision 9 (`9fd99175`).
- Sibling session: `sessions/rules/scheduled-rules-session.md` (the lb#167 addendum this came out of).
