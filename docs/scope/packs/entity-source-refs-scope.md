# Packs scope — entity source refs (declaring that an entity's rows also exist in a datasource)

Status: **SHIPPED (core side, NubeDev/lb#115)** — the field, the receipt carriage, the shape lint and
the `charts.source` unlock are in. Promotes to `doc-site/content/public/packs/packs.md` when that page
exists (there is no packs page under `doc-site/content/public/` yet — writing it is its own task, not
smuggled in here).

**What shipped, and where:**

| Goal | Landed in |
| --- | --- |
| 1 · the `refs:` field | `rust/crates/packs/src/manifest_refs.rs` (`EntityRef`) + `refs: Vec<EntityRef>` on `Entity` in `manifest.rs` |
| 2 · receipt carriage | nothing to write: `Receipt.manifest` carries the whole manifest, so `pack.get` returns `refs` the moment the field exists — asserted end-to-end rather than assumed |
| 3 · shape-only validation | `rust/crates/packs/src/validate_refs.rs`, called from `validate::validate` |
| 4 · `charts.source` unlock | `validate_refs::lint_chart_sources` (the only new *gate*: a store entity's chart `source` must name a declared ref) |

Both new files sit beside `manifest.rs`/`validate.rs` rather than inside them — the
`manifest_retention.rs`/`validate_retention.rs` precedent, and what `FILE-LAYOUT.md` requires
(`manifest.rs` was already at the size line).

**Tests.** Unit: `manifest_refs` (parse round-trip, the `fk` default, absent-block byte-identity, a
typo'd key) and `validate_refs` (every error above, plus the non-checks). Integration on a real node
with a real sqlite twin: `rust/crates/host/tests/pack_refs_test.rs` — receipt carriage through the
real verbs, id parity proven by reading the twin *through the address the receipt carries*, the
dangling-source gate, and the two negatives (an unregistered source neither gates nor grants). Plus
the full payoff over the **real federation sidecar** —
`a_ref_derived_federation_query_returns_the_twin_rows` applies a pack shipping both halves (a
materialized sqlite twin + the store entity refing it), takes `{source, table, fk}` off `pack.get`,
and runs the `federation.query` a downstream surface would build. **Verified live** (`rows: 2` for
`site-001`). It is `#[ignore]`d, the `pack_test.rs` O-1 precedent: the sidecar binary is not in the
default `cargo test` run. Run it with:

```
cargo build -p federation
cargo test -p lb-host --test pack_refs_test -- --ignored
```

A pack entity bound to the store (`backend: store`) often has a **twin in a federation
datasource**: the EMS pack's 8 `ems_site` rows are the SAME sites (same ids, same coordinates)
that `demo-buildings` — the seeded sqlite source — carries in its `site` table, with 6 months of
15-minute readings the store seed deliberately does not duplicate. Today that identity is
**folklore**: both artifacts are generated from one catalog, the pack README documents the parity,
and nothing at runtime declares or checks it. A dashboard author who wants a high-resolution chart
for the selected `${site}` has to *know* the sqlite `site.id` equals the entity pk. This scope adds
the smallest core change that turns the folklore into contract: an optional **`refs:` block on the
entity binding** declaring "this entity's pk is also the key in datasource X, table Y, column Z."
Like `table`/`geo`/`charts`, it is an **address, not behavior** — core stores it in the receipt,
generates no SQL, joins nothing, and validates only shape. Downstream surfaces (dashboards, maps,
rules, a parity diagnostic) read it off `pack.get` and build ordinary `federation.query` reads
parameterised by the entity variable that already exists.

## Goals

1. **Optional `refs:` list on `Entity`** (`rust/crates/packs/src/manifest.rs`): each ref is
   `{ source, table, fk?, label? }` — the datasource **name** the workspace knows it by, the table
   in that source, the column carrying this entity's pk (default: the entity's own `pk`), and an
   optional human label for pickers ("Interval readings (demo)"). Serde stays
   `deny_unknown_fields`; all fields ordinary identifiers except `source`/`label`.
2. **Carry `refs` in the receipt** so `pack.get` returns it — mechanical, the same ride
   `table`/`geo`/`charts` take. No new verb, no envelope change.
3. **Validate shape, not data**, in `pack.validate`: a ref requires the entity to be bound
   (`table` set — a shape-only vocabulary entity has no pk to ref) and `table`/`fk` must be bare
   identifiers. Whether `source` exists is a **workspace** fact, not a pack fact — so it is never a
   validate gate; an unresolvable ref at read time means "this pack offers no such source here"
   (the exact posture `charts:` takes for a missing var reference).
4. **Unlock `charts.source` on a store entity.** Today a chart recipe's `source` field is legal
   only on a `backend: datasource` entity. With refs, a **store** entity may declare a chart recipe
   whose `source` names one of its declared refs — the recipe's derive-path (`table`, `columns`,
   `kind`) then addresses the *datasource* table, and the compiled panel is an ordinary
   `federation.query` cell parameterised by `${<var>:sqlstring}`. This is the concrete payoff: EMS
   `site` can offer *☑ Interval demand · last 7 days* over demo-buildings' 15-minute data from a
   map pin, with zero hand-written SQL.

## Non-goals

- **No cross-backend join engine, no virtual views.** Core never unions store rows with datasource
  rows, never emits SQL from a ref, never proxies one backend through another. The seam that makes
  consumption seamless already ships: the entity **variable** — one selected id parameterises a
  `store.query` and a `federation.query` alike. A federated-view layer was considered and rejected:
  it is a query-federation engine in core (type mapping, pushdown, caching, caps across two
  backends arrive at once) and it breaks the settled "core stores addresses, downstream builds
  queries" line of `pack-entity-binding-scope.md`.
- **No per-row ref data.** A ref is declared once per entity, not per row; id parity is the
  contract. A domain whose external ids genuinely differ from entity pks needs a mapping *column*
  on its own rows and is out of scope until a real case shows up (stated so the optionality is
  honest, not a hidden TODO).
- **No new verb, no probe in core.** The parity check ("do the declared ids actually resolve over
  there?") is a downstream diagnostic built from `store.query` + `federation.query`, both of which
  ship — see the consumer scope. Core's job ends at carrying the address.
- **No ref-to-another-pack's-entity.** Refs point at *datasources* (workspace-registered,
  federation-reachable). Entity-to-entity linking across packs is a different ask with different
  isolation questions.

## Intent / approach

### The field (manifest)

```yaml
entities:
  site:
    label: Site
    backend: store
    table: ems_site
    pk: id
    display: name
    refs:
      - source: demo-buildings   # datasource NAME, resolved in the viewer's workspace
        table: site              # table in that source
        fk: id                   # column holding this entity's pk (default: the entity pk)
        label: Interval data (demo)
    charts:
      - key: demand-hires
        label: Interval demand
        source: demo-buildings   # legal on a store entity IFF it names a declared ref
        table: point_reading
        columns: { time: ts, value: val, entity: site_id }
        kind: demand
        window: 7d
```

`refs: Vec<EntityRef>` on `Entity`, `EntityRef { source, table, fk: Option<String>, label:
Option<String> }`. All optional at the entity level; every existing pack parses unchanged — the
optionality-is-the-safety-property rule from the binding scope holds again.

### Validation (pure, unit-tested)

Extends `validate_binding` (`rust/crates/packs/src/binding.rs`): ref present ⇒ entity has `table`
+ `pk`; `table`/`fk` are bare identifiers (the `geo:` derivation discipline — refuse, don't
quote); duplicate `{source, table}` pairs are an error. A chart recipe's `source` on a store
entity must match a declared ref's `source`, else the recipe is invalid (hard error at validate —
unlike a missing *workspace* source, a dangling in-manifest reference is the pack author's bug).

### Receipt + downstream read

`refs` rides the receipt struct next to `geo`/`charts`. Downstream resolves `source` by **name**
against the workspace's registered datasources at read time — same late binding as every
`federation.query` cell. Rule 10 holds: nothing downstream matches on a pack or entity name; it
routes on the binding fields the receipt carries, and a node whose workspace lacks the source
simply offers nothing.

## How it fits

- **Isolation/tenancy** — a ref names a datasource; whether the viewer can read it is the existing
  federation caps wall (`mcp:federation.query:call` + the approved-endpoint gate). Declaring a ref
  grants nothing.
- **Capabilities & the deny path** — a session without federation caps sees ref-backed offers
  suppressed (or the query denied by the wall) exactly as any federation cell today; store-side
  reads unchanged.
- **API/MCP surface** — none added. `pack.validate`/`pack.get` carry more data; `federation.query`
  and the vars grammar do the work.
- **Data** — core stores the manifest block in the receipt; no rows, no SQL.
- **Rule 10** — the ref is generic mediated config: any pack, any datasource kind (sqlite,
  postgres, timescale). No named-source special case anywhere in core.
- **Rule 9** — tests run against the real in-process sqlite source path (`127.0.0.1:0`
  convention) with real seeded rows; no fakes.

## Example flow

1. The EMS pack declares `site.refs: [{source: demo-buildings, table: site, fk: id}]` and the
   `demand-hires` chart recipe above; `pack.validate` passes (identifiers ok, ref'd source name
   matches the recipe).
2. `pack.apply` writes the receipt; `pack.get` now returns the ref + recipe.
3. In rubix-ai, the map inspector's pin-click checklist (reading `charts:` off the receipt) offers
   *☑ Interval demand · last 7 days*; the operator ticks it. The compiled popout panel is an
   ordinary `federation.query` cell over `demo-buildings` / `point_reading` filtered by
   `site_id = ${site:sqlstring}` — no recipe, ref, pack id or entity key survives into the saved
   map (rule 10).
4. A viewer clicks the Riverside Data Center pin: `setVars` fills `${site}` with the entity pk,
   which — because id parity is the declared contract — selects the same site's rows in the
   sqlite source. The chart renders 15-minute demand the store never stored.
5. On a node whose workspace never registered `demo-buildings`, step 3 offers no such row and a
   previously saved panel fails as any federation cell with a missing source does — honestly, at
   query time.

## Testing plan

- **Unit (packs crate):** manifest parse round-trip with/without `refs`; validate errors — ref on
  an unbound entity, non-identifier `table`/`fk`, duplicate `{source, table}`, store-entity chart
  `source` naming an undeclared ref; existing packs (no refs) parse byte-identically.
- **Integration (real node, rule 9):** apply a pack with refs against `mem://` + the in-process
  sqlite source; `pack.get` returns the block; a `federation.query` built from the ref returns the
  seeded twin rows for a known entity pk.
- **Capability-deny:** a session without `mcp:federation.query:call` gets the standard deny on the
  ref-derived read; the receipt itself (pack.get) is unaffected.
- **Workspace-isolation:** two workspaces, source registered in one — the ref resolves in one and
  yields "no such source" in the other; no cross-workspace leak through the source name.

## Risks & hard problems

- **Silent drift** — id parity is declared, not enforced; a regenerated datasource can drift from
  the store rows with no error until charts go empty. Mitigation is the downstream parity probe
  (consumer scope) + the existing `gen-seed-from-inventory.py --check` for the EMS artifacts.
  Accepted: enforcement would need core to read data, crossing the address/behavior line.
- **Name-based source resolution** — datasource names are workspace-scoped and renamable; a rename
  orphans refs. Same exposure every saved federation cell already has; not new, but refs make more
  surfaces depend on the name. A future "datasource rename updates references" ask is workspace
  tooling, not this scope.

## Open questions — both DECIDED as proposed

- **O-1 (`fk` default): DECIDED — defaults to the entity's `pk`.** The 90% case is literal id
  parity, and the default keeps manifests honest-short. Stated once, in `EntityRef::fk_or`, so no
  call site can drift from it.
- **O-2 (a `kind:` filter on a ref): DECIDED — no.** Filtering is the chart recipe's / consumer's
  job; a ref addresses a table, full stop. The EMS case that prompted the question (`point_reading`
  rows of `kind: demand`) is served by the recipe's existing `kind:` field, so adding a second
  filter would have given one concept two homes.

### Decided while implementing

- **The `charts.source` gate is scoped to `backend: store` ONLY.** A `datasource` entity's chart
  `source` is an ordinary override that predates refs, and an entity with *no* explicit backend is
  routed downstream by `datasource.engine` — gating either would fail packs that validate clean
  today for no safety gained. So the unlock adds exactly one new error, on exactly the shape it
  unlocks.
- **`table`/`fk` must be bare identifiers (`[A-Za-z_][A-Za-z0-9_]*`) — refuse, don't quote.** A
  schema-qualified `main.site` is refused rather than silently split or quoted; supporting it is a
  separate ask (it would need a dialect-aware quoter core deliberately does not own).
- **An empty `source` errors, but an *unregistered* one never does.** These look adjacent and are
  not: the first is unreadable as an address at all, the second is a workspace fact resolved late.
  `validate_refs.rs`'s module doc states that line, because it is the one a future edit is most
  likely to cross.

## Related

- `pack-entity-binding-scope.md` — the parent doctrine (binding = projection, not ORM) this
  extends; same course as `geo:`/`charts:`.
- `pack-core-scope.md` — the "entities stay vocabulary" line this respects.
- Downstream consumer: `NubeIO/rubix-ai → docs/scope/packs/entity-source-refs-ui-scope.md`
  (readers, parity probe, EMS content).
- `NubeIO/rubix-ai → packs/ems/README.md` §seed — the documented-but-undeclared id parity this
  formalises; `docker/postgres/seed-demo-sqlite.sh` — the twin source.
