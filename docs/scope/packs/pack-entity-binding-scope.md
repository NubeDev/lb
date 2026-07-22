# Packs scope — the entity→table binding (making `entities` addressable as data, without making it an ORM)

Status: **SHIPPED in code, 2026-07-22 status flip** — the four binding fields exist on `Entity`
(`rust/crates/packs/src/manifest.rs:94-106`), `validate_binding` is implemented
(`rust/crates/packs/src/binding.rs`) and wired into the linter (`validate.rs`), and the
seed-run-once / never-reclobber ownership decision is implemented (`host/src/pack/apply.rs` +
`sqlite.rs::resolve_existing`). **Release tag + downstream pin bump still pending** (rubix-ai consumes
via a local `[patch]`). Open questions resolved: **O-1** — yes, `federation.write` reaches the
materialized in-process sqlite source (it registers as a normal datasource at loopback `127.0.0.1:0`,
which `resolve` + `enforce_endpoint` accept; no new verb was needed); **O-2** — `federation.delete`
exists (`host/src/federation/delete.rs`); **O-3** — `for_each` stays deferred (U-entity-foreach).
Owning repo: **lb (core)** — the manifest, the receipt,
and the seed re-apply semantics all live in `rust/crates/packs` + `rust/crates/host/src/pack/`. Origin: the
downstream consumer scope `NubeIO/rubix-ai → docs/scope/packs/entity-data-plane-scope.md`, which
wants to give an operator CRUD over a pack's entity rows and make those rows the spine for vars,
dashboards, and rules. That surface is all downstream `ui/`; the **one thing it cannot do without
core** is know *which table an entity maps to*. This scope adds exactly that binding — the smallest
core change that unblocks the whole downstream data plane — and settles the re-apply question a data
plane forces. Parent: `pack-core-scope.md` (this is a new, optional shape on its `entities` block,
not a new `Kind`).

`pack-core-scope.md` drew a hard line: *"`entities` stays a vocabulary — no relations beyond
`parent`, no constraints, no codegen. The moment it needs behavior, that is a NEW scope, not a
field."* This is that new scope, and it holds the line: the binding is a **projection** (which
table, which key, which parent column, which label), not behavior. It stores no rows, generates no
SQL in core, validates no data. It is the address, and downstream reads/writes it through the
`federation.*` verbs that already ship.

## The one fact that shapes everything

`Entity` today is `{label, parent?, kinds[], units{}}` (`crates/packs/src/manifest.rs:75-86`),
explicitly *"a vocabulary, not an ORM."* It is carried in the receipt and read by downstream pickers
as **documentation** — nothing maps `site` to the `site` table, so nothing can address the rows the
pack seeds. The parent scope named the missing piece precisely: *"per-entity query generation needs
an explicit entity→table binding in the manifest (which would EXTEND this promise)."* This scope is
that extension, and nothing more: adding four optional fields to `Entity`, carrying them in the
receipt, and deciding how re-apply treats rows an operator has since edited.

## Goals

1. **Optional binding fields on `Entity`** — `table`, `pk`, `parent_fk`, `display` — all optional,
   `deny_unknown_fields`, line-numbered errors. An entity with no `table` is exactly today's
   shape-only vocabulary (the promise is unbroken); an entity WITH a `table` becomes addressable.
2. **Carry the binding in the receipt** so `pack.get` returns it — the binding is data a downstream
   surface reads, identical to how `entities` already rides the receipt. No new verb.
3. **Validate the binding shape (not the data)** in `pack.validate`: if `table` is set, it must name
   a table the pack's `datasource` schema declares, `pk`/`parent_fk`/`display` must be columns of
   that table, and `parent_fk` requires the entity to declare a `parent`. A **warning, not a gate**
   where the schema is external/opaque (the dialect-lint precedent — the real oracle is the apply).
4. **Settle seed-vs-operator ownership** (the re-apply decision a data plane forces): once a pack's
   datasource has been applied, re-apply must **not** clobber rows an operator has edited or added.
   Seeded rows are *starting data*, not pack-owned objects.
5. **Confirm or provide a write path to the in-process sqlite source** (O-1): `federation.write`
   today reaches a *registered external source*; a pack's datasource is an in-process sqlite
   materializer. Prove it reaches, or name the write path that does — this gates the whole
   downstream data plane.

## Non-goals

- **No SQL generation in core.** Core stores the binding; it does not emit `SELECT`/`INSERT` from
  it. Downstream builds the query from `{table, pk, parent_fk, display}` and runs it through the
  existing `federation.query`/`federation.write` verbs. Core's parse-allowlist stays the boundary.
- **No new `Kind`, no new verb** (except possibly the O-1 write path). The binding is a field on an
  existing block; the receipt already carries `entities`. `federation.*` already exists.
- **No constraints/validation/computed columns/relations beyond `parent_fk`.** The moment the
  binding wants to *enforce* anything about the data, that is again a new scope. This one is an
  address.
- **No entity→multiple-tables in v1.** One entity, one primary table + one `parent_fk`. A domain
  whose entity spans tables (product-management's `item` + `item_event`) binds the primary table
  and leaves the rest to downstream query authoring — stated so the field's optionality is honest,
  not a hidden TODO.
- **No `for_each` rule input here.** Feeding the operator-managed entity set into a rule is a
  rules-engine change (ask **U-entity-foreach**, named below), scoped separately; this scope only
  makes the set *addressable*, which that ask then consumes.

## Intent / approach

### The field (manifest)

```yaml
entities:
  site:  { label: Site,      table: site,  pk: id, display: name }
  meter: { label: Equipment, parent: site, table: meter,
           pk: id, parent_fk: site_id, display: name, kinds: [ahu, chiller, meter] }
  point: { label: Point,     parent: meter, table: point,
           pk: id, parent_fk: meter_id, display: name }
```

Four optional fields on `Entity` (`manifest.rs`): `table: Option<String>`, `pk: Option<String>`,
`parent_fk: Option<String>`, `display: Option<String>`. Serde stays `deny_unknown_fields`;
line-numbered errors on a malformed block. **Optionality is the whole safety property** — every
existing pack parses unchanged, and the shape-only promise holds for any entity that omits `table`.

### The validation (pure, unit-tested — the `pack.validate` half)

A pure `validate_binding(entity, schema)` over the pack's own `datasource.schema` SQL (the DDL the
pack ships, which `pack.validate` already parses for the dialect lint):

- `table` set → must be a declared table (warn if the schema is opaque/external, the dialect-lint
  precedent);
- `pk` / `display` set → must be columns of `table`;
- `parent_fk` set → the entity must declare `parent`, and `parent_fk` must be a column of `table`;
- consistency is a **warning that does not gate apply** where the schema can't be statically read —
  the real oracle is applying against the real source, exactly as the SQL dialect poison list is.

### The receipt carry (mechanical)

`entities` is already in the receipt/`pack.get`; the four fields ride along in the same struct. A
downstream reader (`pack.get`) gets `{label, parent, kinds, units, table?, pk?, parent_fk?,
display?}`. No new read verb, no envelope change.

### The seed-ownership decision (the load-bearing one)

A data plane inverts the clobber rule for datasource ROWS the way `users`/`teams` inverted it for
people (`pack-workspace-seed-scope.md` downstream). The decision, stated so the next PR doesn't
re-litigate it:

- **A pack's datasource SCHEMA (DDL) stays pack-owned** — re-apply may migrate it (additive), as
  today.
- **Seeded ROWS are starting data, applied ONCE, never re-clobbered.** The receipt already knows
  first-apply-only for rules; the same flag governs the seed. A re-apply of a pack whose `seed.sql`
  changed does **not** re-run the seed over rows an operator now owns — it is a no-op-with-a-note on
  the datasource's data, exactly as rules are run-once. Re-seeding is an explicit operator act (a
  future `pack.reseed`), never a silent side effect of re-apply.

This makes "an operator CRUDs the seeded sites, then the pack ships v4" safe by construction: the
schema can evolve, the data is the operator's.

## How it fits

- **Rule 10**: the binding is opaque data on a block the applier already carries; core still knows
  no pack by name. No arm branches on `bas`; the field means the same for every pack.
- **Workspace wall / caps**: unchanged — the binding is read via `pack.get` (member-read, receipts
  are operator documentation) and *acted on* downstream via `federation.*`, each caps-walled and
  workspace-scoped. The binding grants nothing; it is an address a caller still needs
  `federation.query`/`.write` to use.
- **The MCP surface**: no new verb (modulo O-1). `pack.validate` gains binding lints; `pack.get`
  gains four optional fields. Both are additive, back-compatible reads.
- **Symmetric nodes**: nothing role-gated; a binding means the same wherever the workspace lives.
- **No mocks**: `pack.validate` binding tests are pure; the seed-ownership behavior is proven on the
  real embedded node (`mem://` store, the real sqlite materializer) — apply, CRUD a row via
  `federation.write`, re-apply, assert the row survived.

## Example flow

1. A pack author adds `table`/`pk`/`display`/`parent_fk` to `bas`'s entities and bumps the version.
2. `pack.validate` parses the binding, checks each names a real table/column in `schema.sql`, warns
   on nothing (bas is clean) — the CI gate passes.
3. `pack.apply` on a blank workspace registers the datasource, seeds the three sites ONCE, writes a
   receipt whose `entities` now carry the binding.
4. `pack.get` returns the binding; the downstream Sites page (rubix-ai Phase C) renders and edits
   `site` rows via `federation.query`/`.write`.
5. The operator adds a fourth site. The pack ships v4 with a changed `seed.sql`; re-apply migrates
   any schema change but **does not** touch the four rows — the operator's data is intact.

## Testing plan

- **Manifest/validate (unit, pure)**: binding serde (`deny_unknown_fields`, line-numbered errors);
  `validate_binding` over a fixture schema — good binding passes, a `table`/`pk`/`parent_fk` naming
  a missing table/column warns, `parent_fk` without `parent` errors; an entity with no `table`
  yields today's behavior byte-for-byte (the un-broken-promise test).
- **Receipt carry (integration)**: apply a bound pack on the real node, assert `pack.get` returns
  the four fields; apply an unbound pack, assert they're absent (not null-spammed).
- **Seed ownership (integration, the important one)**: apply `bas`, `federation.write` a new site +
  edit a seeded one, re-apply `bas` (changed seed) → assert both survive; assert the run reports the
  seed as run-once/skipped, not clobbered.
- **O-1 write path (integration, gating)**: `federation.write` an INSERT against the in-process
  sqlite `demo-buildings` source; assert it lands (or, if it's refused as non-external, that failure
  IS the finding that scopes the write-path ask).
- **Workspace-isolation**: bound pack in ws A, another in ws B; `pack.get` and `federation.write`
  cross-reads refused.

## Risks & hard problems

- **O-1 — the in-process sqlite write path (the gating unknown).** `federation.write` is documented
  for a *registered external source* and enforces `net:*` on the source endpoint
  (`crates/host/src/federation/write.rs:1-15`). A pack's datasource is the **in-process sqlite
  materializer** (`pack-core-scope.md`'s "one place an apply touches the node filesystem"), DSN =
  node-local file. If `federation.write` resolves and writes it, Phase A downstream is unblocked as
  is. If it refuses (endpoint grant, external-only assumption), core must provide a write path for
  materialized sources — the same shape (`{source, table, columns, rows, key}`), reaching the
  in-process db instead of a sidecar. **Prove this live before anything else** — it is the pivot the
  whole data plane turns on.
- **Binding drift from the schema**: an operator migrates the table (renames `name`), and `display:
  name` now dangles. The validator catches it at author time; at runtime the downstream read simply
  finds no column and degrades — core promises the binding is *well-formed at apply*, not that a
  later hand-migration keeps it true (same honesty as any receipt-vs-live drift).
- **The optionality must be real, not aspirational**: PM's `item` spans `item` + `item_event` and
  will bind only the primary table. If any consumer assumes "bound entity ⇒ single-table complete
  CRUD", it breaks PM. The field's contract is *"names the primary table"*, stated so downstream
  doesn't over-read it.
- **Scope creep back into ORM**: every future request to add `unique`, `required`, `computed`, or a
  second FK is the line this scope exists to hold. Route them to a new scope, not a fifth field.

## Open questions

- **O-1 (gating, above):** does `federation.write` reach the in-process sqlite source, or is a
  materialized-source write path a core ask? Empirical — settle by running it on the real node.
- **O-2:** row DELETE — is there a `federation.delete`, should there be, or is delete a
  `federation.write` tombstone convention? Downstream Phase A needs delete; core owns the verb.
- **O-3 (defers to U-entity-foreach):** feeding the operator-managed entity set into a rule as a
  `for_each` input is a rules-engine change — named here for the end-to-end picture, scoped on its
  own when the data plane's read/write half has shipped.

## Related

- `docs/scope/packs/pack-core-scope.md` — the engine this extends; its `entities`-is-vocabulary line
  and "new field vs new scope" rule are the constraints this scope deliberately honors. The binding
  is a shape on its manifest, carried by its receipt — not a new `Kind`.
- **`NubeIO/rubix-ai` → `docs/scope/packs/entity-data-plane-scope.md`** — the consumer that motivates
  this and builds on it: operator row-CRUD (Phase A, rides `federation.write` today), generated
  entity pages (Phase C, reads this binding), the `entity` var/query source (Phase D), and the
  deferred `for_each` rule input (Phase E → U-entity-foreach). It bumps the `node-v*` pin this tags.
- `crates/host/src/federation/write.rs` + `crates/federation/src/write.rs` — the `federation.write`
  verb whose reach over the in-process sqlite source O-1 must settle.
- `crates/packs/src/manifest.rs` — the `Entity` struct the four optional fields land on.
- `docs/WORKFLOW-LB.md` (downstream) — the PR → `node-v*` tag → pin-bump flow that lands this for the
  consumer.
