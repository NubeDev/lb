# Auth-caps scope — entity-scoped writes (a restricted principal edits only its own entities' rows)

Status: scope (the ask). Promotes to `public/auth-caps/` once shipped.

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
  - a delete removes only rows whose current entity column is in the set.
- **All or nothing.** The check and the write run in one transaction in the sidecar; one row out of
  scope refuses the whole batch with a plain `Denied`, and nothing is written.
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
`federation.row_policy_set`. `RowPolicy::check` refuses `writable` on a `keyed` rule.

### Host
`refuse_unless_owned` (host `federation/row_policy.rs`) gains a third outcome: restricted, the table
is a writable `entity` rule in the enforced policy ⇒ allow, attaching `row_scope {policy, ids}` to the
sidecar input exactly as `federation.query` does. Everything else is unchanged (own tables still
pass, other tables still `Denied`).

### When the entity is part of the key (the simple case)
If the table's primary key contains the entity column (`site_id + name`, not a bare `id`), an upsert
can only ever match a row of the SAME entity, so "the existing row is in scope" follows from "the new
row is in scope" and step 2 below is skipped. This is how the `waste` extension avoids takeover today
(its key is `site_ref + period + stream_id`, and it fills `site_ref` from the one site it checked). A
writable table keyed this way needs only the per-row check on the new value; a table keyed by a bare
id needs the read-before-write in step 2. Recommendation for pack authors: key per-entity tables by
the entity column plus a name.

### Sidecar
`write_rows` / `delete_rows` take the optional scope. Under it, inside the existing transaction:
1. every incoming row's entity column ∈ `ids` (else refuse);
2. `SELECT key, entity FROM table WHERE key IN (batch keys)` — every existing row's entity ∈ `ids`
   (else refuse);
3. write; for a delete, `DELETE … WHERE key IN (…) AND entity IN (ids)` and refuse if the count
   differs from the request.
Ids and keys bind as parameters; identifiers come from the checked policy (plain lowercase). Both
engines (sqlite, postgres) get the same three steps.

## Test plan (real sqlite + postgres, real host, no mocks)
- In scope: insert, update, delete a row for the caller's entity — succeed.
- Out of scope, each refused with nothing written: insert a row for another entity; update another
  entity's row by its key; move an own row to another entity; delete another entity's row; a batch
  mixing one in-scope and one out-of-scope row.
- Fail closed: empty set; NULL entity value; table not writable; `keyed` table; no policy enforced
  (today's behaviour).
- Unchanged: admin unrestricted; an extension's own tables; reads narrowed exactly as before.

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
