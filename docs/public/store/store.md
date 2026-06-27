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

## Engine — two backends, chosen by config (S9)

Every node compiles in **both** embedded engines; which constructor runs is a **config** decision at
boot, never a role code-branch (symmetric nodes):

| Constructor | Engine | When |
|---|---|---|
| `Store::memory()` | `Mem` (in-memory) | tests / dev — ephemeral, gone on drop |
| `Store::open(path)` | **SurrealKV** (on-disk) | a real node — **durable across restart** |

Boot wiring (`Node::open_store`): `LB_STORE_PATH` set → `open(path)`; unset → `memory()`. No
`if cloud`. Everything above the open seam (`read`/`write`/`list`/`write_tx`) is unchanged.

**Engine pinned: SurrealKV** — pure-Rust, no C++ toolchain (the "builds anywhere / on a Pi" posture).
The choice is by the three-axis rule (crash-consistency vetoes → feature coverage → build footprint);
all LOAD-BEARING features are available and the crash set passes, so RocksDB (the documented fallback)
was not needed.

A raw `query_ws(ws, sql, bindings)` escape hatch runs `RELATE`/`DEFINE`/composite-id/multi-statement
statements (used by ingest + tags); it selects the namespace from `ws` first, the same hard wall.

## Durability + crash-consistency (S9)

Proven by a subprocess crash set (`crates/store/tests/crash_test.rs`, SIGABRT — not a graceful drop):
write→drop→reopen present; **kill mid-`write_tx` → rolled back** (not half-applied); **kill during a
flush burst → last committed survives**; reopen after an unclean kill recovers, never corrupt. At-rest
encryption (when added) is **node-level**, whole-store (§6.7 protects secret *values*, not the file).

## Capability spike matrix (the GO/NO-GO deliverable)

A permanent hermetic CI test (`crates/store/tests/capability_spike_test.rs`) defines + exercises each
feature the S9 scopes assume and classifies it. LOAD-BEARING ✗ would be NO-GO (the test fails);
DEGRADABLE ✗ is recorded with its fallback. Result on SurrealKV (surreal 2.6):

| Feature | Class | Result / fallback |
|---|---|---|
| Durability across restart | LOAD-BEARING | ✓ |
| Composite/array record IDs | LOAD-BEARING | ✓ |
| `RELATE` edges with properties | LOAD-BEARING | ✓ |
| Namespace isolation on disk | LOAD-BEARING | ✓ |
| Multi-statement transactions | LOAD-BEARING | ✓ |
| `DEFINE BUCKET` / file storage | DEGRADABLE | ✗ → ingest binary payloads use record-as-content |
| `SEARCH` / BM25 full-text | DEGRADABLE | ✓ → tags full-text ships |
| `HNSW` vector | DEGRADABLE | ✓ → tags vector ships |
| `DEFINE TABLE … AS SELECT … GROUP` | DEGRADABLE | defines ✓ but **does not populate** → tag_counts per-query |
| `LIVE SELECT` | DEGRADABLE | ✓ (unused; motion rides Zenoh) |

## Not yet built

Schema/migrations, operational tuning (per-workspace quotas), at-rest encryption mechanism.
