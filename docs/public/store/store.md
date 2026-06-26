# Store (as built)

The datastore is **embedded SurrealDB**, the one source of truth on every node (§6.1). It holds
**state** only; motion is the bus's job (§3.3). Promoted/updated after S2 (the `list` verb landed
with messaging).

## Tenancy mapping

**Workspace = SurrealDB namespace.** A `Store` handle is opened once; every operation selects the
namespace from `ws` before the query runs (`use_ns(ws).use_db("main")`). Isolation is therefore
**structural**: a query for workspace A physically cannot read namespace B's records (§7).

## Verbs (shipped)

| Verb | Query | Notes |
|---|---|---|
| `read(store, ws, table, id)` | `SELECT data FROM ONLY type::thing($tb,$id)` | `None` if absent in *this* namespace. |
| `write(store, ws, table, id, value)` | `UPSERT … CONTENT { data: $data }` | upsert; idempotent on id. |
| `list(store, ws, table, field, value)` | `SELECT data FROM type::table($tb) WHERE data.<field> = $value` | a pure **filter**; does not order. |

## Conventions

- **Host JSON is wrapped under a `data` field.** The host speaks `serde_json::Value`; SurrealDB
  has its own value model. Wrapping under a single concrete `data` field bridges them cleanly and
  dodges the `Value` ↔ enum-tag mismatch (`debugging/store/content-rejects-serde-json-value.md`).
- **`list` does not order.** The generic store has no business knowing where a record's order key
  lives inside the opaque `data`, and SurrealDB rejects `ORDER BY data.ts` when only `data` is
  projected (`debugging/store/order-by-needs-selected-idiom.md`). The caller that owns the record
  shape orders (e.g. the inbox sorts by `ts`).
- **`field` in `list` is guarded** to `[a-z0-9_]` — it is interpolated into the query, so it must
  be a code-supplied column identifier, never caller input; `value` is always a bound param.

## Engine

S2 uses the in-memory engine (`mem://`); a file/rocksdb engine is the same handle type by config
(symmetric nodes — the engine is config, not code).

## Not yet built

A file-backed engine profile, schema/migrations, and operational tuning (per-workspace quotas).
