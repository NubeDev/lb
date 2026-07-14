# MCP scope — confirmed wire shapes for the host verbs ems provisioning depends on

Status: **confirmed** 2026-07-14. Answers [NubeDev/lb#48](https://github.com/NubeDev/lb/issues/48)
(the six `rules.*`/`series.latest`/`authz.*`/`grants.*` contracts) and
[NubeDev/lb#60](https://github.com/NubeDev/lb/issues/60) (the `series.read` raw/bucketed/paged
contracts, added below). No lb-core code change — the verbs already exist and are exercised green;
this doc is the **authoritative contract** an out-of-tree extension (ems, in `rubix-ai-extensions`)
cites so it can drop its assumed request/reply shapes. Two of #48's six assumptions were **wrong**
and are corrected below; one of #60's field-name assumptions (bucket row) was partly wrong.

> Read with: `mcp-scope.md` §"The contract" (the `<ext>.<tool>` dispatch pipeline + `authorize_tool`
> gate), `../extensions/host-callback-scope.md` (the `SidecarClient` → `POST /mcp/call` path a native
> sidecar calls the host back on — rule 10), `../auth-caps/authz-verbs-mcp-dispatch-scope.md` (why
> `grants.*` is reachable over the one bridge), `../auth-caps/entity-scoped-grants-scope.md`
> (`Scope::Ids`), `../ingest/ingest-scope.md` (`series.latest` + the `Sample` envelope),
> `../rules/rules-scope.md` (the `rules.*` CRUD verbs).

---

## The ask (issue #48)

ems milestone-04's production `CallbackProvisioner`
(`rubix-ai-extensions:.../ems/src/provisioning/callback.rs`) reaches lb-core host verbs over the generic
MCP callback (rule 10) using **assumed** request/reply shapes that had never been exercised against a
running node. The issue asks lb-core to (1) confirm or correct the reply field names and (2) confirm
the verbs are callable by a native sidecar over `/mcp/call` with the caps ems requests.

## Callability — yes, all six, over the one bridge

Every verb below is a **host-native MCP verb** dispatched by `lb_host::call_tool`
(`rust/crates/host/src/tool_call.rs`) by name prefix, gated by the standard workspace-first
`authorize_tool` chokepoint (`mcp:<tool>:call`). A native Tier-2 sidecar reaches them exactly as the UI
bridge does — `POST /mcp/call` under its own intersected `granted = requested ∩ admin_approved`
authority. No extension-id branch, no special-casing (rule 10). So: **callable, provided the sidecar
holds the matching cap** — with one correction to the cap names ems requests (see `rules.*` below).

## Confirmed contracts (each backed by a real green test)

| verb | request | reply | verified by |
|---|---|---|---|
| `rules.save` | `{ id \| name, name?, body, params? }` | `{ "id": "<id>" }` | `crates/host/tests/rules_test.rs:126` |
| `rules.delete` | `{ id }` | `{ "ok": true }` | `crates/host/src/rules/mod.rs:245` |
| `series.latest` | `{ series }` | `{ "sample": Sample \| null }` | `crates/host/tests/ingest_test.rs:91` |
| `authz.check_scoped` | `{ cap, table, id, subject? }` | `{ "allowed": bool }` | `crates/host/tests/authz_scoped_test.rs:83` |
| `authz.scope_filter` | `{ cap, table, subject? }` | `{ "filter": "all" }` \| `{ "filter": { "ids": [...] } }` | `crates/host/tests/authz_scoped_test.rs:185,306` |
| `grants.assign` / `grants.revoke` | `{ subject, cap, scope? }` | `{ "ok": true }` | `crates/host/src/authz/tool.rs:28,41` |

## Corrections ems must make (2 of 6 assumptions were wrong)

1. **`rules.create` does not exist.** ems assumed `rules.create { name, body } → { rule_id }`. The
   verb is **`rules.save`**, keyed by `id` (falling back to `name`), and it replies **`{ id }`**, not
   `{ rule_id }`. The cap is **`mcp:rules.save:call`**, not `mcp:rules.create:call` — the requested cap
   would grant nothing. `rules.save` is an **upsert** (idempotent by id), which is what a provisioner
   wants anyway.

2. **`rules.delete` takes `id`, not `rule_id`**, and replies `{ ok: true }` (ems assumed "any 2xx" —
   fine, but read the arg name right). Pass back the `id` you saved.

3. **`series.latest` replies `{ sample }`, not `{ value, ts }`.** The reply is
   `{ "sample": Sample | null }` where `Sample` is the canonical ingest envelope
   (`crates/ingest/src/sample.rs`): `{ series, producer, seq, ts, payload, labels? }`. The reading ems
   wants is **`sample.payload`** (any SurrealDB-typed value — a scalar for a simple meter) and the
   timestamp is **`sample.ts`**. `sample == null` ⇒ "no committed sample yet" (ems's "not-fresh, not an
   error" reading is correct — treat null as stale). Note "latest" is by `seq` (monotonic), not
   wall-clock `ts` (ingest scope).

4. **`authz.check_scoped` → `{ allowed: bool }`** — ✅ matches ems exactly.

5. **`authz.scope_filter` → `{ filter: "all" }` or `{ filter: { ids: [...] } }`** — ✅ matches exactly.

6. **`grants.assign` / `grants.revoke`** with `scope: { kind: "ids", table, ids }` → `{ ok: true }` —
   ✅ matches exactly. `Scope` is `#[serde(tag = "kind", rename_all = "lowercase")]`
   (`crates/authz/src/scope.rs`), so the `Ids { table, ids }` variant is on the wire as
   `{ "kind": "ids", "table": "...", "ids": [...] }` — omit `scope` entirely for an unscoped (all-rows)
   grant. Both are gated by the single admin cap `mcp:grants.assign:call` (revoke reuses assign's cap);
   the inner grammar gate (`authz/grants.rs`) enforces no-widening.

## Net for ems

The `authz.*` / `grants.*` half (scaffolded earlier from `site_reach/`) was **correct as assumed** — no
change. The `rules.*` / `series.latest` half (new in milestone 04) was **wrong**: swap
`rules.create`→`rules.save` (+ cap + `id` field), read `rules.delete`'s arg as `id`, and read
`series.latest` out of `sample.payload` / `sample.ts` instead of top-level `value` / `ts`. All are
one-file edits in `provisioning/callback.rs` — no lb-core change, exactly as the issue predicted.

## `series.read` — confirmed wire shapes (issue #60)

Status: **confirmed** 2026-07-14. Answers [NubeDev/lb#60](https://github.com/NubeDev/lb/issues/60).
#56 (keyset paging) and #57 (bucketed decimation) landed together in
`23fae1eb` with no shape write-up; this pins the contract ems's
`fetch-history.ts` should code directly against. No lb-core code change — the
verb already exists and is exercised green.

`series.read` is **one verb, one cap (`mcp:series.read:call`), three shapes** selected by an optional
top-level `mode` field (default `"rows"`): `rows` (raw/windowed, keyset-paged) and `buckets`
(server-side decimation). Dispatch: `crates/host/src/ingest/tool.rs:43-50` → `read_rows` /
`read_buckets_mode` (`tool.rs:124-180`) → `crates/ingest/src/page.rs` / `crates/ingest/src/bucket.rs`.

| form | request | reply | verified by |
|---|---|---|---|
| raw/windowed read | `{ series, from?, to?, from_seq?, to_seq?, limit?, cursor?, direction? }` | `{ "samples": [Sample...], "next_cursor": string \| null, "prev_cursor": string \| null }` | `crates/host/tests/series_plane_host_test.rs:82` (`windowed_read_is_half_open_via_mcp`) |
| bucketed/decimated read | `{ series, mode: "buckets", from, to, width_ms? \| budget? }` | `{ "buckets": [Bucket...], "width_ms": u64 }` | `crates/host/tests/series_plane_host_test.rs:127` (`bucketed_read_via_mcp_and_deny_without_cap`) |
| keyset paging | `cursor?` (opaque string, from a prior reply's `next_cursor`) | `next_cursor`/`prev_cursor` are `Option<String>`; `null` (not absent) signals end-of-range | `crates/host/tests/series_plane_host_test.rs:63` (`paged_read_walks_chain_via_mcp`) |

### 1. Raw/windowed read (`mode` omitted or `"rows"`)

A row is the **full canonical `Sample` envelope** (`crates/ingest/src/sample.rs:37-58`), not a
projected `{ ts, value }`:

```
{ series, producer, seq: u64, ts: u64, payload: Value, labels?: Value, qos? }
```

The value field is **`payload`** (any JSON value), same as `series.latest` — **not** `value`. `ts` is
**epoch ms as a `u64` number**, not a datetime string.

`from`/`to` (wall-clock, epoch ms) bound the window **half-open `[from, to)`** — `from` is inclusive
(`>=`), `to` is **exclusive** (`<`), confirmed at `crates/ingest/src/page.rs:88-94`. By contrast,
`from_seq`/`to_seq` (the legacy monotonic bounds) are **inclusive on both ends** (`>=`/`<=`,
`page.rs:80-86`) — an asymmetry worth calling out explicitly since it's easy to assume both bound
forms behave the same way. All bounds are optional; an omitted bound is open (never encode "no
bound" as `u64::MAX` — it silently coerces to float and the comparison mis-evaluates, see
`docs/debugging/ingest/u64-max-bound-coerces-to-float.md`).

### 2. Bucketed/decimated read (`mode: "buckets"`)

Selected by an **explicit `mode: "buckets"` string**, not by param presence. Requires `from` and `to`
(both mandatory epoch ms — `BadInput` if either is missing) plus either `width_ms` (explicit bucket
width) or `budget` (target bucket count; width is derived, capped at `MAX_BUCKETS = 2000`).

Bucket row shape (`crates/ingest/src/bucket.rs:29-44`, wire fields only):

```
{ t: u64, min: f64 | null, max: f64 | null, avg: f64 | null, last: Value, count: u64 }
```

**ems's assumption was wrong on the timestamp field name and the null behavior:**

- The bucket timestamp is **`t`**, not `ts` — epoch ms, floor-aligned to `width_ms`.
- `min`/`max`/`avg` are **`Option<f64>`, `null` when the bucket has zero numeric payloads** — not
  guaranteed non-null. `count` (total samples in the bucket, numeric or not) and `last` (raw payload
  of the chronologically-last sample by `(ts, seq)`) are always present.
- **Empty buckets are omitted from the array entirely** (never emitted with null fields) — a bucket
  with zero samples in the whole window never appears in `buckets`.
- The reply also echoes **`width_ms`** at the top level (the *effective* width lb-core picked), which
  matters when the caller sent `budget` instead of an explicit width.

### 3. Keyset paging (`mode: "rows"`, cursor-driven)

Request cursor param: **`cursor`** (opaque base64 string, `crates/ingest/src/cursor.rs:16-48`,
encodes `(seq, producer)` — the actual sort/seek key, **not an offset**). Also accepts `direction`
(`"back"` or default forward) and `limit` (default/cap `DEFAULT_PAGE_LIMIT`/`MAX_PAGE_LIMIT =
10_000`).

Response cursor fields: **`next_cursor`** and **`prev_cursor`**, both present in every reply as
`Option<String>`. **"No more pages" is signalled by `next_cursor: null`** (field present, JSON
`null`) — **not** by the field being absent — confirmed at `crates/ingest/src/page.rs:130`:
`next_cursor` is `Some` only when the page came back exactly `limit`-full. A malformed or
foreign-workspace cursor decodes to nothing (empty page), never an error or a cross-workspace leak —
proven by `ws_b_replaying_ws_a_cursor_sees_nothing` (`series_plane_host_test.rs:121`).

### Net for ems

`ems`'s assumed bucket row `{ t, min, max, avg, last }` was **half right**: `t` and `last` are
correct; `min`/`max`/`avg` must be read as **nullable**, not assumed-numeric, and a bucket with
zero samples never appears in the array at all (don't render a gap as a zero — it's simply absent).
The raw-read row shape (`Sample`, `payload` not `value`) was previously undocumented for `series.read`
specifically (only `series.latest` was pinned, in #48) — it's the same envelope, confirmed here.
Paging was already correctly assumed to be keyset-based; the concrete field names (`cursor` in,
`next_cursor`/`prev_cursor` out, `null` = done) are now pinned.

**No contract gaps found** — all three forms are wired, capability-gated under the single
`mcp:series.read:call`, and workspace-isolated (mandatory deny + isolation tests cover all three
shapes in `series_plane_host_test.rs`). Nothing here required new behavior; this section documents
and tests the existing dispatch.

## Open questions

None. All six §48 contracts plus the three `series.read` forms (§60) are pinned to a passing test or
the dispatch source. If ems later needs a create-only (non-upsert) rule verb or a top-level scalar on
`series.latest`, that is a **new** lb-core scope, not a correction to this one.
