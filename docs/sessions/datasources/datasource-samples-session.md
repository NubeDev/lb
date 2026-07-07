# Session — `federation.sample`: one AI-ready snapshot of a datasource

Scope: `docs/scope/datasources/datasource-samples-scope.md`
Status: **in-progress** (code + tests written this session; flip to done when merged)

## What was built

One new MCP verb, `federation.sample {source, tables?, limit?}` → a single bounded JSON snapshot
(tables + columns + real foreign keys + up to `limit` sample rows per table) for feeding a model
before it writes SQL. It rides `federation.schema`'s exact gated pipeline and the same
`mcp:federation.query:call` cap (no new grant).

### Sidecar (`rust/extensions/federation/`)

- `src/sample.rs` — `run_sample`: one `connect` per snapshot; per table best-effort (an unreadable
  table is skipped, `info_schema.rs` stance); bounds `MAX_TABLES = 25` (+ `truncated: true`),
  `limit` clamped 1..=50 (default 10); rows read plan-level (`ctx.table(...).limit(...)`, no SQL
  string → no ident quoting); string cells > 256 chars truncated; columns whose name contains
  `password|secret|token|api_key|apikey` emitted as `«redacted»` (NULLs stay NULL — nullness is
  honest signal).
- `src/source/mod.rs` — `ForeignKeyMeta` + `Source::foreign_keys(table)` with a **default `Ok([])`**
  impl (best-effort by contract; additive, so third-party kinds don't break).
- `src/source/sqlite.rs` — FK read via `pragma_foreign_key_list` through a direct read-only
  `rusqlite` connection in `spawn_blocking` (the DataFusion provider path can't express PRAGMA;
  the retained path is the DSN and never reaches a message). A NULL `to` (implicit-PK FK) reports
  the conventional `id`.
- `src/source/postgres.rs` — FK read via the shared `catalog_rows` runner over the three
  `information_schema` FK views, `unwrap_or_default()` → empty on any failure (feature-gated build;
  untested here — no Docker/TLS toolchain).
- `src/query.rs` — `shape` / `catalog_rows` widened to `pub(crate)` for reuse.
- `Cargo.toml` — `rusqlite 0.37 bundled` (same version/features as the host's dev-dep, so the one
  `links = sqlite3` in the workspace graph stays unified).
- `main.rs` dispatch arm + `extension.toml` `[[tools]]` entry.

### Host (`rust/crates/host/`)

- `src/federation/sample.rs` — the gated verb (authorize under the **query** cap → resolve in the
  caller's ws → `net:*` → mediate DSN → one sidecar call) + `sample_descriptor()` (real arg schema,
  `x-lb entity: datasource`); host-side limit clamp too (defense in depth).
- `src/federation/{mod,tool}.rs` — export + MCP bridge arm.
- `src/tool_call.rs` — `federation.sample` added to the `federation.schema` gate alias (gates on
  `mcp:federation.query:call`; without this the outer gate would demand a `federation.sample` cap
  no role carries — the exact bug schema hit before).
- `src/tools/descriptor.rs` — descriptor registered in the palette/agent catalog.

## Decisions (per the scope's open questions — "whatever is best long term")

- **Fixed redaction denylist** (not per-datasource config) — add a record field only when asked.
- **Natural-order sampling** — `TABLESAMPLE`/random left as follow-up.
- **ERD stays on inference** — switching `SchemaErd` to real FKs is a separate UI slice.

## Tests

`rust/crates/host/tests/federation_sqlite_test.rs` extended (real store, real sidecar, real seeded
`.db` — no fakes): fixture gains a real FK (`point_reading.site_id REFERENCES site(id)`), 12 rows
(> default limit), and a `login(username, password)` table. New assertions: snapshot covers the
tables; default limit samples exactly 10 of 12 rows; the FK appears in `relationships`; the
`password` cell is `«redacted»` (and `hunter2` absent); the DSN path appears nowhere; `tables`
filter + `limit:1` honored; **capability-deny** (opaque) and **workspace-isolation** for the new
verb. Existing query round-trip assertion updated 3 → 12 rows.

Run: `cd rust && cargo test -p lb-host --test federation_sqlite_test`

## Docs

- Scope: `docs/scope/datasources/datasource-samples-scope.md` (written this session, indexed in
  `docs/scope/README.md`).
- Skill: `docs/skills/datasources/SKILL.md` — verb table row + a §4 snapshot recipe with a live-run
  curl.

## Slice 2 (same day): visible in the palette + renders in the channel, minimized

Live testing surfaced two gaps:

1. **The palette hid the verb.** `tools.catalog` gated each descriptor on the RAW tool name, but
   `federation.sample`/`federation.schema` gate on the `federation.query` cap via the dispatcher's
   alias — so the catalog demanded per-verb caps no role carries and hid both (breaking its own
   "never hide a tool a call would pass" rule; pre-existing for schema). Fix: the alias moved into
   ONE shared `tool_call::gate_tool_for()` (also covering `outbox.enqueue_held`, `telemetry.*`,
   `nav.pref.*`, `nav.set_default`) consumed by BOTH the dispatcher gate and the catalog.
   Regression test: `tools_catalog_test.rs::catalog_shows_alias_gated_federation_verbs…`.
2. **The result vanished.** The palette's plain-bridge path (`useChannel.callTool`) fires `mcp_call`
   and discards the result — the API ran, nothing appeared in chat. Fix (the reminder.list pattern,
   zero tool knowledge in the UI): `sample_descriptor` now DECLARES a `result` render envelope
   `{v:2, view:"jsonview", source:{tool:"federation.sample"}, options:{collapsed:true}}`, so the
   palette posts a durable, re-runnable `rich_result` item. `JsonView` was widened to read a
   non-flow TOOL source through the one `usePanelData` hook (flow path untouched), gained
   `options.collapsed` (starts minimized — a snapshot can be big; the collapsed line summarizes
   `{tables[6], relationships[6], truncated}`) and a keys-with-sizes summarizer. That durable item
   is exactly what the agent **context basket** paperclips onto the next ask (agent-context-basket
   scope; note the basket fence truncates at 8 KB — use `tables`/`limit` args for a focused attach).

Verified live over the restarted dev node: catalog serves the envelope; `viz.query` over a
`federation.sample` source returns the snapshot as one object row (real FKs from `demo-buildings`).
Green: `tools_catalog_test`, `federation_sqlite_test`, UI views unit tests,
`ResponseViewResultRender.gateway`, `jsonViewMappings.gateway`, palette gateway tests.

## Slice 3 (same day): the basket fences the snapshot's DATA, not its render envelope

Live test: the user basketed the snapshot card and asked "avg energy usage across all buildings" —
the agent ignored the attachment and re-probed the source over six tool calls. Root cause: a
`rich_result` item's durable body is the render ENVELOPE (`{v,view,source:{tool,args}}`) — a
pointer, not data — and `fence_items_into_goal` fenced it verbatim.

Fix (`channel/context_items.rs`): the fence now **dereferences** a `rich_result` ref — it re-runs
the declared `source:{tool,args}` through the one MCP dispatcher **under the poster's principal**
(the identical cap-checked call the card's own render makes; can never widen) and fences the tool
RESULT, naming the tool in the fence line (`…; result of federation.sample`). A denied/failed
re-run falls back to the raw envelope (honest). Dereferenced bodies get a larger 32 KB ceiling
(`MAX_DEREF_BYTES` — host-produced data from a gated verb; raw bodies keep the 8 KB posture).
`fence_items_into_goal` now takes `(node, poster)`; the worker passes its reconstructed poster.

Tests (12 green in `channel::context_items`): deref-fences-the-result (real node + real
`datasource.list`), cap-deny (posterless caps → raw envelope, no data), plus the existing
ws-isolation/cross-channel/caps suite migrated to the new signature.

- Postgres FK read is best-effort and **unverified against a live Postgres** (feature-gated build).
- UI "Copy AI context" button on `DatasourceDetail` (scope example flow step 2) — separate slice.
- ERD real-FK upgrade — separate slice.
