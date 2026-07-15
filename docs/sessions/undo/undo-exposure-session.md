# Undo exposure — hardening, grants, routes, client seam + the §2.3 sync proof

- Date: 2026-07-15
- Scope: ../../scope/undo/undo-scope.md
- Stage: S10 (cross-cutting retrofit), per STAGES.md
- Status: backend + client seam shipped and green; the shell affordance (toolbar/shortcut) NOT built

## Goal

Take undo from host-internal to product-reachable: two hardening fixes the scope's own risks called
out, the role grants that make the verbs reachable by a member, gateway routes, the app-SDK client
seam — and the **§2.3 sync proof the scope has owed since the build session**.

Picked up from a handover. **The handover was substantially wrong about repo state** and that is
worth recording: it described all of this as uncommitted work on `feat/store-online-compaction-67`,
but that branch was already merged (PR #70) and every file it described as pending was committed and
clean on `master`. It also claimed `ui/` had no `src`. `ui/src` does exist — but holds **zero**
TypeScript files and nothing git-tracked, so its *conclusion* (no web shell here) was right by
accident. Verify state before trusting a handover's plan; the plan was built on a false premise.

## What changed

### Hardening (both were real, both are now regression-guarded)

- **Read-error-as-absence** (`host/src/undo_capture/decide.rs`): a failed before-image read used to
  flatten `Err → None` via `.ok()` and journal the step as a *create* — whose undo DELETES a live
  record. `decide()` is now the pure outcome table: only `BeforeRead::Read(absent)` is an undoable
  create; `BeforeRead::Failed` is `NotUndoable`. Nobody observed the prior state, so nothing may be
  restored over it.
- **Prune floor** (`crates/undo/src/prune.rs`): `push_do` is now `#[must_use]` and returns the seqs
  that fell past the depth cap; `save_stack_pruning` commits the trimmed cursor and DELETEs each
  `undo:{seq}` + `undo_live:{seq}` in ONE transaction. Without it the cursor trimmed but the events
  stayed — an unbounded journal. `depth_cap: Option<usize>` was added to the three `Record*` structs
  so this is provable in 3 writes instead of 101 (`None` = `DEFAULT_DEPTH_CAP`, so behaviour is
  unchanged for every caller).

### Grants, routes, client

- `builtin_roles.rs`: `mcp:undo:call`, `mcp:redo:call`, `mcp:history.compensations:call` → member
  (author tier); `mcp:undo.any:call` → admin-only. The dotless verbs match no `mcp:*.<verb>:call`
  wildcard, so they must be named concretely.
- **Behaviour change to know about:** `history_compensations` used to authorize on `history.list`;
  it now authorizes on its own `history.compensations` verb. That is what makes the new member grant
  meaningful, but a caller holding only `history.list` loses access to it.
- `role/gateway/src/routes/undo.rs`: `POST /undo`, `POST /redo`, `GET /undo/history`,
  `GET /undo/history/{seq}/compensations`. Workspace + principal from the token (§7); caps
  re-checked via `lb_host::call_tool`; typed `ok:false` refusals pass through as `200` data while a
  true authorization failure stays an opaque `403`. Body accepts `surface` only — no `actor`: the v1
  shell never does cross-actor undo, admins use MCP.
- `app/sdk`: four verbs added to the `invoke.ts` verb→route switch (the file's own stated pattern —
  extend the map, never fork it), plus `undo/undo.types.ts` for the wire shapes.

### The §2.3 sync proof (`crates/host/tests/undo_sync_test.rs`)

The load-bearing claim: **the conditional restore is enforced where it applies, not where it was
captured.** Two real nodes, two real independent stores. The edge captures an undo; the hub's copy
moves on; the carried undo is REFUSED at the hub and the hub's value survives.

The test carries the edge's real journal rows to the hub itself (real `scan` + real `write`),
because **there is no journal replication in the product**: `ChannelSync` is the only cross-node sync
and it mirrors inbox `Item`s only — no doc replication, no journal replication. So the scope's
literal scenario cannot arise today. The transport is stubbed; the mechanism under test is not. This
is written into the test's header so the next reader doesn't mistake it for a shipped sync path.

## Findings worth keeping

- **The first version of the §2.3 test passed vacuously.** The hub had no journal, so `apply_undo`
  returned `Empty` and no predicate was ever evaluated — a green test proving nothing. The assertion
  is now strictly `Stale` and never `Empty`, with a comment saying why.
- **Then it passed for the wrong reason again:** with one intervening hub write the revs aligned by
  coincidence, the predicate matched, and the undo *applied*. Two writes make the divergence
  unambiguous. A control test (same carry, unmoved hub → undo APPLIES) is what proves the refusal is
  caused by the intervening write and not by the undo merely arriving at a different store.
- **The predicate enforces in two layers** — a read-only pre-check and an in-transaction
  `THROW 'stale'`. Deleting only the pre-check leaves every test green; both must be broken to see
  red. Worth knowing before concluding a staleness test is vacuous.
- `scan` returns the storage envelope (`{rev,data}`) under a qualified id (`undo:1`) while `write`
  wants the inner value under a bare id. Carrying rows without unwrapping both silently produces an
  unreadable stack.

## Tests (all revert-checked — each was confirmed to FAIL when its bug is reintroduced)

| Suite | Result |
|---|---|
| `lb-undo` (incl. new prune floor) | 10 passed |
| `lb-host` undo_test | 5 passed |
| `lb-host` undo_sync_test (§2.3) | 3 passed |
| `lb-host` lib: `decide` | 6 passed |
| `lb-host` lib: grants | 1 passed |
| `lb-role-gateway` undo_routes_test | 8 passed |
| `app/sdk` undo.gateway.test.ts (real node) | 6 passed |

`cargo build --workspace` clean; `cargo fmt --check` clean; `app/sdk` typecheck clean.

Existing test call sites needed `depth_cap: None` added (3 in `crates/undo/tests/undo_test.rs`, 1 in
`crates/host/tests/undo_test.rs`) — the merged struct change had broken them, so the suite did not
compile on master.

`app/sdk/tests/caps-deny.gateway.test.ts::denies_channel_post_without_the_pub_grant` fails on **clean
master** (verified by stashing everything and re-running) — pre-existing, not from this work.

## Not done

- **The shell affordance.** No toolbar button, no Ctrl+Z, no toast. `app/shell` is a thin login →
  full-screen extension mount: no toolbar, no dock, no global shortcut handling, no `hasCap`. This is
  new code, not wiring, and it needs a design decision first (below).
- **The focus contract** (the scope's open question). `packages/ce-wiresheet`'s `CeEditor` owns its
  own Ctrl+Z (`CeEditor.tsx:1209-1260`, a window listener guarding only on `activeElement` being an
  input) and advertises ownership to nobody. Recommendation, not yet decided: a DOM
  `data-owns-undo` attribute on the editor root, with a shell handler bailing on
  `document.activeElement.closest('[data-owns-undo]')` — ce-wiresheet loads as a federated remote, so
  a JS registry isn't shared across the module-federation boundary, but the DOM is.
- A jsdom vitest project for a focus/DOM test (the gateway suite is `environment: "node"`).
- `doc-site/content/public/` promotion — deferred until the shell half lands and the UX stops moving.

## Related

- ./undo-build-session.md — the core build (store `rev`, `lb-undo`, host verbs)
- ./undo-autocapture-session.md — auto-capture on dispatch
