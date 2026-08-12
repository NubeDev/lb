# Viz scope — ONE Grafana-compatible SQL macro layer, query-time and engine-aware

Status: **IMPLEMENTED (2026-07-29, unreleased — needs the next `node-v*` tag)** — session
[`../../sessions/viz/sql-time-macros-session.md`](../../sessions/viz/sql-time-macros-session.md).
All goals landed in one slice (expansion table + `resolution` attach + translator deletion), full
test plan green incl. the real-SQLite integration + deny/isolation re-asserts. One addendum: the
host value pass gained a function-form-aware `replace_bare` so bare `$__timeFrom` and the call form
`$__timeFrom()` compose (see session). Originally: scope (the ask). Decisions final — no open
questions. Consumer half: rubix-ai
`docs/scope/frontend/dashboard/quick-chart-builder-scope.md` (the no-SQL Quick Chart builder —
its datasource-track time bucketing is gated on this). Promotes to
`doc-site/content/public/datasources/` beside the resolution contract once shipped.

Today lb has **two half-macro layers that don't add up to one**. Panel-resolution v1 (shipped)
substitutes the *value* macros — `$__interval` / `$__interval_ms` / `$__timeFrom` / `$__timeTo`
in `host/src/viz/macros.rs` — but the author still hand-writes the bucketing function, and
`date_bin` only works on Postgres/DataFusion; SQLite needs `strftime` math, Timescale wants
`time_bucket`, MySQL differs again. Separately, the Grafana **import** translator
(`host/src/dashboard/grafana/macros.rs`) parses `$__timeFilter(col)` / `$__timeGroup(col,'5m')`
but bakes them into Postgres-flavoured SQL **once, at import**, and punts on non-literal
intervals. This scope replaces both partial answers with **one layer**: the Grafana SQL macro
set, expanded **at query time, per engine**, in the federation child. A no-SQL builder emits one
engine-agnostic query; a migrating Grafana user's SQL panels run **verbatim, live, and
zoom-coarsening** — which is the whole point of being Grafana-compatible. The import-time SQL
translation is **retired**, not kept as a fallback: we are pre-production, and two macro layers
is exactly the long-term debt this scope exists to avoid.

## Goals

1. **The Grafana SQL macro set, query-time.** Expanded per engine in the federation child:
   - `$__timeFilter(col)` → the engine's range predicate over the derived window.
   - `$__timeGroup(col, '<interval>')` → the engine's bucketing expression
     (Timescale `time_bucket`; Postgres/DataFusion `date_bin`; SQLite epoch-floor via
     `strftime`/integer math; MySQL `FROM_UNIXTIME(FLOOR(...))`).
   - `$__timeGroupAlias(col, '<interval>')` → the same, `AS "time"` appended (Grafana-verbatim).
   - `$__time(col)` → the engine's `col AS "time"` epoch/timestamp aliasing form.
- `$__timeFrom()` / `$__timeTo()` (function forms) → window bound literals.
   - `$__timeTable('raw', 'hourly:1h', 'daily:1d', …)` → the literal **table name** of the
     best-fitting tier, selected from the same derived `width_ms`. Variadic, ordered finest →
     coarsest; each arg is a table optionally tagged `:width` with its native resolution (a bare
     name = width 0 = always the finest). Engine-agnostic — expands to a name, no dialect.
    The already-shipped value macros (`$__interval`, `$__interval_ms`, bare `$__timeFrom`/
    `$__timeTo`) stay as they are — the two passes compose (see Intent).
2. **Grafana migration just works.** An imported Grafana SQL panel keeps its macros byte-for-byte
   (`dashboard.import` passes them through), resolves against whatever engine the mapped
   datasource actually is, and re-coarsens on zoom via the shipped resolution derivation — a
   *live* panel, not a frozen Postgres translation.
3. **One emission for no-SQL clients.** rubix-ai's Quick Chart builder emits
   `SELECT $__timeGroup(ts, '$__interval') …  WHERE $__timeFilter(ts)` and is correct on every
   supported engine, with the interval auto-derived and cache-key-stable.
4. **Retire the import-time SQL translator.** `host/src/dashboard/grafana/macros.rs` is deleted;
   `dashboard.import` stops rewriting target SQL. One parser, one expansion, one owner.
5. **The un-macro'd invariant holds.** SQL containing no `$__` token is byte-identical after
   every pass — a hand-written tile never changes behavior.

## Non-goals

- **Not v1.5 structured `decimate`.** That deferred slice (typed `{decimate: …}` on
  `federation.query`, guaranteed spike-safe min/max envelope) remains its own ask; this scope is
  the textual-macro layer and neither blocks nor replaces it. A `$__timeGroup` SELECT returns
  whatever the author's aggregates yield (the panel-resolution "v1 ACCEPTED" posture).
- No general SQL parsing or rewriting — expansion is function-token substitution with
  balanced-paren argument capture, the discipline the import translator already proved.
- No SurrealQL macros: the native store's dialect is singular and known client-side
  (`time::floor`), and the series path has native buckets (`series.read mode:"buckets"`).
  Macros exist to hide *plural* engines.
- No exotic Grafana macros (`$__unixEpochFilter`, `$__unixEpochGroup`, per-plugin oddities) in
  v1 — the set above covers Grafana's documented common SQL-datasource macros; anything outside
  it fails with a clear "unsupported macro" error naming the token (never silent SQL breakage).

## Intent / approach

**Decision — where expansion lives: the federation child.** The child is the only place the
executing dialect is truly known (direct-connect vs DataFusion today; single-source pushdown
re-routes there too — `federation-pushdown-scope.md`), and it already keys pools on the source
`kind` string. The host stays zero-parse. Rejected: host-side expansion (needs a per-target
datasource-record lookup and is still wrong under routing changes); client-side per-engine
emission (forks the dialect table into every client — web, RN, AI authors — and leaves imported
Grafana macros dead). The server owns engine truth.

**Decision — how the resolution reaches the child: an additive `resolution` field.**
`viz.query` already derives `{from, to, width_ms}` per target; it now also attaches it to the
dispatched `federation.query` args as an additive, optional
`resolution: {from_ms, to_ms, width_ms}` — no new verb, old callers unaffected. The child uses
it to expand `$__timeFilter` / `$__timeFrom()` / `$__timeTo()` and to resolve a `'$__interval'`
interval argument. A direct `federation.query` call containing a time macro but no `resolution`
fails with a clear error naming the missing field — never a guess.

**Decision — `$__timeTable` tier selection.** Walk the args **coarsest → finest** (reverse of the
documented finest → coarsest order) and return the first tier whose native width is ≤ the derived
`width_ms` — the coarsest table still resolving at least as fine as the chart needs (least data
scanned without losing resolution); if none qualifies, fall back to the coarsest given. Because a
bare `raw` has width 0, it always qualifies and is selected last — so the one-arg form is always
`raw`, any range. This is the same "governing tier" rule lb's ingest rollup uses internally, now
reachable from hand-authored federation SQL.

> **Known consequence — flag, not a bug.** Selection is width-driven, not range-length-driven, so it
> depends on both the range **and** the panel's point budget (default ~1000). A 1-year range at the
> default budget derives to a ~12h width, which picks a *finer* tier (e.g. `hourly`) — not the
> intuitive "1 year → yearly". This is deliberate: it keeps `$__timeTable` and `$__interval` on the
> **same derivation**, so the pick and the bucket width can never disagree. Authors choosing
> year-tables over fewer points should raise the point budget (coarser derived width), not expect the
> macro to guess intent from range length.

**Decision — pass order.** Host value pass first (shipped, unchanged): `$__interval` → `'12h'`,
bare `$__timeFrom`/`$__timeTo` → epoch ms. Then dispatch; the child expands the function macros.
So `$__timeGroup(ts, '$__interval')` reaches the child as `$__timeGroup(ts, '12h')` — the
literal form — and a hand-written Grafana `$__timeGroup(ts, '5m')` is already in that form.
One grammar at the child: `$__name(args…)` with a literal-duration interval.

**Decision — parser/expander placement: `federation/src/sql_macros.rs`**, one pure module, one
responsibility (token scan + balanced-paren args + the per-engine expansion table). With the
import translator deleted there is exactly one consumer, so no shared crate is warranted; the
pure functions are unit-tested in place. The engine table is keyed on the child's own `kind`
vocabulary inside the federation extension — the mediated seam, no special case in core
(rule 10).

**Decision — import becomes pass-through.** `dashboard.import`'s macro translation step is
removed; imported target SQL is stored verbatim. Panels whose macros fall outside the v1 set
surface the "unsupported macro" error at first render — honest, visible, fixable — instead of
being silently mistranslated at import. Pre-production, no migration shim: any dashboard
imported under the old translator is simply re-imported.

**Decision — `$__timeFilter` comparison form.** v1 assumes a native timestamp column per engine
(the import translator's `to_timestamp` shape as the Postgres baseline) — documented per engine
in the expansion table. Epoch-integer columns use the explicit epoch macros later if a real
migration needs them (additive; out of v1).

## How it fits the core

- **Tenancy / capabilities: nothing new.** Expansion happens inside the already-gated
  `federation.query` dispatch under the caller's grant; deny paths unchanged.
- **MCP surface: additive only.** No new verb; one additive optional field
  (`federation.query.resolution`) written by `viz.query` and usable by direct callers.
- **Rule 10:** the engine table lives inside the federation extension keyed on its own `kind`;
  core knows no engine names.
- **Symmetric nodes / placement:** pure child code plus a one-line host attach; no `if cloud`.
- **SDK/WIT impact: none.**
- **One responsibility per file:** `federation/src/sql_macros.rs` (new);
  `host/src/viz/query.rs` gains the `resolution` attach; `host/src/viz/macros.rs` unchanged;
  `host/src/dashboard/grafana/macros.rs` deleted.
- **Coordination:** touches `federation/src/query.rs` — same file as the paging/pushdown line of
  work; land as one focused slice.

## Example flow

1. In rubix-ai's Quick Chart builder a user picks the `demo-buildings` SQLite datasource, "avg of
   power_kw", "group by device", look "Area". The emitter writes one engine-agnostic SQL:
   `SELECT $__timeGroup(ts, '$__interval') AS time, device, avg(power_kw) AS power FROM
   energy_samples WHERE $__timeFilter(ts) GROUP BY 1, 2 ORDER BY 1`.
2. `viz.query` derives the resolution for the visible range (30 d → `1h`), value-substitutes the
   interval, and dispatches `federation.query` with
   `resolution: {from_ms, to_ms, width_ms: 3_600_000}` attached.
3. The child resolves `demo-buildings` → `kind: "sqlite"`, expands both function macros with the
   SQLite forms, executes; ~720 bucketed rows return. The user zooms to 6 h — the derivation
   yields finer buckets, the expansion follows, no client change.
4. The user repoints the same panel at a Timescale copy of the data: identical stored SQL, the
   child now expands `time_bucket`. No panel edit.
5. A migrating Grafana user imports their dashboard: `WHERE $__timeFilter(ts) GROUP BY
   $__timeGroup(ts,'5m')` is stored verbatim and runs live against the mapped datasource; a
   panel using an unsupported macro renders a clear "unsupported macro `$__foo`" status.
6. **Deny unchanged:** without the `federation.query` grant the target fails exactly as today.

## Testing plan

Real infra, seeded data, no mocks (`scope/testing/testing-scope.md`).

- **Expansion unit table:** every engine × every macro in the set × several intervals;
  balanced-paren edges (nested parens in `col` expressions); a time macro with no `resolution` →
  the named error; an unsupported `$__foo(...)` → the named error; un-macro'd SQL byte-identical.
- **Integration (real sources):** the SQLite demo source returns correctly bucketed rows through
  the full `viz.query` path with a derived interval; zoom changes bucket width (the
  panel-resolution invariant re-asserted through function macros). Postgres/Timescale behind the
  existing gated live-verification pattern (`federation-cache-live-verification-scope.md`).
- **Import pass-through:** a Grafana SQL panel with `$__timeFilter`/`$__timeGroup` imports with
  macros byte-identical in the stored target and executes live; the deleted translator's test
  cases are re-homed as expansion-table cases so no covered macro regresses.
- **Capability-deny + workspace-isolation** re-asserted through a macro'd target (mandatory).

## Risks & hard problems

- **Timestamp column typing per engine** (native timestamp vs epoch integer) — v1's
  native-timestamp assumption is documented per engine; a mismatch surfaces as an engine error
  on the diagnosed path (`query-diagnostics-scope.md`), and the epoch macros are the named
  additive follow-up if a real migration hits it.
- **DataFusion vs direct-connect routing** — expansion must match the dialect that *executes*;
  the child expands after routing decides, not before.
- **Release discipline:** translator deletion and query-time expansion land in the **same**
  `node-v*` release so there is never a build with zero macro handling; rubix-ai bumps the pin
  once.

## Related

- `panel-resolution-scope.md` — the shipped value-macro pass + derivation this composes with;
  its deferred v1.5 (structured `decimate`) is the sibling, not superseded.
- `../datasources/federation-pushdown-scope.md` (routing owns dialect) ·
  `../datasources/sqlite-datasource-demo-scope.md` (the test source) ·
  `../datasources/query-diagnostics-scope.md` (the error surface) ·
  `grafana-parity-backend-scope.md` (import pin; its macro-translation step is retired here).
- Downstream consumer: rubix-ai `frontend/dashboard/quick-chart-builder-scope.md` — emits the
  macro form from the no-SQL builder; bumps the `node-v*` pin when this ships.
