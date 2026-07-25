# Session — entity-scoped option sources for `viz.query`

- **Topic:** viz (record-picker / option-source reach), the Forms-10x non-blocking lb ask.
- **Status:** code + tests green in-tree. **Needs release + tag + pin-bump** by the user
  (rubix-ai already `[patch]`es `lb-node` at this checkout, so it builds against these changes
  locally; a downstream tag bump is the durable pickup).
- **Scope origin:** `rubix-ai/docs/scope/forms/forms-10x-scope.md` → "Entity-scoped option sources".
  Sits alongside the pack-store-datasource entity-grant read path (`docs/scope/packs/`).

## The ask

A record-picker resolves choices via `viz.query`. Where the target is a store-backed entity with
entity-grant reach (EMS's `ems_site`), a raw `store.query` read does NOT honor the reach filter — only
the typed `.list` verb (`ems.site.list`) does, via `authz.scope_filter`. Let a picker `Target`
optionally **name an entity** so `viz.query` applies the same `scope_filter`. Additive and opt-in:
an enum, a tool list, or a plain store read all still work unchanged; the hint only tightens reach when
the source is an entity table. Rule 10: generic, driven by the grant/binding, never `if entity == "ems_site"`.

## What changed

An OPTIONAL `entity` hint on a `viz.query` source/target. When present, after the target dispatches,
the resolver post-filters the returned rows to the caller's entity-grant reach — the SAME
`lb_authz::scope_filter` the `.list` verb uses. Generic: `table`/`cap`/`pk` are opaque strings.

### Wire shape (the field the UI may OPTIONALLY send)

On any `sources[]` entry (or the v2 single `source`), alongside `tool`/`args`:

```json
{
  "refId": "A",
  "tool": "store.query",
  "args": { "sql": "SELECT data.id AS id, data.name AS name FROM ems_site" },
  "entity": { "table": "ems_site", "cap": "mcp:ems.site.list:call", "pk": "id" }
}
```

- `entity` — OPTIONAL. Absent ⇒ today's path byte-for-byte (no resolve, no filter).
- `entity.table` / `entity.cap` — REQUIRED when `entity` is present; empty/missing either ⇒ the hint is
  ignored (parsed to `None`, no error). `cap` is the entity's `.list` verb cap (the key the entity-grant
  is scoped under). `table` is the store table (the pack binding's `Entity.table`).
- `entity.pk` — OPTIONAL, defaults to `"id"` (the pack binding's `Entity.pk`); the row column compared
  against the reachable id set.

There is no serde struct on the wire — the hint is parsed leniently from the panel `Value`
(`EntityReach::from_value`), so existing panels/records with no `entity` key deserialize and resolve
exactly as before. (The in-memory `ResolvedTarget.entity` is `Option<EntityReach>`.)

### Semantics (the tightening lens)

- `scope_filter` = `All` (caller holds the cap with full reach — admin/unscoped) ⇒ rows UNCHANGED.
- `scope_filter` = `Ids(reachable)` ⇒ keep only rows whose `pk` value ∈ `reachable`.
- Cap not held / scoped to a different table ⇒ `Ids([])` ⇒ empty (a tech with no sites sees none).
- **Clean degradation:** if the result carries no `pk` column at all (hint attached to a non-entity
  target), rows pass through unchanged — inert, never an error, never a silent blank.
- **Fails CLOSED** on a store-read error while resolving reach (empty, not a leak).
- Reach is read from the REAL store (live grants), never the token — a just-revoked grant bites here.

## Files

- `rust/crates/host/src/viz/reach.rs` — **NEW.** `EntityReach` (parse + fields) and
  `apply_entity_reach` (the `scope_filter` post-filter). One responsibility, ~135 lines.
- `rust/crates/host/src/viz/mod.rs` — register `mod reach;` + doc line.
- `rust/crates/host/src/viz/query.rs` — `ResolvedTarget` gains `entity: Option<EntityReach>`; parsed in
  `panel_targets` for both `sources[]` and the v2 `source`; applied in `dispatch_target`'s success arm
  BEFORE the `ok`/`empty` status is derived (a zero-reach read honestly reads `empty`).
- `rust/crates/host/tests/viz_query_entity_reach_test.rs` — **NEW.** 4 real-infra tests (below).

No change to any core mediation chokepoint; no branch on an entity id (rule 10 held — `ems_site` appears
only as a test fixture and doc example).

## Tests (real embedded node, `mem://` store, real grants + caps — no mocks)

`cargo test -p lb-host --test viz_query_entity_reach_test`:

```
running 4 tests
test entity_hint_all_reach_passes_through ... ok
test no_entity_hint_is_unchanged ... ok
test hint_on_non_entity_result_degrades_cleanly ... ok
test entity_hint_tightens_to_reachable_rows ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.36s
```

- `entity_hint_tightens_to_reachable_rows` — tech scoped `ems_site:[north]` → only North via a raw
  `store.query` + hint; South filtered.
- `entity_hint_all_reach_passes_through` — supervisor with `Scope::All` on the list cap → both rows.
- `no_entity_hint_is_unchanged` — SAME panel, no hint → both rows despite the scoped grant (opt-in).
- `hint_on_non_entity_result_degrades_cleanly` — hint on a `SELECT name` (no pk column) → both rows,
  no error.

Regression (no existing behavior moved):
`cargo test -p lb-host --test viz_query_test --test viz_resolution_test`
→ **17 + 6 passed, 0 failed** (`viz_query_acceleration_test` is `page-cache`-gated → 0 ran here).
`cargo fmt -p lb-host --check` clean.
(`make build-wasm` run first, per the Makefile — the node reads the guest `.wasm` at boot.)

## Notes / caveats

- The filter is a **post-read reach lens** (tool-agnostic): it works for a `store.query`, a
  `federation.query`, or even the typed `.list` verb (harmless double-check). It does NOT push
  `id IN [...]` into the SQL, so the underlying read still fetches up to its own cap
  (`store.query` = `MAX_QUERY_ROWS`) before filtering — fine for a bounded picker/option source.
- Because the picker must SELECT the `pk` column for the hint to bite, a scoped caller who queries the
  entity table WITHOUT selecting `pk` bypasses the lens (the "no pk column ⇒ passthrough" degrade). The
  picker always selects the id it needs, so this is the intended advisory posture, documented in
  `reach.rs`. Hard enforcement still lives in the typed `.list` verb.
- `page-cache` fingerprint: `entity` does not change the dispatched tool list, so
  `panel_target_tools` / the subject-scoped fingerprint are unaffected. The result is now
  subject-reach-dependent; the subject-scoped cache is already keyed per subject.

## Downstream (rubix-ai UI) — how to send it, degrading cleanly

The record-picker builder may add `entity: { table, cap, pk }` to the option-source target it already
posts to `viz.query`. Omit it and nothing changes. The three fields come from the pack entity binding
the picker targets (`Entity.table`, `Entity.pk`, and the entity's `.list` cap). No SDK/WIT change.
