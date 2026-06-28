# Undo scope — the reversible-command journal (undo / redo)

Status: scope (the ask). Promotes to `public/undo/` once shipped. Stage: **S10 — cross-cutting
retrofit** (`../../STAGES.md`). The capture chokepoint (host dispatch + the `write_tx` store seam,
README §6.5/§6.8) already ships; **platform-level reversibility was never scoped** — and unlike audit
and observability it was also the *latest into `key-stack.md`* (now row 44), the most-missed of the three.

> Read with: README §6.5 (dispatch — where a tool's reversibility is classified), §6.8 (sync/
> authority + the explicit *"real-time co-editing is a per-extension CRDT concern"* line that bounds
> this scope), §6.10 (outbox — the **irreversible motion** undo must refuse), §3.3 (state vs motion —
> the dividing line of this whole scope), `../observability/observability-scope.md` "The shared seam",
> `../audit/audit-scope.md` (every undo is itself an audited, reversible action),
> `lb-outbox`'s `write_tx` (the seam the before-image rides).

Every mutation in the platform is a host-mediated tool call, yet there is **no platform primitive to
reverse one**. Without it, each extension reinvents undo (badly), and — worse — a naive undo would
try to "un-send" things that **cannot be undone**: an outbox-delivered PR, a sent email, an external
webhook. The core design insight (and the reason this is a *platform* concern, not a feature) is the
line it draws: **undo is coherent only for reversible *state* mutations (SurrealDB records);
irreversible *motion* (outbox effects, §6.10) is never undone — it is *compensated*, a different
pattern.** Getting that boundary into the platform, once, is the deliverable; the undo/redo stack is
the easy part on top of it.

## Goals

- **A reversible-command journal at the host.** When a mutating tool runs, the host captures enough
  to invert it — generically, a **before-image** of the records it changed plus the store-managed
  `rev` it produced — as a journal entry `{ ws, actor, tool, trace_id, ts, kind: do|undo|redo, before,
  after, rev, group }`. Undo restores the before-image **conditionally** (only if the live `rev` still
  matches, enforced at the authoritative node); redo re-applies the after-image. Both are themselves
  mediated, audited, reversible tool calls.
- **The hard reversible/irreversible classification, in the platform.** Every tool is classified
  `reversible` (pure state mutation) | `irreversible` (enqueues an outbox effect / external motion) |
  `compensable` (irreversible but ships a declared compensating action). **Undo refuses irreversible
  tools** and offers the compensating action for `compensable` ones. A tool that *both* mutates state
  and enqueues an effect is **irreversible as a whole** (you cannot un-send the effect even if you
  could restore the record) — this composition rule is the subtle, load-bearing part.
- **Per-(workspace, actor) undo/redo stacks**, with an extension able to scope a finer stack (per
  document, per session) when it wants editor-style undo within its own surface. Bounded depth; a new
  `do` truncates the redo stack (standard semantics).
- **Workspace-walled and capability-gated.** You can only undo within a workspace (never across the
  wall) and only an operation you hold the cap to perform; by default you undo **your own** ops, with
  an explicit, audited admin override for others'. Undo is not a capability-escalation backdoor.
- **Instrumented before-image as the floor; declared inverse as an opt-in.** A before/after image is
  captured for every record a mutation touches **through the instrumented store seam** — `read`/`write`/
  `list` upserts go through it for free, so the common upsert-shaped verbs are undoable with **zero
  per-tool work**. The cases the generic capture cannot see for free are made explicit, not assumed
  away: **creates** journal a `before: absent` tombstone, **deletes** journal the full prior record
  with `after: absent`, **`RELATE` edges** journal the edge's existence + properties, and the raw
  `query_ws` escape hatch (store.md §"Engine") is **not generically capturable** — a tool using it is
  `non-generic` and must either route its mutation through an instrumented builder or **declare its
  touched set** to be undoable; otherwise it is marked not-undoable. A tool may additionally *declare* a
  semantic inverse (`create`↔`delete`) for a cleaner/cheaper undo; optional optimization, not required
  for an upsert-shaped tool to be undoable.

## Non-goals

- **Not collaborative / operational-transform undo.** Real-time co-editing is explicitly a
  per-extension CRDT concern (§6.8, Automerge/Yjs) — *not* a platform feature. Platform undo is
  **single-actor, coarse-grained** (one tool call = one undoable step). It deliberately does **not**
  solve "undo my edit but keep my collaborator's intervening edit"; attempting that with a flat
  before-image would clobber the collaborator (see Risks). That problem stays inside the CRDT
  extension.
- **Not a reversal of external effects.** Anything delivered through the outbox (§6.10) is **out of
  scope for undo** — a delivered PR/email/webhook is undone only by a *compensating* action
  (close the PR, send a correction), which the owning extension declares. Undo never silently issues
  a compensation; it surfaces the declared one for the user to confirm.
- **Not full version history / time-travel of a record.** The journal holds a **bounded** undo/redo
  stack, not the complete mutation history of every record forever. Long-form content history (doc
  versions) is a per-asset concern; the audit ledger holds the *action* record (no before-image).
- **Not a distributed transaction / saga engine.** Multi-step *compensation across services* (the
  saga pattern) is acknowledged as the right model for irreversible chains but is its own scope; here
  we ship the **classification + the hook** that says "this is compensable, here's the action," not a
  saga orchestrator.
- **No new persistence layer.** The journal is SurrealDB records (rule #2).

## Intent / approach

**Capture the before-image at the store-write seam, not by guessing.** The host cannot know which
records an arbitrary tool touched by inspecting the tool — but it *does* mediate every store write
(extensions never touch the store directly; access is host-mediated and capability-checked, §3.5/§6.6).
So the journal hooks the **`lb_store::write_tx` seam** (the same one-tx seam audit and the outbox
reuse): in the transaction that applies a mutation, also read the affected records' prior state and
write a journal entry — **before-image and change commit atomically**, so the journal can never be
out of step with the data. Undo issues the inverse write through the same seam (and journals *itself*
as a `kind:undo` entry, enabling redo). This reuse — one transactional seam serving outbox durability,
audit proof, and undo before-images — is why these three "missed floors" share a foundation rather
than three bolt-ons.

**Classification is runtime transaction taint, not trusted dispatch metadata.** A manifest hint may
*declare* a class, but the authoritative class is **computed from what the transaction actually did**.
The host taints the in-flight transaction the moment its path reaches the outbox (§6.10) — even when
the reaching happens through a *nested* tool call from a tool that declared itself `reversible`. At
commit the host reads the taint: an untainted, instrumented mutation emits an undoable before-image
entry; a **tainted** transaction emits **no undoable entry**, marks the step "external — not undoable"
(greyed in the UI), and records any declared compensation handle. **The composition rule is enforced by
the taint, not by a manifest field:** if a single logical action both mutates state and enqueues an
effect — or calls anything that does — the *enclosing* action is tainted `irreversible`/`compensable`,
never `reversible`; the class is the **max** over everything the transaction touched. A declared
`compensable` can only *add* a compensation to a derived `irreversible`; it can never *downgrade* one.
Encoding this wrong (a "reversible" tool whose nested call silently enqueued an effect) is the footgun
the whole scope exists to prevent, which is why class is derived from runtime taint and not believed
from a manifest.

**Undo is a forward action, not a magic rewind.** Restoring a before-image is an ordinary
`(table,id)` upsert — so it **syncs like any write** (§6.8) and is **audited like any action**
(`../audit/`). This keeps undo inside every existing invariant (the wall, the cap check, the audit
ledger, the sync path) instead of being a privileged side-channel that bypasses them. **Rejected:**
an out-of-band "restore" that writes records without going through dispatch/`write_tx` — it would
skip the cap check and the audit entry, making undo a hole in exactly the systems this retrofit set
is closing.

**Undo is a *conditional* restore enforced at the authoritative node, not a local LWW upsert.** This is
the correctness core. A naive optimistic check done only on the edge is unsafe: an offline edge can pass
its local check, then later sync a now-stale restore that LWW-clobbers a hub-side or intervening change
(§6.8 — edges read-cache, the hub holds authority for shared data). So every undoable journal entry
records, per touched record, the **expected `rev`** (a store-managed monotonic revision; see below) of
the state the undo expects to overwrite — i.e. the `after` it produced. `undo()` submits a **conditional
restore operation** that carries `{ touched records, expected rev each, before-image, group }`; the
**authoritative node applies it only if every current record still matches its expected `rev`**, and
**refuses atomically otherwise** — no record is half-restored. The expected-`rev` predicate travels
*with* the operation and is checked at the apply point, so an offline-captured undo that arrives stale is
refused at the hub, not silently merged. A local-only mutation (never shared) is its own authority and
applies locally under the same predicate.

**A store-managed revision is a prerequisite, not an assumption.** The shipped store wraps host JSON
under an opaque `data` field with no generic version (store.md §Conventions). This scope therefore
**requires the store seam to stamp a monotonic `rev`** (or equivalently a canonical `after_hash`) on
every `write_tx`-applied record, surfaced so the journal can record it and the conditional restore can
test it. Without this the optimistic check is ad-hoc and the offline-stale guarantee above does not
hold; adding `rev` at the `write_tx` seam is in-scope for this work.

**Undo restores through the same seam, but is not a privileged rewrite.** The restore is an ordinary
`(table,id)` write through `write_tx` (so it is capability-checked, audited, and synced like any write)
**guarded by the conditional predicate** — never an out-of-band write that skips dispatch. **Rejected:**
a forced restore or an out-of-band "restore" that bypasses the predicate or the seam — it would clobber
concurrent writers and skip the cap/audit entry, making undo a hole in the systems this retrofit closes.
The honest limit stands: when the predicate fails, undo **declines** rather than clobbers.

**Generic restore is safe only when the journal captures the full durable invariant surface.** A raw
record upsert can skip tool-level validation, derived/aggregate records, relation consistency, or
secondary indexes that the forward tool maintained. Generic before-image undo is therefore the floor
**only for self-contained record mutations**; an action that maintains derived state must journal that
derived state in its **group** (so the group restores all-or-nothing) or **declare a semantic inverse**.
A tool whose durable footprint the generic capture cannot fully see is marked `non-generic` and is
undoable only via its declared inverse — never by a partial raw restore that leaves invariants broken.

**Bounded, per-actor stacks; redo truncated on new work — immutable events plus a materialized cursor.**
The journal is **two shapes, not one**: append-only, immutable journal *event* rows (`do`/`undo`/`redo`,
each with before/after/`rev`/`group`) that sync exactly like audit rows; and a separate **materialized
stack-state record** per `(ws, actor[, surface])` holding the mutable cursor, the live undo/redo
position, and prune markers. Redo-truncation, the current cursor, and depth pruning mutate the
stack-state record (an ordinary LWW state record under §6.8), while history stays immutable and
append-style — so "stack changed" never rewrites history, and two panes reconcile the cursor as ordinary
state. The stack is `(ws, actor[, surface])` keyed, depth-capped (config), and a new `do` advances the
cursor past — and prunes — the redo tail. An extension that wants document-scoped undo passes a
`surface` key; the platform mechanism is the same.

## How it fits the core

- **Tenancy / isolation:** journal entries carry `ws` and live in the workspace namespace; undo is a
  write *within* the workspace, so it physically cannot cross the wall. A ws-B actor can neither see
  nor pop ws-A's undo stack (structural, §7).
- **Capabilities:** undo of a tool requires you to **hold that tool's cap** (you cannot reach a
  mutation via undo that you couldn't perform directly — no escalation). By default the stack is your
  own; an admin override to undo another actor's op needs a distinct grant (`mcp:undo.any`) and is
  audited. `mcp:undo:call`/`redo:call`/`history.list:call` gate the verbs; deny is opaque.
- **Placement:** *either* — the journal is a core-crate mechanism on every node; an offline edge can
  undo its own local mutations and the restoring write syncs like any other. No `if cloud`.
- **MCP surface:** `undo(surface?)`, `redo(surface?)`, `history.list(surface?)` (the stack, for a UI
  affordance), and `history.compensations(step)` (what compensating actions a non-undoable step
  offers). **No** verb writes the journal directly — capture is host-internal at `write_tx` (no
  forgeable "journal.write", same discipline as audit/tags). Reads are gated; the live state of a
  stack is a `list`, not a stream (the stack is state; if a multi-admin UI needs change push, that's a
  later watch).
- **Data (SurrealDB):** an immutable `undo_journal` event table per workspace (`undo:{seq}` with
  `before`/`after`/per-record `rev`/`group`/`kind`), plus a mutable `undo_stack:{actor[:surface]}`
  state record holding the cursor (above). Events are append-style and bounded per `(actor, surface)`;
  the cursor record is ordinary LWW state. **State.** The before-image is a record snapshot stamped with
  the store-managed `rev`. **Buckets are degraded on the shipped engine** (`DEFINE BUCKET` ✗,
  store.md — binary payloads already use record-as-content), so v1 file undo uses the **record-as-content
  versioned fallback**, not a `DEFINE BUCKET` copy-on-write ref; very large records may opt out of
  journaling and declare a compensation instead (see Risks). Old entries past the depth/age cap are
  pruned (unlike the WORM audit ledger — different retention by design).
- **Bus (Zenoh):** none for the mechanism (the journal is state). A "stack changed" hint, if a
  multi-pane UI needs it, is ordinary fire-and-forget motion published by the caller — not by this
  crate (state vs motion stays clean).
- **Sync / authority:** undo's restoring write is a **conditional** `(table,id)` upsert on the existing
  §6.8 path — the expected-`rev` predicate travels with the operation and is enforced **at the
  authoritative node**, so an offline-captured restore that arrives stale is **refused at the hub**, not
  LWW-merged. The immutable journal *events* are append-style and sync like audit rows; the mutable
  *cursor* is an ordinary LWW state record. **The honest limit:** if a record changed after its
  before-image was captured (a concurrent writer), the predicate fails and the undo is **refused**
  rather than clobbering the intervening write — platform undo is single-actor by contract.
- **Secrets:** a before-image must never inline a secret value; if a mutation touched a secret it is
  referenced, not snapshotted (§6.7), and a `Secret<T>` cannot land in `before`/`after`. **This requires
  secret references to be immutable/versioned** — a bare ref that is later overwritten cannot restore the
  old value. v1 therefore undoes a secret mutation only when the prior secret *version* is still
  resolvable by ref; if a secret is non-versioned the touching action is `non-generic` and undoable only
  via a declared compensation (e.g. re-set the value), never by restoring a dangling ref.

## Example flow

1. A user calls `doc.rename` (a pure state mutation, class `reversible`). In one `write_tx`, the rename
   commits **and** a journal entry `{ before: {title:"draft"}, after: {title:"v1"}, kind:do }` is
   written, atomically. It is audited (`decision=allow`) and (if synced) replicated like any write.
2. The user hits undo. `undo()` checks they hold `mcp:doc.rename:call` and submits a **conditional
   restore**; the **authoritative node** confirms the doc's current `rev` still matches the entry's
   recorded `after` `rev` (no intervening writer), restores `before` through `write_tx`, and journals a
   `kind:undo` entry — which redo can re-apply. The restore is itself audited. (If the doc lived only on
   this offline edge, the edge is its own authority and applies the same predicate locally.)
3. The user calls `workflow.open_pr` — this enqueues an **outbox effect** (§6.10), so it is class
   `irreversible`. The journal records the step as **"external — not undoable"**; `undo()` **refuses**
   to reverse it and `history.list` shows it greyed.
4. The PR action was registered `compensable` with a `workflow.close_pr` compensation.
   `history.compensations(step)` surfaces "Close the PR opened by this step" — the user confirms, and
   `close_pr` runs as a *new, forward, audited* action (a compensation, not an undo). The original
   action stays on the audit ledger forever; nothing was rewritten behind the world's back.
5. Meanwhile a collaborator edited the same doc between steps 1 and 2. The optimistic version check in
   step 2 fails; `undo()` returns **"the document changed since this step — undo refused"** rather
   than silently clobbering the collaborator. (Collaborative undo is the CRDT extension's job, §6.8.)

## Testing plan

Mandatory categories from `../testing/testing-scope.md`:

- **Capability-deny (§2.1):** `undo`/`redo`/`history.list` refused without their grant; undoing a tool
  whose cap the actor lacks is refused (no escalation via undo); `undo.any` required to touch another
  actor's stack — opaque deny throughout.
- **Workspace-isolation (§2.2):** a ws-B actor cannot list, undo, or redo ws-A's journal; the restore
  write lands only in the caller's workspace (store + MCP).
- **Offline/sync (§2.3):** an offline edge undoes a local mutation; the restore syncs idempotently
  (deterministic entry/upsert id → applied once across a re-sync, no duplicate restore). **The
  authoritative-node conditional check is the load-bearing case:** an offline edge captures an undo,
  meanwhile the hub's copy changes; on re-sync the conditional restore is **refused at the hub** (its
  expected `rev` no longer matches), proving the predicate is enforced at the apply point and not only
  locally — no silent LWW clobber.
- **Revision predicate (§new):** the store seam stamps a monotonic `rev` on every `write_tx` record;
  a conditional restore succeeds iff every touched record's current `rev` equals the expected `rev`, and
  **refuses all-or-nothing** if any one differs (a multi-record group, one record changed → whole undo
  refused, none restored).
- **The irreversible boundary (specified — the load-bearing claim):** (a) an `irreversible` tool
  (enqueues an outbox effect) is **journaled as not-undoable** and `undo()` **refuses** it; (b) a
  **mixed** action (state + effect) is classified `irreversible`/`compensable`, never reversible — the
  composition `max` rule; (c) a `compensable` tool surfaces its declared compensation and running it is
  a *new audited forward action*, leaving the original on the audit ledger; (d) **runtime taint, not
  metadata:** a tool that *declares* `reversible` but whose **nested call** reaches the outbox is tainted
  at commit and emits **no undoable entry** — proving the class is derived from what ran, not believed
  from the manifest. A test that only covers pure-state undo would miss the entire reason this scope
  exists.
- **Non-generic capture:** a tool mutating via the raw `query_ws` escape hatch (or maintaining derived
  state) without a declared touched-set/inverse is marked **not-undoable** rather than partially
  restored; a tool that declares its inverse undoes correctly without invariant breakage.
- **Atomicity:** the before-image and the change commit in **one `write_tx`** — a forced failure leaves
  **neither** (no orphan journal entry, no un-journaled change).
- **Redo semantics:** undo→redo round-trips a record exactly; a new `do` truncates the redo stack.
- Unit: the classification `max`-composition; the optimistic version check; stack depth/prune; the
  `Secret<T>`/snapshot-ref non-inlining.

## Risks & hard problems

- **The irreversible-effect footgun is the whole point.** If a tool that opens a PR is mis-classified
  `reversible`, undo will "succeed" by restoring a record while the PR stays open in the world —
  silent, dangerous divergence. Mitigation is structural: the moment a tool's path reaches the outbox,
  the host **derives** `irreversible` (not the author's say-so), so the classification is computed from
  the effect, not trusted from a manifest field. Author-declared `compensable` only *adds* a
  compensation; it cannot *downgrade* a derived `irreversible`.
- **Concurrent-write clobber.** A flat before-image restore is last-writer-wins; restoring it over an
  intervening write loses that write. The optimistic version check (refuse undo if current ≠ expected
  `after`) makes this safe-by-refusal, at the cost of "undo sometimes declines." That is the correct
  trade for a non-CRDT platform; true collaborative undo stays in the CRDT extension (§6.8). **Do not**
  paper over this with a forced restore.
- **Before-image size for large records / files.** Snapshotting a large record per mutation is costly;
  snapshotting blob content inline is prohibitive. **And `DEFINE BUCKET` is degraded on the shipped
  engine** (store.md), so the copy-on-write *bucket* ref is not available for v1: file undo uses the
  **record-as-content versioned fallback** already used for binary ingest (retain the prior content
  version, don't re-copy), and very large records may opt out of journaling with a declared compensation
  instead. Naive inline snapshots would balloon the store — a real design constraint, not a tuning note.
  COW *bucket* refs are a follow-up gated on bucket support landing.
- **Multi-step / grouped actions.** A job or a batch is many mutations; undo must operate on a
  **group** (a `group` id on entries) so "undo that import" reverses all-or-nothing, in reverse order,
  and refuses if any step is irreversible. Group reversal that hits an irreversible step mid-way must
  **not** half-undo — it refuses up front (pre-check the group's max class).
- **Journal growth + retention** (unlike audit, this *is* prunable): depth/age caps per (actor,
  surface), with COW content refs GC'd when no journal entry references them.
- **Interaction with the audit ledger.** Undo entries and the actions they reverse **both** stay on the
  audit ledger forever (audit is WORM; the undo *journal* is bounded). An undo does not erase history —
  it adds a reversing action to it. Conflating "undo" with "delete the audit trail" would be a security
  regression; they are explicitly separate stores with opposite retention.

## Decisions (v1 — settled)

These are decided for v1, not open. Each names the rejected alternative so the *why* survives.

- **Reversibility class is derived host-side; no ABI change for v1.** The class is computed from runtime
  transaction taint (reaches-the-outbox ⇒ irreversible), which needs **no** WIT change. Author-declared
  `compensable` (naming a compensation tool) and the editor `surface` key *do* want a manifest field —
  shipped as a **separate, additive, scoped ABI change** when the first extension needs it, not now.
  *Rejected:* trusting a manifest `reversible` flag — a nested outbox call would make it a lie.
- **Stack granularity: per-(ws, actor) by default, opt-in `surface`.** Editor-like extensions pass a
  `surface` key for document-scoped undo; the platform mechanism is identical. *Rejected:* per-surface
  required — needless ceremony for the common single-stack case.
- **Group/transaction undo: reverse order, all-or-nothing, pre-checked.** A job/batch registers its
  mutations under one `group` id; undo reverses them newest-first, and **refuses up front** if the
  group's **max** class is irreversible — it never half-undoes. The conditional `rev` predicate covers
  the whole group atomically. *Rejected:* best-effort partial group undo — leaves the world incoherent.
- **Optimistic check granularity: whole-record `rev`.** A store-managed monotonic `rev` (per
  `(table,id)`) is the predicate unit — simplest correct refusal. *Rejected:* field-level merge — that
  is collaborative/CRDT undo, explicitly out of scope (§6.8).
- **File/blob undo: record-as-content versioned fallback, depth-capped.** Buckets are degraded
  (store.md), so v1 retains prior content *versions* in-record up to the configured depth, then falls
  back to "not undoable, here's a compensation." COW bucket refs follow once bucket support lands.
- **Admin override (`undo.any`): distinct grant, always prominently audited.** Undoing another actor's
  op requires `mcp:undo.any` and writes an audit entry naming both actors. *Rejected:* implicit admin
  reach — undo must never be a capability-escalation backdoor.
- **Compensation registry: declare-a-handle only; orchestrator deferred.** A tool names its compensating
  tool and a static argument mapping; multi-step saga orchestration is a jobs-adjacent follow-up
  (`../jobs/`). *Rejected:* building a saga engine here — out of scope, deferred deliberately.

## Related

- README **§6.5** (dispatch — where class is read), **§6.8** (sync/authority + the CRDT-is-per-extension
  line that bounds this), **§6.10** (outbox — the irreversible motion undo refuses), **§3.3** (state vs
  motion — the dividing line), **§3** (one datastore, the wall, capability-first).
- `../observability/observability-scope.md`, `../audit/audit-scope.md` — the sibling projections of the
  same chokepoint ("The shared seam"); every undo is an audited, traced action.
- `../inbox-outbox/outbox-scope.md` — the `write_tx` seam reused for the atomic before-image, and the
  effect-delivery path that *defines* irreversibility.
- `../jobs/jobs-scope.md` — multi-step actions whose undo is a *group*; the saga/compensation
  orchestrator is a jobs-adjacent follow-up.
- `key-stack.md` — the `undo` / reversible-command-journal row (row 44; before-image + store-managed
  `rev` over the `write_tx` seam).
</content>
