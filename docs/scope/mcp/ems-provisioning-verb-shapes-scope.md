# MCP scope — confirmed wire shapes for the host verbs ems provisioning depends on

Status: **confirmed** 2026-07-14 (branch `confirm-ems-provisioning-verb-shapes`). Answers
[NubeDev/lb#48](https://github.com/NubeDev/lb/issues/48). No lb-core code change — the verbs already
exist and are exercised green; this doc is the **authoritative contract** an out-of-tree extension
(ems, in `rubix-ai-extensions`) cites so it can drop its assumed request/reply shapes. Two of the six
ems assumptions were **wrong** and are corrected below.

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

## Open questions

None. All six contracts are pinned to a passing test or the dispatch source. If ems later needs a
create-only (non-upsert) rule verb or a top-level scalar on `series.latest`, that is a **new** lb-core
scope, not a correction to this one.
