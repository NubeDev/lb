# Session — entity `refs:` + the `charts.source` unlock (NubeDev/lb#115)

**Date:** 2026-07-29. **Scope:** `docs/scope/packs/entity-source-refs-scope.md` (this repo, the owning
side). Downstream consumer: `NubeIO/rubix-ai → docs/scope/packs/entity-source-refs-ui-scope.md`
(readers, the parity probe, EMS content) — **not** touched here.

The ask in one line: a store-backed pack entity often has a twin in a federation datasource (EMS's 8
`ems_site` rows ARE `demo-buildings`' `site` rows), that identity was folklore, and this makes it
declarable — as an **address**, never behavior.

## What landed

| Goal | Where |
| --- | --- |
| 1 · the field | `packs/src/manifest_refs.rs` (`EntityRef {source, table, fk?, label?}`) + `refs: Vec<EntityRef>` on `Entity` |
| 2 · receipt carriage | **no code** — see below |
| 3 · shape lint | `packs/src/validate_refs.rs`, called from `validate::validate` |
| 4 · `charts.source` unlock | `validate_refs::lint_chart_sources` |

Tests: `packs` unit (68 green) + `crates/host/tests/pack_refs_test.rs` (3 default + 1 `#[ignore]`d
sidecar test). `pack_test`/`pack_store_test` unchanged and green; fmt + clippy clean.

## The three decisions worth recording

**Goal 2 needed no code, and that is the finding.** `Receipt.manifest` already carries the WHOLE
manifest (`receipt.rs`), so `refs` rode to `pack.get` the instant the field existed. The temptation was
to add a projection anyway; the honest move was to write the assertion instead of the code — the
integration test reads `refs` back off the real `pack.get` rather than trusting the reasoning. Same for
`skip_serializing_if`: an absent block must not materialize `refs: []` into the receipt (a consumer
distinguishing "no refs" from "empty refs" would break), which is asserted directly.

**Both new files sit BESIDE their parents, not inside them.** `manifest.rs` was already at the
`FILE-LAYOUT.md` size line, and `manifest_retention.rs`/`validate_retention.rs` had set the precedent
exactly one scope earlier. The scope text said "extends `validate_binding`" — followed in spirit
(same lint pass, same `Finding` vocabulary), not in file placement, because `binding.rs` is a *DDL-vs-
binding* checker and refs have no DDL to check against. Mixing them would have put two oracles in one
file.

**The one line the lint must never cross, and it is subtle.** An empty `source` errors; an
*unregistered* one never does. They look adjacent and are not: the first is unreadable as an address,
the second is a **workspace** fact resolved late — datasources are registered per workspace, and every
saved `federation.query` cell already resolves its source by name at read time. Gating on it would
refuse a valid pack on every node but the one it was authored against. This is stated in
`validate_refs.rs`'s module doc because it is the check a future edit is most likely to "helpfully"
add.

## Scoping the unlock so it breaks nothing

The `charts.source` gate fires **only** on `backend: store`. On a `datasource` entity, `source` is an
ordinary override that predates refs; on an entity with *no* explicit backend, routing is decided
downstream by `datasource.engine`. Gating either would fail packs that validate clean today for zero
safety gained. So goal 4 adds exactly one new error, on exactly the shape it unlocks — the rest of the
lint's findings are structural defects readable from the manifest alone (the dangling-parent class,
hence errors not warnings).

`table`/`fk` are held to bare identifiers (`[A-Za-z_][A-Za-z0-9_]*`) — the `geo:` derivation
discipline: refuse, don't quote. A schema-qualified `main.site` is refused rather than split or quoted;
supporting it needs a dialect-aware quoter core deliberately does not own.

## Proving it, rather than asserting it

The default-run integration test proves id parity **through the address the receipt carries**: it takes
`{table, fk}` off `pack.get` and selects with it against a real sqlite twin. That is the claim refs
make, and it was previously only a README sentence.

The full payoff needed the real federation sidecar, which is not in the default `cargo test` run
(`pack_store_test.rs` module doc). So `a_ref_derived_federation_query_returns_the_twin_rows` is
`#[ignore]`d, following `pack_test.rs`'s O-1 precedent — and it was **run for real** during this
session: `rows: 2` for `site-001`, over a pack shipping both halves (materialized sqlite twin + store
entity refing it). Reproduce with `cargo build -p federation` then
`cargo test -p lb-host --test pack_refs_test -- --ignored`.

The negative matters as much: `a_ref_to_an_unregistered_source_neither_gates_nor_grants` asserts that a
principal holding no federation cap is still denied `federation.query` after declaring a ref.
**Declaring a ref grants nothing** — otherwise the block would be a caps bypass wearing a manifest
field's clothes.

## Open questions — both closed as proposed

- **O-1 `fk` default** → the entity's `pk`. Stated once, in `EntityRef::fk_or`, so no call site drifts.
- **O-2 a `kind:` filter on a ref** → no. The chart recipe already has `kind:`; a second filter would
  give one concept two homes.

## What is NOT done here (deliberately)

- The **downstream consumer**: map-inspector offers, the compiled `federation.query` panel, the parity
  probe, and the EMS pack actually declaring its refs. That is rubix-ai's scope and its own issue.
- The **doc-site promotion**. There is no `doc-site/content/public/packs/` page at all yet — writing
  the packs public doc is its own task, not something to smuggle in under this one.
