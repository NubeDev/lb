# Packs scope — pack UPGRADE (a version bump re-applies additively, preserving operator rows)

Status: **scope (the ask), 2026-07-22.** Owning repo: **lb (core)** — the decision matrix, the apply
orchestration, and the materialized-source schema reconciliation all live in `crates/packs` +
`crates/host/src/pack/`. Origin: the downstream consumer `NubeIO/rubix-ai` hit the wall directly — an
operator edited `bas` (v1 → v4, adding the entity→table bindings AND `lat`/`lng` columns), ran
`make pack-apply PACK=bas`, and got `pack.apply refused (400): manifest version 4 is HIGHER than the
applied version 1 — upgrade is not built yet`. The refusal is honest (the engine's own words), but it
means a pack **cannot evolve** — which contradicts the whole domain-pack premise ("a pack installs a
product's skeleton and its opening data" implies the skeleton grows over time). This scope builds the
one path the decision matrix explicitly stubs: **upgrade.** Parent: `pack-core-scope.md` (this fills the
`version > prior` arm its `decide()` refuses) + `pack-entity-binding-scope.md` (whose seed-ownership
re-apply rule this extends from same-version to a version bump).

## The one fact that shapes everything

`decide()` (`crates/packs/src/decision.rs`) has exactly one un-built arm:

```
version < prior.version   → Refuse (downgrade — always)
version > prior.version   → Refuse ("upgrade is not built yet")   ← THIS
version == prior, checksum == prior  → NoOp (or re-Apply if the prior was partial)
version == prior, checksum != prior  → Refuse (drift — bump the version)
no prior                  → Apply { run_rules: true }  (first apply)
```

Everything the matrix needs for an upgrade **already exists except the schema reconciliation**:
- Row preservation on re-apply: `sqlite::resolve_existing` (pack-entity-binding-scope) keeps the
  operator's rows — a re-apply does not rebuild or re-seed.
- Rules run-once: `run_rules` is false on any non-first apply, so an upgrade does not re-fire rules.
- Receipt carry: the receipt already stores version + checksum + the full manifest; recording the new
  ones is mechanical.
- Additive DDL planning + destructive refusal: `federation.migrate`'s `plan_migrate`
  (`crates/federation/src/source/dialect.rs`) diffs a desired schema against a live catalog and plans
  CREATE TABLE / ADD COLUMN / ADD FK, **refusing destructive changes** (a dropped column is a future
  verb, not a silent drop). This is exactly an upgrade's schema step.

So an upgrade is: **accept `version > prior` as a new `Upgrade` decision, run the same row-preserving
re-apply, reconcile the materialized schema ADDITIVELY, and record the new version.** The single genuinely
new piece is bridging the pack's raw `schema.sql` text to the additive reconciliation (§"The schema
reconciliation").

## Goals

1. **An `Upgrade` decision.** `decide()` returns `Decision::Upgrade` for `version > prior.version`
   instead of refusing. Downgrade stays refused (always). The decision carries `run_rules: false`
   (an upgrade is never a first apply) and a `from_version`/`to_version` pair for the receipt + the
   loud-listing.
2. **Row-preserving, additive re-apply.** An upgrade drives the SAME plan an apply does (every object
   through its own capability-checked seam), with the seed-ownership rule intact (rows are the
   operator's, never re-seeded). The datasource object additionally **reconciles the schema**: any table
   or column the new `schema.sql` declares that the live db lacks is CREATE'd / ADD COLUMN'd; nothing is
   dropped or retyped.
3. **Destructive changes are REFUSED, not silently dropped.** If the upgrade's schema removes or retypes
   a column the live db has, the datasource object is `failed` with a clear message (the same posture as
   `federation.migrate`), and the receipt records the partial — the operator's data is never mangled by
   a pack bump. Re-typing / dropping is a future explicit `pack.migrate --destructive` act.
4. **Receipt records the upgrade.** The new receipt carries the new version + checksum + manifest, and
   the apply result lists `upgraded pack: v1 → v4` and every object's outcome (an upgrade is a loud act,
   like a clobber).
5. **Idempotent after the fact.** Re-running the same upgraded version is a NoOp (the matrix's
   same-version + same-checksum arm), so "apply v4 twice" is safe.

## Non-goals

- **No downgrade.** `version < prior` stays refused, always. Rolling back a pack is a restore act, not an
  apply (the store's own concern).
- **No destructive migration.** Dropping/retyping a column via a version bump is REFUSED (goal 3). The
  destructive path is a separate, explicit, named future verb — never a side effect of an upgrade. This
  is the same line `federation.migrate` already holds.
- **No data migration / backfill.** An upgrade adds a nullable column; it does not compute values for it.
  Backfilling `lat`/`lng` for existing sites is an operator edit (the entity data plane's row CRUD), or
  a future `pack.reseed`, never an upgrade side effect.
- **No re-seed.** The seed is run-once (pack-entity-binding-scope). An upgrade whose `seed.sql` changed
  does NOT re-run it over operator-owned rows — re-seeding stays the explicit future `pack.reseed`.
- **No cross-engine migration.** Schema reconciliation is sqlite-only (the one materialized engine), the
  same limit `materialize` already has. A postgres/external pack registers a pointer; its schema is the
  operator's to migrate.
- **No multi-version stepping.** v1 → v4 is one upgrade, not three sequential ones. The schema
  reconciliation is a diff against the LIVE db, so it reaches the v4 shape in one pass regardless of the
  intermediate versions — there is no per-version migration script to chain (packs ship a target schema,
  not deltas).

## Intent / approach

### The decision (pure, unit-tested — the `decide()` half)

Add `Decision::Upgrade { run_rules: false, from_version, to_version }`. The `version > prior` arm returns
it. `resolve_decision` (`apply.rs`) maps `Upgrade` to `(run_rules=false, clobbering=true)` — an upgrade
clobbers pack-owned objects (rules/dashboards/etc. are re-authored to the new version) exactly as a
same-version partial re-apply does, and the clobber is listed. The ONLY behavioral difference from a
same-version re-apply is the datasource object's schema-reconciliation step and the recorded version.

### The schema reconciliation (the one genuinely new piece)

On an upgrade, the datasource object must bring the materialized db's schema up to the new `schema.sql`
WITHOUT dropping the operator's rows. Two candidate constructions, decided here:

- **(A) Parse `schema.sql` → `DesignSchema`, reuse `plan_migrate`.** Add a raw-DDL → `DesignSchema`
  parser in core (there is none today — the schema designer builds `DesignSchema` in the UI), then feed
  it to the SAME `plan_migrate(desired, live, "sqlite")` `federation.migrate` uses, and apply the
  additive plan via `Source::apply_ddl`. Reuses the destructive-refusal + additive-planner wholesale;
  costs a DDL parser (the sharp, error-prone part — a real SQL-subset parser, or a narrow
  `CREATE TABLE` scanner like `binding.rs`'s, extended to column types).
- **(B) Idempotent re-execution.** Re-run the pack's `schema.sql` against the existing db with each
  statement made idempotent: `CREATE TABLE` → `CREATE TABLE IF NOT EXISTS`, and for each declared
  column, an `ALTER TABLE ADD COLUMN` guarded by a `pragma_table_info` existence check (sqlite has no
  `ADD COLUMN IF NOT EXISTS`). Simpler — no full DDL parser — but it does NOT get destructive refusal for
  free (a removed column just… stays, silently, which is the SAFE direction but means the upgrade can't
  WARN about a schema that dropped a column), and a retyped column is invisible to it.

**Decision: (A), reusing `plan_migrate`** — because destructive refusal (goal 3) is load-bearing and
must not be re-implemented, and because the resulting plan is the SAME shape the schema designer's
Migrate flow shows, so a `pack.validate --upgrade` dry-run can preview exactly what an upgrade will do
(the operator sees "1 ADD COLUMN: site.lat" before committing). The DDL parser is scoped to the
`CREATE TABLE (col type constraints, …)` subset packs actually ship (the same grammar `binding.rs`
already scans for table/column NAMES — this extends it to carry the column TYPE + nullability + PK, which
`DesignColumn` needs). A statement the parser can't read fails the upgrade with a clear "cannot plan an
upgrade for this schema — apply to a fresh workspace or migrate manually" rather than guessing.

### The re-apply orchestration (mostly existing)

`apply_datasource` gains an `upgrade: bool` alongside `first_apply`. On upgrade: `resolve_existing` the
db (rows preserved), then reconcile the schema additively (plan + apply the additive DDL; refuse
destructive → the object is `failed` with the refusal message), then re-register. Every other object
kind is unchanged — an upgrade re-drives them through their capability-checked seams exactly as a
re-apply does.

### The dry-run preview (`pack.validate` on an upgrade)

`pack.validate` against a workspace with a lower applied version reports `decision: "upgrade"` and, for
the datasource, the planned additive DDL (or the destructive refusal) — so CI / an operator sees the
upgrade's schema effect before `pack.apply`. Reuses the migrate planner's dry-run output shape.

## How it fits

- **Rule 10**: the upgrade path names no pack — `version > prior` and the schema diff are generic over
  any pack's data. No arm branches on `bas`.
- **Seed ownership / one-datastore**: unchanged and reinforced — an upgrade preserves rows by
  construction (`resolve_existing`), adds columns nullable, never re-seeds. The datasource stays the one
  source of truth; the operator's edits survive a version bump, which is the entire point.
- **Caps**: an upgrade re-checks every object's cap under the caller's principal (the same wall a first
  apply hits — `pack.apply` gets you in, each object re-checks). The schema reconciliation runs under the
  same authority the datasource object already needs. Embedding grants nothing extra.
- **The MCP surface**: no new verb. `decide` gains an arm; `pack.apply`/`pack.validate` gain an upgrade
  outcome. Both additive, back-compatible.

## Example flow (the exact case that motivated this)

1. `bas` v1 is applied to `acme`; an operator has CRUD'd a few sites (their rows).
2. The pack author bumps `bas` to v4: adds the entity→table bindings (manifest metadata) AND `lat`/`lng`
   columns to `site` (schema).
3. `make pack-validate PACK=bas` → `decision: upgrade (v1 → v4)`, datasource plans `2 ADD COLUMN
   (site.lat, site.lng)`, no destructive change — the gate passes.
4. `make pack-apply PACK=bas` → the upgrade runs: the site rows are PRESERVED, `lat`/`lng` are added
   (nullable, empty for existing rows), the bindings land in the new receipt, rules/dashboards
   re-author to v4. The result lists `upgraded bas: v1 → v4` + every object outcome.
5. `pack.get` now returns the v4 manifest with bindings → the rubix-ai Entities pages light up on
   `acme`, and the operator backfills `lat`/`lng` per site via the row editor (or leaves them null — a
   real BAS condition).
6. Re-running `make pack-apply PACK=bas` is a NoOp (same version, same checksum).

## Testing plan

- **Decision (unit, pure)**: `version > prior` → `Upgrade { from, to }`; `version < prior` → Refuse
  (downgrade); same-version arms unchanged. The un-broken-matrix test.
- **DDL parser (unit, pure)**: `CREATE TABLE site (id TEXT PRIMARY KEY, name TEXT NOT NULL, lat REAL)` →
  a `DesignTable` with the right columns/types/nullability/PK; an unparseable statement is a clear error,
  not a silent skip.
- **Additive upgrade (integration, real node)**: apply `bas` v1, `federation.write` a couple of sites +
  edit one, apply a v4 whose schema adds `lat`/`lng` → assert the rows SURVIVED, the columns EXIST
  (`federation.schema` shows them), the new column is null on the old rows, the receipt is v4, and rules
  did NOT re-run. The load-bearing test.
- **Destructive refusal (integration)**: a v-bump whose schema DROPS a column → the datasource object is
  `failed` with the refusal message, the receipt records the partial, and the live db is UNCHANGED (no
  column dropped, no rows lost).
- **Idempotence**: apply v4 twice → the second is a NoOp.
- **Dry-run preview**: `pack.validate` on a lower applied version reports `upgrade` + the planned DDL.
- **Downgrade still refused**: v4 applied, apply v1 → refused.

## Risks & hard problems

- **The DDL parser is the sharp edge.** A pack's `schema.sql` is author-written SQL; a parser that
  mis-reads a column type plans a wrong ALTER or refuses a valid upgrade. Scope it to the `CREATE TABLE`
  subset packs actually ship (extend `binding.rs`'s scanner), fail LOUDLY on anything outside it
  ("apply to a fresh workspace or migrate manually"), and pin it with the real `packs/*/schema.sql` as
  fixtures — never a hand-invented grammar.
- **Additive-only is a real limitation, stated honestly.** An upgrade cannot rename or retype a column,
  or backfill data. For `bas` v1 → v4 (add nullable columns) that is exactly right; for a pack that
  needs a rename, the answer is the explicit destructive/reseed verb, NOT loosening this. Do not let
  "just this once" widen the additive boundary — that is the line this scope exists to hold.
- **Partial upgrade recovery.** An upgrade that fails mid-plan (a cap deny on one object) leaves a
  partial receipt at the NEW version — the same-version re-apply arm must then re-drive the denied
  objects. Confirm the matrix handles "partial receipt at v4, re-apply v4" as a re-Apply, not a NoOp
  (it should, by the existing partial-receipt arm — but the version now matches, so test it).
- **Schema drift vs receipt.** The receipt says v4; the live db is what the reconciliation achieved. If
  an operator hand-migrated the db between applies, the diff is against the LIVE catalog (correct — the
  planner diffs live, not the last receipt), so a hand-added column is simply not re-added. State this so
  "the receipt is not the schema oracle, the live db is" stays true.

## Open questions

- **O-1:** DDL parser (A) vs idempotent re-execution (B)? Leaning A (reuses destructive refusal + gives a
  dry-run preview), but if the parser proves too broad for the packs that exist, B with an explicit
  "no destructive detection" caveat is a smaller first step. Decide when the parser is prototyped against
  the real `packs/*/schema.sql`.
- **O-2:** does an upgrade re-run the seed for BRAND-NEW tables (a table the old schema didn't have, so
  it has no operator rows to protect)? Lean yes — a table with no prior existence has no rows to
  clobber, so seeding it on upgrade is safe and matches "seed a fresh table once." Confirm against the
  run-once flag's granularity (per-datasource vs per-table).
- **O-3 (defers):** the destructive/reseed verb (`pack.migrate --destructive` / `pack.reseed`) is named
  across pack-entity-binding-scope and here; scope it on its own when an upgrade actually needs to drop or
  rename, not before.

## Related

- `docs/scope/packs/pack-core-scope.md` — the engine + the decision matrix this fills the `version >
  prior` arm of.
- `docs/scope/packs/pack-entity-binding-scope.md` — the seed-ownership re-apply rule this extends from a
  same-version re-apply to a version bump; the `binding.rs` DDL scanner the upgrade parser extends.
- `crates/federation/src/migrate.rs` + `crates/federation/src/source/dialect.rs` — the `plan_migrate`
  additive planner + destructive refusal the schema reconciliation reuses.
- `crates/host/src/pack/apply.rs` (`apply_datasource`) + `sqlite.rs` (`resolve_existing`) — the re-apply
  orchestration the upgrade extends.
- **`NubeIO/rubix-ai → docs/scope/packs/entity-data-plane-scope.md`** — the consumer that hit the wall;
  once this ships, an operator upgrades a pack and the new bindings/columns reach their entity pages
  without abandoning their workspace's data.
