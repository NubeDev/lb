# Undo scope — exposure: from green mechanism to a product surface

Status: scope (the ask). Promotes to `doc-site/content/public/undo/` once shipped. Stage: **S10
follow-on** (the retrofit's undo row is "building" in `STATUS.md`; this is the slice that finishes it).

The undo mechanism is shipped and green end to end — store `rev`, the `lb-undo` journal, the
conditional stale-refusing restore, the cap-gated `undo`/`redo`/`history.*` verbs, and
auto-capture-on-dispatch (`undo-scope.md`, both session docs). But a review (2026-07-15) found that
**nothing in the product can reach it**: no built-in role grants the verbs' caps — the dotless
`undo`/`redo` (and `history.compensations`) match none of the `mcp:*.<verb>:call` role wildcards —
no typed gateway route exposes them, and no UI calls them. The same review found one real
correctness bug in the capture path. This scope is the exposure slice: **fix the bug, grant the
caps, add the routes, ship the shell affordance, and prove the load-bearing sync case** — the
distance between "the mechanism exists" and "a user can press undo".

## Goals

- **Pre-exposure hardening (fix before traffic multiplies blast radius):**
  - **A failed before-image read must never journal "absent".** Today
    `undo_capture/capture.rs` maps a `read_versioned` *error* to the same shape as a genuine
    create (`before: None, rev 0`), so a transient read failure on an existing record journals a
    tombstone before-image — and a later undo would **delete real data** (the rev predicate guards
    the *after* state, so it passes). Fix: a read error marks the step **not-undoable**; only a
    successful read that finds nothing is `absent`. Regression-tested.
  - **The prune floor the cursor already promises.** `StackState::push_do` trims the cursor past
    the depth cap with a comment saying the immutable entry "is pruned separately" — but no prune
    exists, so journal events (and their `undo_live` companions) grow unbounded. Exposure makes
    growth real: prune events that fall past the depth cap, in the same transaction as the push.
- **Role grants.** `member` gains `mcp:undo:call`, `mcp:redo:call`, `mcp:history.compensations:call`
  (authoring-tier: undo is a mutation, and the no-escalation check already means undo can never
  reach beyond the caps you hold). `mcp:undo.any:call` joins `ADMIN_ONLY_CAPS`. `history.list`
  already rides the viewer `mcp:*.list:call` read wildcard — keep it (read-only, and a viewer
  seeing history they cannot act on is correct, not a leak).
- **Typed gateway routes** (`role/gateway/src/routes/undo.rs`, mirroring the `flows.rs` precedent):
  `POST /undo`, `POST /redo`, `GET /undo/history`, `GET /undo/history/{seq}/compensations` — each
  authenticating the session, taking the workspace from the token (§7), and re-checking the cap
  server-side via `lb_host::call_tool`. The verbs' UI-shaped outcomes
  (`ok:false, reason:"stale"|"not_undoable"|"empty"`) pass through typed, not stringly.
- **The shell affordance** (`ui/`, per the platform's usability bar — a verb nobody can press is
  not shipped): global **Ctrl/Cmd+Z / ⇧⌘Z** wired to the routes, and a **History panel** rendering
  `history.list` — irreversible steps greyed ("external — not undoable"), compensable steps
  offering their declared compensation behind an explicit confirm (a *new forward action*, per the
  parent scope), a `stale` refusal surfaced as an honest toast ("changed since this step — undo
  refused"), never silence.
- **The load-bearing sync test the parent scope names and the build skipped** (§2.3): an offline
  edge captures an undo, the hub's copy changes, the conditional restore is **refused at the hub**
  on re-sync — proving the predicate is enforced at the apply point, not only locally.
- **`docs/skills/undo/SKILL.md`** — the verbs become an agent-drivable surface the moment they are
  granted; the implementing session writes the skill from a live run (§6 checklist).

## Non-goals

- **Grouped undo reversal** (reverse-order, all-or-nothing, refuse-if-any-irreversible). The
  `group` id is threaded end to end; the reversal logic is its own follow-up scope. The History
  panel may *display* a group as one row, but v1 undoes single steps.
- **Widening the reversible floor** beyond the five captured verbs (`inbox.record`,
  `assets.put_doc`/`delete_doc`/`put_asset`/`delete_asset`). More allowlist entries — and the
  **structural `Secret<T>` never-in-a-snapshot guard** the parent scope requires — ship together in
  the floor-widening follow-up. (Safe today: none of the five captured tables carries secret
  material; the guard is a hard prerequisite for the *next* entry, stated here so it isn't lost.)
- **No `history.watch` stream.** The stack cursor is state; the panel reads `history.list` on open
  and after its own actions. A multi-pane live cursor is a later watch, per the parent scope.
- **No manifest `compensation` WIT field, no file/blob undo, no saga orchestrator** — all deferred
  by the parent scope's settled decisions; unchanged here.

## Intent / approach

**Grants are the real gap; routes are consistency.** The generic `/mcp/call` bridge (`routes/mcp.rs`)
already dispatches any tool, so once the caps are granted, undo is *technically* reachable over
HTTP. We still ship dedicated routes because that is the house pattern for every core surface
(`flows.rs`, `inbox.rs`, `history.rs` for channels…): typed request/response, the refusal reasons
handled as data, and the shell never composing raw MCP envelopes for a core affordance.
**Rejected:** bridge-only exposure — it works, but it makes the shell hand-roll the outcome
handling every core route gets for free, and it breaks the convention that `/mcp/call` is the
*extension-page* seam.

**Fix, then expose — in that order.** The read-error-as-absence bug is exactly the "silent wrong
restore" class the parent scope exists to prevent; exposing undo to every member before fixing it
multiplies a rare footgun by real traffic. The fix lands first in the same slice. **Rejected:**
shipping exposure and hardening as separate scopes — the ordering constraint *is* the point of
scoping them together.

**Prune on push, in one transaction.** When a new `do` trims the cursor past the depth cap, delete
the fallen-off journal events and their `undo_live` companions in the same store transaction that
persists the cursor — no background sweeper, no window where the cursor and the events disagree.
**Rejected:** a periodic GC job — more machinery for a bound the push already knows exactly.

**The shell yields Ctrl+Z to focused editors.** Two editor-grade undo systems already exist in the
product (the flows canvas's client-side undo; ce-wiresheet's engine-shared undo), and the parent
scope deliberately supports finer `surface`-scoped stacks. The global shortcut fires **only when no
surface that owns its own undo has focus**; a focused editor keeps its native semantics. This is a
UX contract, not plumbing — get it wrong and platform undo *fights* editor undo, which is worse
than no affordance.

## How it fits the core

- **Tenancy / isolation:** routes take the workspace from the verified session token, never the
  body (§7); the verbs behind them are already workspace-walled (proven in the host suite). Route
  tests re-prove it over HTTP.
- **Capabilities:** `mcp:undo:call` / `mcp:redo:call` / `mcp:history.compensations:call` at member
  tier; `mcp:undo.any:call` admin-only; `history.list` on the existing viewer read wildcard. Deny
  is an opaque `403` with no existence signal (the MCP deny contract). The host-side no-escalation
  check (the original tool's cap) and `undo.any` are untouched — this scope adds *grants and
  plumbing*, no new authority.
- **Placement:** either. The mechanism is symmetric and already on every node; the routes are
  gateway-role config, not a code branch.
- **MCP surface / API shape (§6.1):** no new verbs. Shape check for the exposure: two action calls
  (`undo`, `redo` — single-step, always fast, synchronous), get/list reads (`history.list`,
  `history.compensations`), **no batch** (grouped undo deferred), **no live feed** (cursor is
  state; poll-on-open, watch deferred). CRUD is N/A — the journal is host-written only, by design
  (no forgeable `journal.write`).
- **Data (SurrealDB):** no new tables. Pruning deletes `undo:{seq}` events + `undo_live:{seq}`
  companions past the depth cap — the parent scope's retention posture (the journal, unlike the
  WORM audit ledger, *is* prunable).
- **Bus (Zenoh):** none. No new motion.
- **Sync / authority:** unchanged mechanism; this scope *proves* the §2.3 case (stale offline
  restore refused at the hub) that the build sessions left untested.
- **Secrets:** none touched by exposure. The structural snapshot guard is named in Non-goals as the
  floor-widening prerequisite.
- **SDK/WIT impact:** none. No ABI change.
- **Skill doc:** yes — `docs/skills/undo/SKILL.md`, written by the implementing session from a live
  run (undo/redo/history over the routes and over MCP, including the refusal shapes).

## Example flow

1. A member saves a doc in the shell (`assets.put_doc`) — auto-captured as an undoable step, as
   already shipped.
2. They press **Ctrl+Z** (no editor with its own undo has focus). The shell calls `POST /undo`;
   the gateway authenticates, re-checks `mcp:undo:call`, and dispatches. The doc's before-image is
   conditionally restored; the History panel refreshes and shows the step as undone.
3. A collaborator had meanwhile edited the same doc: the rev predicate fails, the verb returns
   `ok:false, reason:"stale"`, and the shell shows "the document changed since this step — undo
   refused". Nothing was clobbered, and the user was told the truth.
4. The panel shows an earlier `workflow.open_pr` step greyed ("external — not undoable") with its
   declared compensation. The user clicks it, confirms, and `close_pr` runs as a new, forward,
   audited action through the ordinary dispatch path.
5. A viewer opens the same panel: they can *see* history (`history.list` rides the read wildcard)
   but `POST /undo` is an opaque `403` — and even a mis-granted `mcp:undo:call` would still refuse
   at the host's no-escalation check, since a viewer holds no write caps to undo with.

## Testing plan

Mandatory categories (`../testing/testing-scope.md`), all against the real store/gateway (rule #9):

- **Capability-deny (§2.1), at the route:** `POST /undo`/`/redo` and `GET .../compensations`
  refused `403`-opaque for a principal without the grant; `undo.any` still required over the route
  to touch another actor's stack.
- **Workspace-isolation (§2.2), at the route:** a ws-B session cannot list, undo, or redo ws-A's
  journal over HTTP; the restore lands only in the caller's workspace.
- **Offline/sync (§2.3) — the load-bearing case:** an offline edge captures an undo; the hub's
  copy of the record changes; on re-sync the conditional restore is **refused at the hub**
  (expected `rev` mismatch), no silent LWW merge. This is the parent scope's named proof, owed
  since the build sessions.
- **Capture read-error regression:** the outcome mapping (read `Err` → not-undoable; read
  `Ok(absent)` → `before: absent`) extracted as a pure decision function
  (FILE-LAYOUT: one responsibility per file) and unit-tested — no mock store needed, and the
  distinction can never silently regress to "absent".
- **Prune:** pushing past the depth cap deletes the fallen-off event + its `undo_live` companion
  in the same transaction; the remaining stack still undoes/redoes correctly; a pruned seq in
  `history.compensations` returns the existing "not in the journal" error, not a panic.
- **Route round-trip:** save → `POST /undo` restores → `POST /redo` reapplies, with the typed
  refusal shapes (`stale` after an intervening write; `empty` on a fresh stack) asserted as data.
- **UI (gateway-backed, no fakes):** History panel renders undoable vs greyed-irreversible rows
  from a real seeded journal; the compensation confirm fires the real forward call; the stale
  toast renders from a real `reason:"stale"`; Ctrl+Z is **not** captured while a
  surface-owning editor has focus.

## Risks & hard problems

- **Shortcut contention is the UX cliff.** A shell-global Ctrl+Z that fires while the flows canvas
  or wiresheet has focus makes platform undo *destroy* editor state trust. The focus contract in
  "Intent" is the mitigation; the UI test for it is mandatory, not optional polish.
- **Undo becomes discoverable before users understand refusal.** "Undo refused: changed since"
  is correct behavior that reads as a bug if surfaced tersely. The toast copy must say *what*
  changed (the record, since this step) and *why that's protective* — one sentence, not a code.
- **Prune racing an in-flight undo.** An undo reads its entry, then restores; a concurrent push
  pruning that seq mid-flight must not leave a half-state. The one-transaction prune plus the
  existing "seq not in the journal" typed error is the containment; the prune test covers the
  ordering.
- **The panel invites "undo someone else's step".** `history.list` shows the actor's own stack by
  default; the admin `undo.any` path stays deliberately out of the v1 panel (an admin can use MCP)
  so the UI never normalizes cross-actor undo. Revisit only with a real admin ask.

## Open questions

- **Panel placement:** its own right-dock tab (alongside Config/Debug in the shell's existing
  `RightDock` pattern) vs. a popover off the toolbar undo button. Recommendation: dock tab —
  history is a list you scan, not a tooltip — but the shell owner should confirm against the
  current dock economy.
- **Does `redo` get a shortcut on day one** (⇧⌘Z), or button-only until the focus contract has
  soaked? Recommendation: ship both shortcuts behind the same focus gate; they share the risk.
- **How does a surface declare "I own undo while focused"?** A shell-level registry flag on the
  surface registration vs. a DOM-level `data-owns-undo` boundary check. Small, but it *is* the
  focus contract — decide during implementation and record the choice in the session doc.

## Related

- `undo-scope.md` — the parent: mechanism, classification, conditional restore (all shipped).
- `../../sessions/undo/undo-build-session.md`, `../../sessions/undo/undo-autocapture-session.md` —
  what shipped and what these goals close (the capture floor there is stale: it has since grown to
  five verbs in `crates/host/src/undo_capture/plan.rs`).
- `../testing/testing-scope.md` §2.1–2.3 — the mandatory categories above.
- `rust/role/gateway/src/routes/flows.rs` — the typed-route precedent these routes mirror;
  `routes/mcp.rs` — the extension-page bridge deliberately *not* used for this.
- `crates/host/src/authz/builtin_roles.rs` — where the grants land (and why the wildcards missed
  the dotless verbs).
- `../audit/audit-scope.md` — every undo/redo/compensation stays an audited forward action.
- `key-stack.md` row "Undo / reversible commands".
- `docs/skills/undo/SKILL.md` — owed by the implementing session (drivable surface).
