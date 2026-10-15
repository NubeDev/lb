# Packs scope — entity typed value (a row says what its value is)

Status: scope (the ask). Promotes to `doc-site/content/public/packs/packs.md` once shipped (there is
no packs page under `doc-site/content/public/` yet — writing it is its own task, as
`entity-source-refs-scope.md` notes).

A settings-style entity holds rows of different shapes in one table: one row is a number (a panel
capacity, `190`), the next a document (twelve monthly design figures, `{"jan":123400, …}`), the next
a flag. Today a downstream surface can only guess how to show and edit each row — the binding names
the table, the key and the label, nothing about what a row's value *is*. This scope adds the smallest
core change that turns that guess into contract: an optional **`typed_value:` block on the entity
binding** naming three columns of the bound table — the one carrying each row's **type**
(`number` | `text` | `bool` | `json`), the one carrying its **value**, and, for `json` rows, the one
carrying the **JSON Schema** that value must match. Like `table`/`geo`/`charts`/`refs`, it is an
**address, not behavior**: core carries it in the receipt, generates no SQL, reads no rows and
validates only shape. Downstream reads it off `pack.get` and renders a number box, a text box, a
switch, or a schema-driven form per row — the type lives in the DATA, one table serves every shape.

## Goals

1. **Optional `typed_value:` on `Entity`** — `{ type, value, schema? }`, each a column name of the
   bound `table`. New file `rust/crates/packs/src/manifest_typed.rs` (`TypedValue`) beside
   `manifest.rs`, which is past the size line — the `manifest_refs.rs` precedent. Serde stays
   `deny_unknown_fields`; absent ⇒ today's byte-identical manifest.
2. **Carry it in the receipt** — nothing to write: `Receipt.manifest` carries the whole manifest, so
   `pack.get` returns `typed_value` the moment the field exists (asserted, not assumed).
3. **Validate shape, not data**, in `pack.validate` (new `validate_typed.rs`, the `validate_refs.rs`
   precedent):
   - **error** — `typed_value` on an entity with no `table` (nothing to address), or two of the three
     naming the same column;
   - **warning** — a named column that the pack's schema does not declare on `table` (the binding's
     own rule: where the schema is opaque the apply is the oracle).
4. **The type words are a documented convention, not a core enum.** `number`, `text`, `bool`, `json`
   are what downstream renders; core never reads a row, so it never sees one. A word downstream does
   not know renders as `text` (the no-surprise fallback), so adding a word later is a downstream
   change, not a core one.

**Prerequisite, same PR:** the sqlite direct read typed each column from its FIRST row, so a column
holding a number on one row and JSON text on the next read back half-blank (`SELECT NULL UNION ALL
SELECT 190.0` → `[null, null]`). `source/sqlite_cells.rs` reads each cell by its own storage class
instead. A typed `value` column is exactly that mixed column, so this scope cannot ship without it.

## Non-goals

- **No data validation in core.** Core does not check a row's `value` against its `type` or its
  `schema`, on apply or on `federation.write`. That is the line `pack-entity-binding-scope.md` exists
  to hold ("the moment the binding wants to enforce anything about the data, that is again a new
  scope"). Validation is the consumer's: it already ships JSON Schema 2020-12 with `ajv` in the
  editor (`flows` config forms), and a server-side write gate, if ever wanted, is its own scope.
- **No type coercion.** Core does not cast `value` on read or write; sqlite stores what it is given
  and the cells fix returns it as stored.
- **No new verb, no new `Kind`.** A field on an existing block, carried by an existing receipt.
- **No per-entity schema.** The schema is a per-ROW column, not a manifest field: two `json` rows of
  one entity (monthly design figures, a day/night comfort band) carry different shapes.

## Intent / approach

The type lives in the row because the shape varies by row, not by entity. Three alternatives were
considered and rejected:

- **A column per shape** (`value` plus `jan … dec`): every row carries columns it does not use, the
  table widens with each new shape, and it was the shape that exposed the first-row typing bug (a
  blank `jan` on the first row blanked every row's `jan`).
- **Downstream infers the type from the value**: works for display, fails for the add form (a new
  row has no value to infer from) and leaves the schema nowhere to live.
- **A schema per entity in the manifest**: forces one shape per entity, which is the problem this
  solves.

Naming three columns keeps the binding an address. It is the fifth optional block on `Entity` after
`table`/`pk`/`parent_fk`/`display`, `geo`, `charts` and `refs` — each one a projection a downstream
surface reads, none of them behavior — so it extends the binding the way those did rather than
crossing into an ORM.

## How it fits the core

- **Tenancy / isolation:** unchanged — rows are read and written through `federation.*`, workspace
  first; the receipt is workspace-scoped like every other.
- **Capabilities:** none added. Declaring `typed_value` grants nothing; editing a row still needs the
  caller's own `mcp:federation.write:call`.
- **Placement:** either — a manifest shape, no runtime.
- **MCP surface:** none new. `pack.validate`/`pack.apply` gain the shape check; `pack.get` returns
  the field.
- **Data:** the pack's own datasource table; core writes nothing new.
- **Rule 10:** nothing names a pack, an entity, a column or an extension; the block is opaque data
  carried for whoever reads it.

## Testing plan

- **Unit — `manifest_typed`:** parse round-trip; absent block is byte-identical; a typo'd key is a
  line-numbered error; `schema` optional.
- **Unit — `validate_typed`:** `typed_value` without `table` errors; two names on one column errors;
  a column the schema does not declare warns; a clean block passes silently.
- **Integration — federation, real sqlite file (`tests/sqlite_json_cells_test.rs`):** a NULL first
  row does not blank the column; a column holding a number on one row and JSON text on the next reads
  back as both; cells keep their storage class (integer, real, text, blob as hex). Mutation check:
  route `SqliteSource::query_direct_json` back through Arrow and the first two go red.
- **Receipt carriage:** `pack.get` after `pack.apply` returns the block unchanged (the
  `pack_refs_test.rs` pattern, on a real node).

**Skill doc:** N/A — no new drivable surface; the block is authored in `pack.yaml` and documented
on the packs public page when that page is written.

## Open questions

- **O-1:** should `schema` be allowed on a non-`json` row (e.g. a `number` with `minimum`/`maximum`)?
  Recommendation: yes, downstream's call — core never reads it, so it costs core nothing to allow.

## Related

- `pack-entity-binding-scope.md` — the binding this extends, and the no-data-validation line this
  scope keeps.
- `entity-source-refs-scope.md` — the previous projection block on `Entity`; the file-split and
  receipt-carriage precedents followed here.
- `rust/crates/flows/src/config_schema.rs` — the JSON Schema 2020-12 dialect the consumer validates
  with (`jsonschema` host-side, `ajv` editor-side).
- **`NubeIO/rubix-ai`** — the first consumer: a site-settings entity where `Effective kW` is a number
  and `Design generation` is a monthly `json` document, edited through a schema-driven form.
