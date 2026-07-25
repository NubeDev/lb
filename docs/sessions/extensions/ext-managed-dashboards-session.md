# Session — managed dashboards: a `managedBy` marker, the admin-override triad, and a typed denial

- **Scope:** [`scope/extensions/ext-managed-dashboards-scope.md`](../../scope/extensions/ext-managed-dashboards-scope.md)
  (Ask 2 — §Goals 2–5, decisions D1–D5). Ask 1 (the `ui-v0.15.0` SDK tag) was already cut; untouched here.
- **Builds on:** [`scope/extensions/native-caller-identity-scope.md`](../../scope/extensions/native-caller-identity-scope.md)
  (why a sidecar's own `sub` is `ext:<id>`) and the shipped `dashboard.delete_any` override.
- **Consumer:** `NubeIO/rubix-ai-extensions → extensions/modbus/docs/scope/device-dashboards-scope.md` (product
  motivation); `NubeIO/rubix-ai → docs/scope/frontend/managed-dashboard-badge-scope.md` (the shell half).
- **Stage:** S10 (extensions). **Status:** host half shipped — `cargo build --workspace` green, 11 new
  headless tests green against a real store.
- **Date:** 2026-07-25.

## The ask, restated

A dashboard an extension generates (`owner = "ext:modbus"`) was a second-class citizen: indistinguishable
from a hand-made board in the roster, un-editable by every human **including an admin** (`dashboard.save`
was owner-only with no override while `dashboard.delete` had one), and it answered an operator's drag-and-save
with a bare `Denied`. Three additive fixes, no new authority for extensions.

## What shipped

**1. `managedBy` on the record (Goal 2 / D1).** `Dashboard.managed_by: String`, `rename = "managedBy"`,
`#[serde(default, deserialize_with = "null_default")]` — the crate's existing additive-field convention, so a
pre-field record round-trips with no migration. It holds the **bare** extension id (`"modbus"`); the full
principal already lives on `owner` (`"ext:modbus"`). Relayed on `DashboardSummary` (D3) so a roster badges and
filters without a full `get`.

It is **derived from the saving principal, never accepted as input**. The derivation lives in exactly one
file — `dashboard/managed.rs::managed_by_of(principal) -> Option<String>` — which strips the `ext:` prefix off
`principal.owner_sub()`. Matching that prefix is a shape match on an identity *form*, not a branch on which
extension (rule 10); a future identity scheme changes this one function. `owner_sub()` (not `sub()`) is
deliberate: it is the same identity `save` stamps on `owner`, so the two fields can never disagree.

`dashboard.save` derives the marker on **create** and **preserves** it on update — an admin's `save_any` fix
can neither blank it nor steal it, and a human re-save of an unmanaged board cannot acquire one. `dashboard.pin`
(the other record-creating verb) uses the same helper. No verb reads a `managedBy` argument, so a human sending
one through the MCP surface is silently ignored.

**2. The admin-override triad (Goal 3 / D2).** `dashboard.save_any` and `dashboard.share_any`, each mirroring
the shipped `dashboard.delete_any` verbatim: a catalog row in `system/catalog.rs`, an entry in
`authz::builtin_roles::ADMIN_ONLY_CAPS` (so only the `workspace-admin` bundle carries it), and the check in the
same **position** —

```rust
if existing.owner != principal.owner_sub()
    && authorize_dashboard(principal, ws, "dashboard.save_any").is_err()
{ return Err(...); }
```

— owner first, override attempted strictly second via `&&`'s short-circuit, each its own capability, never
folded into an ambient admin-role test. Neither is a bypass of the verb's base cap: gate 2
(`mcp:dashboard.save:call` / `.share:call`) still runs first, so an `*_any` cap widens *whose* assets the verb
reaches, never *whether* the caller may call it. Both are asserted by tests.

**3. The typed managed-denial (Goal 5).** `DashboardError::ManagedDenied(String)`, `Display` =
`denied: managed=<ext id>`. It is produced by one helper, `save::managed_denial`, under two conditions: the
board is marked **and** the caller could already *read* it (`may_read_dashboard` — gate 3). Every other refusal
stays the opaque `Denied`, so a caller who cannot read a private managed board learns nothing from the refusal
— **no existence leak**. Over MCP it becomes a new generic variant `ToolError::DeniedBecause { code, subject }`
(`code = "managed"`, `subject = "<ext id>"`), which the gateway maps to `403` with that `Display` as the body.

## Decisions taken that the scope did not spell out

- **A new generic `ToolError` variant rather than a dashboard-shaped one.** The browser's save goes
  REST → `call_tool` → `ToolError`, so the typed denial had to survive that hop or die at the boundary. It is
  modelled on the existing `Ambiguous` precedent — structured detail is safe precisely because the variant is
  unreachable by an unauthorized caller — and it carries an opaque `code`/`subject` pair the MCP layer never
  interprets, so no dashboard/extension concept leaks into `lb-mcp`.
- **Every other consumer of that variant collapses it to its opaque denial.** The wasm guest bridge
  (`callback.rs`), the agent-run dispatcher, the CLI local transport, and the datasources/flows/rules/undo
  routes all treat `DeniedBecause` exactly as `Denied`. Only the dashboard REST route and the generic
  `/mcp/call` relay the detail — widening is opt-in, per route.
- **`share` denies opaquely, `save` denies typed.** The scope's Goal 5 names the save path ("when a save is
  refused"), and the UI affordance is "duplicate to edit", which is a save-path answer. A typed share denial has
  no corresponding client action, so it was not invented.
- **`nav::resolve` strips a `ManagedDenied` item like any denial** rather than faulting the menu. A read never
  produces the variant today; the arm exists so a future one cannot take a workspace's nav down.

## Not done (deliberately, per the scope's non-goals)

No host interpretation of `managedBy` (no extension lookup, no lifecycle coupling); uninstall does **not**
cascade-delete a board (D5); no pack coupling; no merge/rebase of human edits over a regenerated board —
overwrite stays the contract and the marker is what makes it visible.

## Tests (real store, real caps, no fakes)

Two new headless suites, 11 tests, all green (split by responsibility per FILE-LAYOUT):

- `crates/host/tests/dashboard_override_test.rs` (3) — the `save_any`/`share_any` deny-then-allow pairs
  mirroring `delete_any`'s test shape, plus the check-order assertions (owner path first; an override is never
  a bypass of gate 2) and the "an admin fix is not a takeover" assertions on `owner`/`managedBy`/`visibility`.
- `crates/host/tests/dashboard_managed_test.rs` (8) — marker provenance (a human cannot set it through the MCP
  surface; an extension always gets it; `ext:a` cannot save over `ext:b`'s board), workspace isolation of a
  managed board including from a same-named `ext:modbus` principal in the other workspace, the pre-field
  round-trip (a raw record written with **no** `managedBy` key, plus the explicit `null` shape), the
  `DashboardSummary` relay, and the denial shape at both the host and MCP layers.

The hot-reload item from the scope's plan belongs to the consuming extension's run, not this host slice.

## Pre-existing failures observed (NOT caused here, NOT fixed here)

Both reproduce on a clean tree with every change stashed:

- `lb-host` unit test `system::catalog::tests::host_catalog_covers_dispatch_prefixes` — the dispatcher handles a
  `forms.` prefix that has **no** row in `HOST_TOOLS`. The forms family's catalog rows are missing.
- `lb-role-gateway` `tests/apikey_routes_test.rs` — 6 of 8 fail at the `create_key` helper (`/admin/apikeys`
  does not return `200`).
