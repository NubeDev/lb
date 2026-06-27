# Audit scope — the immutable, workspace-walled audit ledger

Status: scope (the ask). Promotes to `public/audit/` once shipped. Stage: **S10 —
cross-cutting retrofit** (`../../STAGES.md`). The capture chokepoint (host dispatch + capability
check, README §6.5/§6.6) ships on every node from S1; the **durable record of those decisions** was
never scoped. `key-stack.md` row "Observability/audit" flags it; README §6.14 already promises a
*model-call* audit — this **generalizes that promise to every mediated action**.

> Read with: README §6.6 (the cap decision this records), §6.5 (the dispatch point), §6.7 (secrets —
> store references/digests, never values), §6.8 (sync — audit is the ideal **append-style** shared
> data), §6.14 (the AI-gateway audit this subsumes), `../observability/observability-scope.md` and
> `../undo/undo-scope.md` (the two sibling projections of the same chokepoint —
> see "The shared seam" in the observability scope), `lb-outbox`'s `write_tx` seam (reused here).

A **capability-first** platform can *enforce* who may do what — but it keeps **no durable, tamper-
evident record** of what was actually attempted, allowed, or denied. You can stop an unauthorized
action; you cannot later *prove* it was stopped, investigate a breach, satisfy a compliance ask, or
answer "who deleted that workspace, and were they allowed to?" This scope adds the **audit ledger**:
one immutable, append-only, workspace-walled record emitted at the same chokepoint that already
makes the security decision — capturing **both allow and deny** (the deny half is the security-
interesting one: an attempted access is itself the signal).

## Goals

- **One canonical `AuditEntry` per mediated action**, written at the host dispatch/cap chokepoint:
  `{ seq, ws, actor (principal id), tool, params_digest, decision: allow|deny, reason, ts, node,
  trace_id, prev_hash, hash }`. Captures **every** call — host service or WASM guest — because every
  call is dispatched here; a guest **cannot opt out of being audited** (the same property that makes
  it capability-checked here).
- **Immutable + append-only.** No `update`, no `delete` MCP verb exists; the host append is the
  *only* writer (like tags' `DEFINE EVENT` — host-internal, no caller-facing write to forge). A
  workspace can read its ledger, never alter it.
- **Tamper-evident via a per-(ws, node) hash chain.** Each entry's `hash = H(prev_hash ‖ canonical
  payload)`; deleting or editing any entry breaks the chain from that point, detectably. The chain is
  per **origin** (ws + node) so it needs **no cross-node ordering** (which the sync model can't cheaply
  give); the hub keeps the union of chains and can verify each.
- **Records references, never secrets or raw payloads.** `params_digest` is a hash + a redacted shape
  summary (the helper shared with observability); secret material is `Secret<T>` and never reaches the
  entry (§6.7). The ledger proves *that* `secret.request` happened and by whom — never the value.
- **As durable as the action it records.** For an **allow that mutates state**, the audit entry is
  written in the **same `write_tx`** as the domain change (the outbox's existing one-tx seam) — the
  change and its proof commit together or not at all. For a **deny** (no domain change), it is a
  standalone append. An action can never be durable while its audit record is lost.
- **Workspace-walled, plus a system ledger for cross-tenant admin.** Tenant actions live in the
  workspace's audit table; super-admin/cross-workspace actions (purge a workspace, rotate a key) also
  append to a reserved `_lb_audit_system` ledger (node-level, like the workflow directory's reserved
  namespace) so the hard wall is never the *gap* in the audit trail.

## Non-goals

- **Not operational telemetry.** The audit ledger is complete, immutable, and durable; it is **not**
  sampled spans/metrics — that is `../observability/`. A deny appears in both: a (droppable) metric
  there, an (immutable, complete) ledger entry here. Different guarantees, deliberately not merged.
- **Not the undo journal.** Audit records *that* an action happened (a digest); it stores **no
  before-image** and drives **no reversal** — that is `../undo/`. Same capture point, different store,
  different retention, different immutability (the undo journal is bounded and prunable; the ledger is
  WORM-style).
- **No SIEM / alerting / anomaly engine in core.** The platform produces a trustworthy ledger and a
  read/export surface; correlation rules and alerting are an external consumer (export the ledger, or
  tail `audit.watch`). Don't reinvent a security analytics product.
- **No per-keystroke / payload-content capture.** The ledger is action-grained (one entry per tool
  call), not a data-diff log. (Data deltas are undo's before-images; content history is a per-asset
  concern.)
- **No bypass of the capability wall via DB row-permissions.** Tenancy is the host cap gate +
  namespace, same as everywhere; an append-only `DEFINE TABLE` constraint is belt-and-braces, never
  *the* wall.

## Intent / approach

**Append at the chokepoint the cap check already runs in.** The host dispatch function decides
allow/deny once; immediately after that decision it appends an `AuditEntry`. Because dispatch is the
*only* path to any tool (§6.5), the ledger is complete by construction — there is no second code path
that could act without auditing, the same reason capability-first works. **Completeness is therefore
a property of the architecture, not of discipline** — and the single biggest reason this had to be a
platform primitive, not a per-extension add-on.

**Same transaction for the mutating allow; standalone append for the deny.** Reuse the
`lb_store::write_tx` seam the outbox introduced: a mutating tool's domain write *and* its audit row
land in one `BEGIN…COMMIT`. A deny mutated nothing, so its entry is a lone append (still chained).
This gives the ledger exactly the outbox's durability property — committed-together-or-not-at-all —
without inventing a second durability mechanism.

**Tamper-evidence by hash chain, scoped to avoid distributed ordering.** A single global total order
across nodes is exactly what the §6.8 authority-partitioned sync deliberately does *not* provide. So
the chain is **per (ws, node)**: monotonic `seq` + `prev_hash` within one origin, which a single node
trivially orders. Audit rows are perfect **append-style shared data** (§6.8) — they never conflict,
never need last-writer-wins — so they sync to the hub cleanly, and the hub verifies each origin chain
independently and holds the union. **Rejected:** a cross-node Merkle tree / global chain — it would
demand the global ordering the sync model rejects, for marginal benefit over per-origin chains.

**Reading the audit is itself audited.** `audit.query`/`audit.list` require a high-privilege grant
(`mcp:audit.read`) and — because they are mediated tool calls — append their own `decision=allow`
entry. Nobody reads the ledger invisibly. (Termination is trivial: a read appends one entry; it does
not read-then-append-then-read.)

**Why generalize §6.14's gateway audit rather than keep it special.** The gateway already must record
"actor, workspace, tool/workflow source, input refs, output dest, token/cost, approval checkpoint."
That is an `AuditEntry` with model-specific fields. Making model-call audit a **typed extension of the
one ledger** (a richer `params_digest`/`meta`) means one query surface, one retention policy, one
tamper guarantee — not two parallel audit systems. **Rejected:** a separate gateway audit table —
it splinters the trust story and the query surface.

## How it fits the core

- **Tenancy / isolation:** every entry carries `ws`; the workspace ledger lives in the workspace
  namespace; a ws-B `audit.query` physically cannot reach ws-A entries (structural wall, §7).
  Cross-tenant super-admin actions append to the reserved `_lb_audit_system` ledger so privileged
  actions *above* the wall are still recorded — the wall is never an audit blind spot.
- **Capabilities:** **read** is gated by `mcp:audit.read` (high privilege; opaque deny — no
  existence leak). **Write** has *no* grant because there is *no caller write verb* — only the host
  appends. The thing most worth auditing (the cap decision) is recorded for allow *and* deny.
- **Placement:** *either* — every node appends locally (an offline `appliance` keeps an intact local
  chain); entries sync to the hub as append-style shared data. No `if cloud`; "where the ledger is
  read/aggregated" is config (the hub), not a code branch.
- **MCP surface:** read-only by design (SCOPE-WRITTING §6.1 — ship only the verbs with a caller):
  `audit.query(filter, paging)` and `audit.get(seq)` (gated reads); an optional `audit.watch` live
  feed for a security console (bus, fire-and-forget tail of *new* entries — the durable ledger is the
  source of truth, the tail is convenience). **No create/update/delete verbs** — append is host-only;
  immutability is the point. `audit.verify(ws, node)` re-walks a chain and reports the first break.
- **Data (SurrealDB):** an `audit` table per workspace namespace (rows `audit:{seq}` with `prev_hash`/
  `hash`), plus the reserved `_lb_audit_system` ledger. **State** — the durable source of truth (the
  one place audit data *does* live in the store, unlike observability). `DEFINE TABLE` permissions
  pinned append-only as defense-in-depth.
- **Bus (Zenoh):** only the optional `audit.watch` tail — **fire-and-forget** new-entry notification
  (§6.2). Audit must **never** depend on the bus for durability; the store row is the truth. (Audit
  itself is not a must-deliver *outbox* effect — it is the local-durable record; the outbox is for
  *external* effects.)
- **Sync / authority:** the headline fit — audit rows are append-only `(table, id)` records that ride
  the existing §6.8 path with **zero conflict risk** (never updated → no last-writer-wins). The
  per-(ws,node) chain makes offline auditing safe: an edge accumulates an intact local chain offline
  and it merges into the hub union on reconnect (the offline/sync mandatory category).
- **Secrets:** the discipline — `params_digest` is a hash + redacted summary; `Secret<T>` never
  reaches an entry. A `secret.request` is audited (who/what/when), never the value (§6.7).

## Example flow

1. An edge user calls `workspace.purge` (a destructive, cross-tenant-ish admin action). Dispatch
   checks the cap: **allow**. In **one `write_tx`**, the workspace tombstone *and* an `AuditEntry
   { ws, actor, tool: workspace.purge, decision: allow, params_digest, prev_hash, hash, trace_id }`
   commit together; because it is a super-admin action it also appends to `_lb_audit_system`.
2. Minutes later a different user calls `workspace.purge` on the same workspace without the grant.
   Dispatch: **deny**. A standalone audit entry `{ decision: deny, reason: missing mcp:workspace.purge }`
   is appended and chained — the *attempt* is now on the record, which is the security-relevant fact.
3. A compliance reviewer (granted `mcp:audit.read`) runs `audit.query({tool: "workspace.purge"})`.
   They see both entries — and **their own read** appended a fresh `decision=allow` entry.
4. An auditor runs `audit.verify(ws, node)`: the host re-walks the chain, recomputes each `hash`, and
   confirms no entry was altered or removed (or reports the first break). The deny in step 2 cannot
   have been quietly deleted to hide the attempt.
5. The edge node was offline during steps 1–2; on reconnect its intact local chain syncs to the hub,
   which verifies the origin chain and folds it into the union — no entry lost, no conflict.

## Testing plan

Mandatory categories from `../testing/testing-scope.md` — the security gate, not extras:

- **Capability-deny (§2.1):** (a) `audit.query` without `mcp:audit.read` is refused, opaquely — a
  caller cannot even confirm the ledger exists; (b) **the deny of *another* tool is itself recorded**
  — assert a refused `workspace.purge` produces a `decision=deny` audit entry. Auditing the deny is
  the headline.
- **Workspace-isolation (§2.2):** a ws-B `audit.query` returns **zero** ws-A entries (store + MCP);
  a ws-B principal cannot `audit.verify` or read ws-A's chain. Cross-tenant admin actions land in
  `_lb_audit_system`, not in a tenant ledger, so the wall is held *and* the privileged action is still
  recorded.
- **Offline/sync (§2.3):** an offline node appends an **intact, verifiable chain**; on reconnect it
  syncs append-only to the hub with **no conflict and no loss**; the hub verifies the origin chain.
  A re-sync does not duplicate entries (deterministic `(ws, node, seq)` id upserts once).
- **Immutability/tamper (specified — the load-bearing claim):** (a) there is **no** MCP verb that
  updates or deletes an entry; (b) a direct store edit of any entry makes `audit.verify` report a
  break at that point (the hash chain detects it); (c) the **same-`write_tx`** property — a forced
  failure of the domain write leaves **neither** the change nor its audit row (no action without its
  proof, no proof without its action).
- **Completeness:** a guest (WASM) tool call appends an audit entry **without** the guest's
  cooperation — the guest cannot act un-audited.
- Unit: the canonical-payload encoder + `hash`/`prev_hash` chaining; the `params_digest`/redaction
  helper (shared with observability); the `Secret<T>` non-appearance.

## Risks & hard problems

- **Completeness depends on the single chokepoint.** Any code path that performs an effect *without*
  going through host dispatch is an audit hole — which is the same invariant capability-first already
  requires (nothing acts except through the host). The audit ledger raises the cost of ever adding a
  bypass: a bypass is now both an authz hole *and* an audit hole. Guard it in review.
- **Hash-chain vs. the sync model.** A global order is what §6.8 declines to provide; per-(ws,node)
  chains are the right granularity, but the hub's *union* is not itself a single chain — queries that
  want a global timeline must merge by `ts` across origins and accept clock skew. Don't promise a
  single global tamper-proof order; promise per-origin tamper-evidence + a merged view.
- **Storage growth (WORM, by definition).** The ledger only grows; an append-only table with no
  delete needs an **archival/export** policy (cold-storage export + a verified truncation that
  preserves the chain head), not deletion. Retention is a compliance setting, and "delete old audit"
  is itself an audited, restricted action — or, better, an export-then-seal, never a silent purge.
- **Same-tx coupling cost.** Putting the audit append in the domain `write_tx` is the durability win
  but adds a write to every mutating call's hot path. It is one small upsert; the deny path (the high-
  volume attack case) is a standalone append off the domain tx, so a deny storm doesn't bloat domain
  transactions.
- **`params_digest` must not become a re-identification leak.** A digest of low-entropy params can be
  brute-forced back to the value; for sensitive params, store a *shape summary* (types/sizes) plus a
  salted hash, not a bare hash of the value.

## Open questions

- **Hash function + canonical encoding:** reuse the registry/`lb_auth` SHA-256 + Ed25519 stack
  (no second crypto lib) — lean: SHA-256 chain now; **optionally** Ed25519-*sign* each chain head
  periodically for non-repudiation later. Canonical encoding (sorted-key CBOR/JSON) must be pinned so
  `verify` is reproducible.
- **System-ledger scope:** exactly which super-admin actions also write `_lb_audit_system` (lean:
  every action whose `ws` is null/`_system` or that crosses the wall — purge, key rotation, registry
  publish, user disable across workspaces).
- **Retention/export contract:** export format (the OTel-adjacent log shape? a signed NDJSON?), the
  seal-and-truncate procedure, default retention per role.
- **Gateway-audit fields:** the exact `meta` extension for model calls (token/cost/model/approval-ref)
  so §6.14's promise is met by *this* ledger, not a parallel one.
- **`audit.watch` need:** is a live security tail wanted for v1, or is export-to-SIEM enough? Lean:
  export first; add the tail when a console needs it.
- **Same-tx vs. async append performance** under a write-heavy workload (ingest): does ingest's
  high-volume path get *coarse* audit (one entry per batch commit, not per sample)? Lean: yes — audit
  the batch, not each sample, mirroring its metrics treatment.

## Related

- README **§6.6** (the cap decision recorded), **§6.5** (the dispatch chokepoint), **§6.14** (the AI
  gateway audit this generalizes), **§6.7** (secrets — refs/digests, never values), **§6.8** (sync —
  append-style shared data, the perfect fit), **§3** (capability-first, one datastore, the wall).
- `../observability/observability-scope.md`, `../undo/undo-scope.md` — the sibling projections of the
  same chokepoint ("The shared seam").
- `../inbox-outbox/outbox-scope.md` — source of the reused `write_tx` one-tx seam and the durability
  pattern (audit borrows its committed-together property).
- `../auth-caps/` — the cap grammar/grants whose decisions are the primary audited event;
  `admin-crud-scope.md` (the destructive verbs that most need an audit trail).
- `key-stack.md` — the "Observability/audit" row (this resolves the audit half of its "needs a
  dedicated scope" note); add the `audit` read capability to the cap grammar.
</content>
