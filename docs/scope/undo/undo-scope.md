# Undo scope — the reversible-command journal (undo / redo)

Status: scope (the ask). Promotes to `public/undo/` once shipped. Stage: **S10 — cross-cutting
retrofit** (`../../STAGES.md`). The capture chokepoint (host dispatch + the `write_tx` store seam,
README §6.5/§6.8) already ships; **platform-level reversibility was never scoped** — and unlike audit
and observability it is also *absent from `key-stack.md`*, the most-missed of the three.

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
  to invert it — generically, a **before-image** of the records it changed — as a journal entry
  `{ ws, actor, tool, trace_id, ts, kind: do|undo|redo, before, after, group }`. Undo restores the
  before-image; redo re-applies the after-image. Both are themselves mediated, audited, reversible
  tool calls.
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
- **Generic before-image as the floor; declared inverse as an opt-in.** Record-level before/after
  images work for *any* CRUD with **zero per-tool work** — the default. A tool may additionally
  *declare* a semantic inverse (`create`↔`delete`) for a cleaner/cheaper undo; optional optimization,
  not required for a tool to be undoable.

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

**Classification lives in tool metadata, read at dispatch.** A tool registers its reversibility class
(default `reversible` for a pure state verb; `irreversible` the moment it calls the outbox;
`compensable` + a named compensating tool when it can offer one). Dispatch (§6.5) reads it: a
`reversible` call journals a before-image; an `irreversible` call journals **nothing undoable** and
marks the step "external — not undoable" so the UI shows it greyed; a `compensable` call records the
compensation handle. **The composition rule:** if a single logical action both mutates state and
enqueues an effect, the *enclosing* action is `irreversible`/`compensable`, never `reversible` — the
classification is the **max** over its parts. Encoding this wrong (calling such an action reversible)
is the footgun the whole scope exists to prevent.

**Undo is a forward action, not a magic rewind.** Restoring a before-image is an ordinary
`(table,id)` upsert — so it **syncs like any write** (§6.8) and is **audited like any action**
(`../audit/`). This keeps undo inside every existing invariant (the wall, the cap check, the audit
ledger, the sync path) instead of being a privileged side-channel that bypasses them. **Rejected:**
an out-of-band "restore" that writes records without going through dispatch/`write_tx` — it would
skip the cap check and the audit entry, making undo a hole in exactly the systems this retrofit set
is closing.

**Bounded, per-actor stacks; redo truncated on new work.** The stack is `(ws, actor[, surface])`
keyed, depth-capped (config), and a new `do` clears the redo stack — standard, predictable. An
extension that wants document-scoped undo passes a `surface` key; the platform mechanism is the same.

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
- **Data (SurrealDB):** an `undo_journal` table per workspace (`undo:{seq}` with `before`/`after`/
  `group`/`kind`), bounded per `(actor, surface)`. **State.** The before-image is a record snapshot;
  for **file/bucket** content the snapshot is **metadata + a copy-on-write content ref**, not an
  inline blob copy (see Risks). Old entries past the depth/age cap are pruned (unlike the WORM audit
  ledger — different retention by design).
- **Bus (Zenoh):** none for the mechanism (the journal is state). A "stack changed" hint, if a
  multi-pane UI needs it, is ordinary fire-and-forget motion published by the caller — not by this
  crate (state vs motion stays clean).
- **Sync / authority:** undo's restoring write is an `(table,id)` upsert on the existing §6.8 path,
  last-writer-wins on the rare contested record. The journal entries themselves are append-style and
  sync like audit rows. **The honest limit:** if the record changed *after* the before-image was
  captured (a concurrent writer), an undo's LWW restore would clobber that intervening write — so
  platform undo is single-actor by contract, and a stale undo is **refused** when the record's current
  version differs from the after-image it expects (optimistic check — see Risks).
- **Secrets:** a before-image must never inline a secret value; if a mutation touched a secret it is
  referenced, not snapshotted (§6.7), and a `Secret<T>` cannot land in `before`/`after`.

## Example flow

1. A user calls `doc.rename` (a pure state mutation, class `reversible`). In one `write_tx`, the rename
   commits **and** a journal entry `{ before: {title:"draft"}, after: {title:"v1"}, kind:do }` is
   written, atomically. It is audited (`decision=allow`) and (if synced) replicated like any write.
2. The user hits undo. `undo()` checks they hold `mcp:doc.rename:call`, confirms the doc's current
   version still matches the entry's `after` (no intervening writer), restores `before` through
   `write_tx`, and journals a `kind:undo` entry — which redo can re-apply. The restore is itself
   audited.
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
  (deterministic entry/upsert id → applied once across a re-sync, no duplicate restore); the optimistic
  version check refuses a **stale** undo whose target was changed by an intervening synced write (no
  silent clobber).
- **The irreversible boundary (specified — the load-bearing claim):** (a) an `irreversible` tool
  (enqueues an outbox effect) is **journaled as not-undoable** and `undo()` **refuses** it; (b) a
  **mixed** action (state + effect) is classified `irreversible`/`compensable`, never reversible — the
  composition `max` rule; (c) a `compensable` tool surfaces its declared compensation and running it is
  a *new audited forward action*, leaving the original on the audit ledger. A test that only covers
  pure-state undo would miss the entire reason this scope exists.
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
  snapshotting **bucket/blob content** inline is prohibitive. So file undo journals **metadata + a
  copy-on-write content reference** (the prior content version is retained, not re-copied), and very
  large records may be opt-out of journaling with a declared compensation instead. Naive inline
  snapshots would balloon the store — a real design constraint, not a tuning note.
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

## Open questions

- **Reversibility metadata + the SDK/WIT boundary (flag loudly).** The class is mostly *derived* by the
  host (reaches-the-outbox ⇒ irreversible), which needs **no** ABI change. But a guest declaring a
  `compensable` compensation tool, or an editor-style `surface` key, may want a manifest field /
  host import — an **additive, forever** WIT change. Lean: derive the class host-side for v1 (no ABI
  change); add a manifest `compensation` field as a separate scoped ABI change when an extension needs
  it.
- **Default stack granularity:** per-(ws, actor) global vs. per-surface required. Lean: per-(ws,actor)
  default, opt-in `surface` for editor-like extensions.
- **Group/transaction undo semantics:** confirm "reverse order, all-or-nothing, refuse if any step
  irreversible" and how a job's steps register as a group.
- **Optimistic-check granularity:** whole-record version vs. field-level. Lean: whole-record version
  (`vsn`/updated-at) for v1 — simplest correct refusal.
- **File/blob undo depth:** how many COW content versions to retain per asset before falling back to
  "not undoable, here's a compensation."
- **Admin override (`undo.any`) policy:** when, and how prominently audited.
- **Compensation registry shape:** how a tool names its compensating tool + how arguments map (the
  saga-handle design) — bounded here to "declare a handle," the orchestrator deferred.

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
- `key-stack.md` — add an `undo` / reversible-command-journal row (currently absent — the most-missed
  of the three).
</content>
