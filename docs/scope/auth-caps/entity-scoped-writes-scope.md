# Auth-caps scope — entity-scoped writes (a restricted principal edits only its own entities' rows)

Status: **built in this PR** (scope + implementation). Promotes to `public/auth-caps/` once shipped.

> Read with: `entity-scoped-data-scope.md` (the row policy and the resolver this reuses — and the rule
> this scope relaxes, "writes refused when restricted"), `entity-scoped-grants-scope.md`,
> `../packs/entity-typed-value-scope.md` (the settings rows that motivate it),
> `../testing/testing-scope.md` §2.

`entity-scoped-data` narrows a restricted principal's READS to their entities and refuses every write
on a policy-enforced source (except an extension's own tables). That was the safe first step, and it
leaves a real gap: per-entity data that the people responsible for an entity should maintain. The
first ask is per-site settings in a pack datasource — a site's panel capacity, its monthly design
figures, its comfort band. The facility manager for one site should edit that site's settings and
nothing else; today the choice is "admin edits everything" or "the FM edits every site's rows",
because a workspace-wide `mcp:federation.write:call` reaches every row and a restricted one reaches
none.

We want **entity-scoped writes**: on a table an admin marks writable, a restricted principal's
`federation.write` / `federation.delete` succeeds only when every row it touches — as written AND as
it stood before — belongs to one of their entities. Reads already work this way; writes get the same
set, the same resolver, the same fail-closed rule.

## Goals

- **Opt-in per table, by an admin.** A policy table gains `writable: true`. Absent ⇒ today's refusal,
  so every existing policy behaves exactly as before.
- **Only `entity` tables.** A writable table must be a `TableRule::Entity` (the row carries its entity
  id in one column). A `keyed` table resolves its entity through another table, so a write could
  re-key a row out of scope; it stays refused.
- **Both sides of the write are in scope.** For each row in the batch:
  - the NEW value of the entity column is in the caller's set (no writing a row for another entity);
  - for an upsert whose key matches an existing row, that row's CURRENT entity column is in the set
    too (no taking over, or moving, another entity's row);
  - a delete removes only rows whose current entity column is in the set (a key held by another
    entity's row simply deletes nothing — refusing would reveal that the row exists).
- **All or nothing.** The batch runs in one transaction in the sidecar; one row out of scope refuses
  the whole write, and nothing is written.
- **Fail closed.** Empty set, a NULL or missing entity value, a table not marked writable, a column
  the policy does not name: refused.
- **The caps wall is unchanged.** The caller still needs `mcp:federation.write:call` /
  `mcp:federation.delete:call`; this scope only stops refusing what those caps would otherwise allow,
  for rows in scope. Admins and `Principal::system` stay unrestricted.
- **Rule 10.** The entity is an opaque id and the column an opaque name from the policy; lb never
  learns what a site is.

## Non-goals

- **DDL.** `federation.migrate` stays refused for a restricted caller (a column added for one site is
  added for all).
- **Column-level rules.** Which columns a restricted caller may change (e.g. a typed setting's schema)
  is not this scope; a row in scope is writable whole. A product that needs it hides the control.
- **Keyed tables, store records, series.** Federated `entity` tables only, as above.
- **Audit trail.** Writes are already attributed by the gateway's call log; a per-row history is its
  own scope.

## Design

### Policy
`TableRule::Entity { column, writable: bool }` (default `false`), set with the existing admin-only
`federation.row_policy_set`. A `keyed` rule has no such flag, so it can never be writable.

### Host
`refuse_unless_owned` becomes `write_scope_for` (host `federation/row_policy.rs`), with three
outcomes: unrestricted, or an extension's own table ⇒ write as before; restricted and the table is a
writable `entity` rule in the enforced policy ⇒ attach `write_scope {policy, ids}` to the sidecar
input; anything else ⇒ `Denied`. It rides its own key, not `row_scope`, so the sidecar's existing gate
("only `federation.query` may carry a `row_scope`") stays exactly as it is.

### When the entity is part of the key (the simple case)
If the table's primary key contains the entity column (`site_id + name`, not a bare `id`), an upsert
can only ever match a row of the SAME entity, so "the existing row is in scope" follows from "the new
row is in scope" — the statement's guard below never even fires. This is how the `waste` extension
avoids takeover today (its key is `site_ref + period + stream_id`, and it fills `site_ref` from the
one site it checked). A table keyed by a bare id relies on the guard. Recommendation for pack
authors: key per-entity tables by the entity column plus a name.

### Sidecar
`row_policy::write` turns `write_scope` into a `WriteGuard {column, ids}` — re-checking that the
policy parses, the table is a writable `entity` rule and the set is non-empty — and checks every
incoming row's entity value is one of the ids. Then the guard goes INTO the statement, so the check on
the existing row and the write are one atomic step (no read-then-write window):
- upsert: `INSERT … ON CONFLICT(key) DO UPDATE SET … WHERE "table"."<entity>" IN (<ids>)`. An
  existing out-of-scope row is left alone, the statement changes zero rows, and the source refuses
  and rolls the batch back (`write_sql.rs`, run by `sqlite::run_rows` / `pg_write::run_rows`);
- delete: `DELETE … WHERE <key> AND "table"."<entity>" IN (<ids>)`.
Ids, keys and cells bind as parameters; identifiers come from the checked policy. One generator
(`source/write_sql.rs`) serves both engines — they used to build the same SQL twice.

## Tests (real sqlite + postgres, real host, no mocks)
- `row_policy/write_tests.rs` (sqlite, through the scoped entry points): own rows written; a row for
  another entity, a takeover of another entity's key, moving an own row out, and a batch mixing one
  bad row are each refused with nothing written; a delete never reaches another entity's row; not
  writable and an empty set fail closed; an unrestricted call writes as before. Mutation-checked: an
  always-true guard, a skipped row check and an always-true delete guard each turn their tests red.
- `tests/scoped_write_pg_test.rs` (postgres): the guarded upsert's rollback on a takeover and the
  guarded delete, on a real server (`LB_TEST_PG_DSN`; SKIP without one).
- `host/tests/federation_scoped_writes_test.rs` (real node, real sidecar): a member whose handed menu
  marks only Site A writes and deletes Site A's rows of a writable table and nothing of Site B's; a
  table not marked writable stays `Denied`; an admin writes as before.
- Unchanged and still green: `entity_scoped_data_test`, `federation_ext_own_tables_test`, every
  federation test (sqlite and postgres).

## Precedent: how `waste` does it today, and why a core path is still wanted
The `waste` extension (NubeIO/rust-waste-management) already gives restricted people per-site
writes, with no core change: people call its own verb (`waste.entry.upsert_month`), the extension
asks `authz.entity_scope {table: "site", subject}` — the resolver this scope reuses — and writes AS
ITSELF to a table it created (an extension's own tables are the one exception to "restricted writes
are refused"). It works, and it is the right shape for an extension's own data. For a pack's tables
it does not generalise: every per-entity table would need its own extension, verbs and page, and the
generic entity page (which writes through `federation.write`) could not serve restricted people at
all. This scope gives every pack table the same guarantee once, with the same resolver.

## Rollout (first consumer: rubix-ai per-site settings)
Mark the settings table writable in the config source's policy → grant the single-site team
`federation.write` → verify as that team's login that it sees and edits only its site.

## Open questions
- **O-1:** should a writable table also be able to say "insert only, no delete"? Recommendation: no
  until asked; the delete cap already separates them.
- **O-2:** global (non-entity) settings sit beside per-entity ones in a pack. Under an enforced policy
  a restricted reader cannot read a table the policy does not name. Leave it to the pack to keep
  global settings in a separate source, or allow a policy to list tables as "unscoped, read-only"?
  Recommendation: the latter, as its own small change — it is a read rule, not a write rule.
  **Resolved:** a policy names such a table `{kind: "shared"}` (`row_policy::TableRule::Shared`):
  read whole by every restricted caller, entities or not, and never written by one, since only a
  writable `entity` rule writes.
