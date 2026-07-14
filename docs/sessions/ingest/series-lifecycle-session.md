# Session — series lifecycle: `series.delete` + `series.rename`

Status: shipped (backend). UI left untouched by request — the Ingest page's `SeriesRail`
already supports `onRename`/`onRemove` hover actions, so wiring the buttons is a later,
purely-frontend step. This session added the **backend verbs, routes, caps, and tests** only.

## The ask

> for the ingest, can we have a delete and a way to rename [a series].

## What a series owns (why this isn't a one-table delete)

A series named `X` denormalizes its name across the whole data plane, all workspace-scoped:

| Store | on delete `X` | on rename `X → Y` |
|---|---|---|
| `series` (sample rows, `series` field) | delete rows where `series = X` | set `series = Y` where `series = X` |
| `series_rollup` (rollup tiers) | delete where `series = X` | set `series = Y` where `series = X` |
| `ingest_staging` (not-yet-committed) | delete where `sample.series = X` | set `sample.series = Y` |
| `series_meta` (registry row, id = name) | delete row `X` | delete `X`, upsert `Y` (carry `labels_applied`) |
| tag graph (`series:X` entity) | delete `tagged` edges where `in = series:X` | re-point `in` **and `ent`** to `series:Y` |
| retention policies (prefix-keyed) | **left alone** (a prefix may cover other series) | **left alone** |

The **`ent` gotcha**: `tags.find` returns a *denormalized* `ent` string stored on each edge (verbatim,
so a dotted id round-trips without backtick-escaping — see
`debugging/tags/relation-drops-key-value-fields.md`). Updating only the `in` record link left
`series.find` returning the *old* name — the rename must rewrite `ent` too. A test caught this
(`rename_carries_samples_and_tags`).

## No silent merge (rename)

The dedup identity is `(series, producer, seq)`. Rewriting `X`'s rows into an already-populated `Y`
could collide two logical samples under one key. So `rename_series` **refuses** a target that already
exists (registered OR holding rows) — surfaced as `BadInput` (a client error about the target), not
`Denied`. `from == to` is refused as `Unchanged`.

## Shape (one verb per file, FILE-LAYOUT)

- `lb-ingest`: `delete.rs` → `delete_series`; `rename.rs` → `rename_series` + `RenameError`. Raw
  verbs, no auth (the host is the cap chokepoint).
- `lb-host` `ingest/`: `delete.rs` → `series_delete`; `rename.rs` → `series_rename` (maps
  `RenameError::{TargetExists,Unchanged}` → `IngestError::BadInput`). Both gate first via
  `authorize_ingest` (workspace-first §3.6, then `mcp:<verb>:call` §3.5). Wired into `tool.rs`
  (`series.delete` / `series.rename` MCP arms) and re-exported from `lib.rs`.
- Caps: minted `mcp:series.delete:call` + `mcp:series.rename:call`, granted **only** to the admin/owner
  role, alongside `series.retention.*` — destroying/renaming a whole series (across every producer's
  history) is workspace-data administration, never an author privilege.
- Gateway (`role/gateway`): `DELETE /series/{series}` + `POST /series/{series}/rename` (body `{ to }`),
  registered in `server.rs`, each re-running the host gate server-side; ws + principal from the token.

## Tests (real store, no mocks — rule 9)

`crates/host/tests/series_lifecycle_test.rs`, all green:
- `delete_removes_samples_and_tag_edges` — full-footprint delete (samples gone, delisted,
  `series.find` no longer returns it).
- `delete_unknown_series_is_ok` — idempotent no-op.
- `rename_carries_samples_and_tags` — samples + tag edge move to the new name; old name emptied.
- `rename_into_occupied_name_is_refused` — merge guard (`BadInput`), both series intact.
- **`delete_and_rename_denied_without_cap`** — mandatory capability-deny (a writer lacking the
  destructive caps is `Denied`; the series survives).
- **`ws_b_cannot_delete_or_rename_ws_a_series`** — mandatory workspace-isolation (gate 1 fires before
  the cap; ws-A series untouched).

`cargo test -p lb-ingest -p lb-host` is green (the only failing binary is the pre-existing
`agent_decision_test`, which needs a `hello_ext.wasm` artifact not built in this environment —
unrelated to this change).

## Follow-up (not done here, by request)

- **UI**: wire `SeriesRail`'s `onRename` (inline editor) + `onRemove` (trash + caller-owned confirm) to
  new `renameSeries`/`deleteSeries` API verbs; the `__schema.<name>` meta-series must be deleted/renamed
  alongside the real series (it is itself just another series behind the same verbs).
