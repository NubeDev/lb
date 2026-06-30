# Channels — in-channel SQL query with auto-plotted charts (session)

- Date: 2026-06-29
- Scope: ../../scope/channels/channels-query-charts-scope.md
- Stage: post-S8 (channels surface; builds on the shipped `federation.query` verb)
- Status: done

## Goal

A channel member posts a SQL query into a channel; a host worker runs it through the existing
`federation.query` capability, persists the result as a durable channel `Item`, and the UI auto-plots
a chart. The query, the runner, and the result live in one ordered, auditable channel history. Exit
gate: post a `kind:"query"` item → a `kind:"query_result"` item appears in history AND streams over
SSE, with both grants (channel `pub`, then the datasource grant) checked host-side and the deny path
opaque.

## What changed

**Kind-tagged payloads** (`crates/host/src/channel/payload.rs`) — no `Item` schema migration: a
`kind` key rides INSIDE the existing `body` JSON. `ItemPayload` is an internally-tagged enum
(`query` / `query_result` / `query_error`); a body with no recognized `kind` (or non-JSON) is chat.
`parse_payload` / `result_body` / `error_body` are the one owner of the envelope.

**Chart picker** (`crates/host/src/channel/chart.rs`) — pure, host-computed into the result payload so
every subscriber renders identically. Rule: temporal x → line; categorical x + numeric → bar; single
numeric column, many rows → histogram; nothing plottable → `chart:null` (table-only). Column types
are inferred from the row values, conservative (fail safe to table).

**Inline query worker** (`crates/host/src/channel/query_worker.rs`) — runs INLINE in `channel::post`
(`post.rs`) when the posted item is `kind:"query"` (one item → one execution, idempotent by
construction; no bus-redelivery dedup). It runs `federation.query` **under the poster's principal**
(so a member without the datasource grant is denied here), caps the result (500 rows / 256 KB,
`truncated` flag), picks the chart, and posts a `query_result` / `query_error` item back under the
`system:query-worker` identity via the shared `deliver` (no `pub` re-gate — it IS the host posting).

- **Re-entrancy guard:** only `kind:"query"` triggers work — the worker's own result/error items
  parse to a different variant and return early. Tested.
- **Opaque deny:** a missing grant AND a missing source both collapse to "query not permitted" in
  `federation_error_message` — no source-existence leak. A bad SELECT stays an honest, distinct
  message.

`post.rs::post` now takes `&Node` (so the inline worker can reach `federation.query`); the gateway
`POST /channels/{cid}/messages` route already calls `lb_host::post`, so the worker runs end to end
over HTTP with no route change.

**Chart-row zip fix** (`query_worker.rs::keyed_rows`, this final session) — `federation.query`
returns `rows` as column-aligned **arrays** (`[[c0,c1,…],…]`), but `pick_chart` keys cells by column
**name** (`row.get(col)`). The worker now zips a keyed view of the (already-capped) rows *just for
the picker*; the persisted payload keeps the compact array rows (the UI maps a chart series' field
name back to its column index). Without this, the happy-path round-trip showed every `query_result`
coming back `chart:null` — the deny-path test never reached real rows, so the bug hid until the
sqlite happy-path test ran. See Debugging.

**UI**: result items render as cards in the channel view — chart-first with a ⊞ table toggle,
`chart:null` → table-only, a "showing first N rows" caption when `truncated`, an inline human error
on `query_error`. Shared TS types mirror the Rust payload + chart shapes. (Built this session — see
the file list / test section.)

## Decisions & alternatives

- **Chose** the inline worker in `channel::post` over a separate bus-subscribed worker (scope open
  question, "lean inline"). Inline is idempotent by construction (one post → one execution) and needs
  no dedup key; a bus subscriber would need redelivery-dedup. Rejected: the subscriber (more
  decoupled but more moving parts for no gain in this slice).
- **Chose** `kind` as a key inside the existing `body` payload over a new top-level `Item` field — no
  schema migration, purely additive, untagged bodies stay chat. Rejected: an `Item` migration (the
  scope explicitly preferred avoiding it; the body envelope carries `kind` cleanly).
- **Chose** running the worker under the **poster's** principal (not the system identity) for the
  `federation.query` call, so the datasource grant is the poster's — a member with channel `pub` but
  no datasource grant is correctly denied. The system identity is used only to *post the result item*
  back (the host answering).
- **Chose** host-computed chart in the payload over per-client chart derivation — every subscriber
  renders the identical chart, and the rule lives in one unit-tested file.

## Tests

Mandatory categories that apply: **capability-deny** (sub-without-pub post denied at the channel
gate; pub-without-datasource-grant → opaque `query_error`; non-SELECT rejected host-side via the
federation path) and **workspace-isolation** (ws-B can't post into / read a ws-A channel; a ws-A
source name from ws-B resolves to nothing). Plus the chart-picker unit (table-driven fixed row-sets,
in `chart.rs`), the payload round-trip unit (`payload.rs`), the cap unit (`query_worker.rs`), and the
real-gateway integration. The **deny-path** round-trip lives in `gateway_routes_test.rs`
(`posting_a_query_item_without_the_grant_…`); the **happy-path** round-trip (this session) is the new
`gateway_query_test.rs`: it builds the sqlite-only federation sidecar
(`FEDERATION_NO_POSTGRES=1 cargo build --release -p federation`), seeds a REAL on-disk SQLite file
with real rows (a `daily(day, signups)` table via `rusqlite` bundled — no Docker/TLS toolchain),
installs the sidecar + `datasource.add` of the sqlite source, posts a `kind:"query"` item over the
real gateway, and asserts the `query_result` item carries `columns:["day","signups"]`, 4 rows, and a
**non-null line chart**, AND that the result **streams live over SSE**. A second test covers the
**workspace-isolation** query path: a ws-B poster naming a ws-A source resolves to nothing → an
opaque `query_error`, and no ws-A column/value ever appears in ws-B history. Both SKIP cleanly (with
a message) if the sidecar binary is absent. UI `CommandPalette.gateway.test.tsx` renders the query
chip → result table + chart; `chart:null` → table-only; `query_error` → inline error.

Green output (backend):

```
$ cargo test -p lb-role-gateway --test gateway_query_test --test gateway_routes_test
running 2 tests
test ws_b_cannot_query_a_ws_a_source_name ... ok
test posting_a_query_item_round_trips_a_result_with_columns_rows_and_chart ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.35s
running 8 tests
test the_sse_stream_without_a_token_is_401 ... ok
test mcp_catalog_is_capability_filtered_over_http ... ok
test the_sse_stream_authenticates_by_query_token_and_pushes_a_live_message ... ok
test posting_a_query_item_without_the_grant_round_trips_an_opaque_query_error ... ok
test mcp_catalog_returns_ws_and_tools_for_a_holder_and_403s_without_the_cap ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s

$ cargo test -p lb-host --lib       # incl. channel::query_worker::keyed_rows_… regression
test result: ok. 57 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s

$ cargo test -p lb-role-gateway      # whole gateway suite green (datasources, rules, prefs, …)
test result: ok. (all gateway integration binaries pass)

$ cargo build --workspace            # Finished `dev` profile … in 1m 05s
$ cargo fmt                          # clean, no diff
```

Pre-existing, unrelated failures (NOT this slice): `lb-host`'s `fleet_monitor_test` (needs a
prebuilt `fleet-monitor` binary) and `github_bridge_normalize_test` (missing prebuilt `.wasm`) —
both fail only because an artifact isn't built, same class as noted in the task.

Green output (UI — from the real spawned gateway harness):

```
$ npx tsc --noEmit            # exit 0, clean
$ pnpm test                   # Test Files 24 passed (24), Tests 167 passed (167)  (incl. parsePalette.test.ts ×11)
$ pnpm test:gateway           # Test Files 39 passed (39), Tests 175 passed (175)
                              # CommandPalette.gateway.test.tsx ×6: catalog one-fetch + 0ms open,
                              # capability-filtered (two seeded principals), structured kind:"query"
                              # round-trip, query_result → table+chart, chart:null → table-only,
                              # query_error → inline alert — all against the REAL gateway, no fakes.
```

## Debugging

[channels/query-result-missing-truncated-field.md](../../debugging/channels/query-result-missing-truncated-field.md)
— every untruncated `query_result` silently parsed back as **chat** because
`QueryResultPayload::truncated` had `skip_serializing_if` (drops the `false` case from the wire) with
no matching `#[serde(default)]`, so the reader rejected the omitted field and `parse_payload`
swallowed it to `None`. Fixed by adding `#[serde(default)]`; round-trip regression tests
fail-before/pass-after.

[channels/query-result-chart-always-null.md](../../debugging/channels/query-result-chart-always-null.md)
— every `query_result` came back `chart:null` even for a clean temporal/numeric shape. Root cause:
`federation.query` returns column-aligned **array** rows but `pick_chart` keys cells by column name,
so the picker matched nothing. Fixed with `query_worker::keyed_rows` (zip arrays→objects for the
picker only); regression unit test `keyed_rows_zips_arrays_into_objects_so_the_picker_plots` asserts
the raw arrays do NOT plot while the keyed rows DO, and the new happy-path gateway test asserts a
real non-null line chart end to end.

## Public / scope updates

Promoted to `docs/public/channels/channels.md` (the kind-tagged payload contract, the chart spec, the
inline-worker flow). Scope open questions resolved: worker trigger = inline in `channel.post`;
row/byte cap = 500 rows / 256 KB with a `truncated` flag; `kind` lives as a key inside the existing
payload (no migration); chart payload `{ type, x, series[], bins? }` (shared TS type); no host-side
`/`-text parsing (the UI builds the structured payload).

## Dead ends / surprises

The `truncated` serde asymmetry (see Debugging) was caught by the worker's own round-trip unit test
before any integration ran — a good argument for the unit test living next to the payload type.

## Follow-ups

- A query timeout surfaced as `query_error` relies on the federation sidecar's own timeout — confirm
  it exists when the sidecar build is exercised (the integration test skips when it can't build).
- Cross-channel result sharing / pinning and a chart editor are explicit non-goals (later dashboards
  scope).
- STATUS.md updated.
