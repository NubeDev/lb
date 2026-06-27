# Frontend — admin console UI (session)

- Date: 2026-06-27
- Scope: ../../scope/frontend/admin-console-scope.md
- Stage: S9+ — **slice 4 of 4** (admin-CRUD/lifecycle/console). Builds on slices 1–3
  ([authz-grants](../auth-caps/authz-grants-session.md), [admin-crud](../auth-caps/admin-crud-session.md),
  [lifecycle-management](../extensions/lifecycle-management-session.md)).
- Status: done (UI: shared confirm + admin section + extensions console + nav; demos retired)

## Goal
Build the **admin console** — the UI that finally drives the destructive/admin verbs slices 1–3 shipped
on the backend. One cap-gated `features/admin/` section (Workspaces · Users · Teams · Members ·
Roles/Grants) + a top-level `features/extensions/` console (both tiers, lifecycle), every destructive
path routed through **one shared `ConfirmDestructive`**. Consume the `http.ts` verbs + 1:1 fakes that
already exist; mint **no new backend verbs** (Non-goal). Retire the demo `RegistryView`/`NativeView`
into the unified extensions console without dropping coverage.

## What changed
**The safety keystone — one shared confirm** (`features/confirm/ConfirmDestructive.tsx`): props
`title` · `consequence` · `reversible` · `escalation: none|type-name|second-gate` (+ `confirmName`).
EVERY delete/disable/remove/uninstall routes through it. Blocks until an explicit, satisfied confirm;
`type-name` requires typing the entity id (workspace purge); `second-gate` an ack checkbox (uninstall);
Cancel does nothing. It is the human safety net, **not** the security boundary.

**Caps surfaced to the UI** (the one thin plumbing change): `LoginReply` gained `caps: Vec<String>`
(gateway `routes/login.rs`) so the UI can cap-gate **display**. `Session` gained `caps?: string[]`;
new `lib/session/admin-caps.ts` (`CAP` map, `ADMIN_CAPS`, `hasCap`, `isAdmin`) mirrors the gateway's
dev `member_caps()`. The session fake returns `ADMIN_CAPS` (dev-login = admin, matching `dev_claims`).
**Load-bearing comment + tests:** the UI gate is convenience; the **gateway** re-checks every verb and
the forged-call deny is already proven in Rust (`role/gateway/tests/admin_routes_test.rs`).

**The admin section** (`features/admin/`) — one sub-view + hook per entity, the proven view+hook+api
triple over the existing `http.ts`/fake:
- `WorkspacesAdmin` — list (node directory) + archive (reversible, single confirm) + **purge**
  (type-the-name escalation; backend also needs `workspace.purge` cap + confirm token == id).
- `UsersAdmin` — list (active status) + create + disable (reversible confirm) + delete (type-name,
  grant-revocation consequence shown).
- `TeamsAdmin` — list + create + rename + delete (member count read **live** so the cascade copy is
  accurate + the consequence).
- `MembersAdmin` — list + add + **remove** (the freshness-asymmetry consequence: docs unreadable
  immediately, inherited caps drop on next sign-in). Completes the collaboration `MembersView`.
- `GrantsAdmin` — read a subject's grants + assign/revoke (reversible confirm). **Read + assign/revoke
  only — NO role editor** this slice (the resolved open question).
- `AdminView` — the tabbed shell; each tab gated on its controlling cap (per-control gating, the
  scope's lean), shown if the session carries *any* admin cap.

**The extensions console** (`features/extensions/`) — `ExtensionsView` + `useExtensions` over the
`ext_*` verbs: lists **both tiers** with tier · version · enabled · running · health · restart count;
start/stop via enable/disable (reversible confirm); uninstall (second-gate, binary-eviction
consequence). Install-from-catalog/upload deferred this pass (scope allows it). **Retired**
`features/registry` + `features/native`; their lifecycle/restart-count/both-tier coverage is ported
onto `ExtensionsView.test.tsx`.

**Nav** (`App.tsx` + `shell/NavRail.tsx`) — `admin` + `extensions` surfaces added, **cap-gated**:
`NavRail` takes an `allowed` list; App computes it (`isAdmin(caps)` → admin; `hasCap(extList)` →
extensions). A plain member never sees them.

## Follow-on this session (upload + the visibility fix)
After the first pass, two gaps surfaced when running the real UI:
- **The Extensions section was hidden** — the gateway's dev claims (`member_caps()` in
  `role/gateway/src/session/credentials.rs`) never granted `mcp:ext.list:call`, so `hasCap(extList)`
  was false on the real token and the nav entry never showed. **Fixed:** added `mcp:ext.list:call`,
  `mcp:ext.disable:call`, `mcp:ext.uninstall:call`, `mcp:ext.publish:call` to the dev admin claims
  (re-login picks them up). Also added a **dev-only seed** (`seedDevExtensions` in `ext.fake.ts`, called
  from `useExtensions` only when there's no gateway and not under test) so the no-gateway demo build
  shows the two reference extensions instead of an empty list. Verified extensions are **per-workspace**
  (`ext_list(node, caller, ws)` authorizes + reads per ws; the fake keys installs by `ws()`).
- **Upload was deferred** — now **shipped end to end**. The host verb `lb_host::ext_publish`
  (`crates/host/src/ext/publish.rs`, verify-before-store, workspace-first, gated `mcp:ext.publish:call`)
  and the gateway route `POST /extensions` (`role/gateway/src/routes/ext.rs::publish_extension`, `204`
  ok / `403` deny / `422` verification failure) already existed; this session wired the **UI**:
  `ext_publish` in `http.ts` (POST `/extensions` with the `Artifact` body verbatim), `publishArtifact`
  + the `Artifact` type in `lib/ext/ext.api.ts`, a focused `UploadArtifact.tsx` control (file-pick →
  parse via `FileReader` → publish; the UI never mints/signs — it transports a publisher-produced signed
  artifact), and the `ext_publish` case in `ext.fake.ts` (verify-before-store mirror via a `__trusted`
  flag — a tampered upload installs nothing). **Per-workspace install** (the resolved decision): an
  upload verifies then installs into the *current* workspace; ws-B never sees ws-A's upload.

## Decisions (open questions resolved)
- **Extensions console placement:** top-level `features/extensions/` (it's substantial), reached from
  the cap-gated nav — the scope's lean.
- **Admin cap check:** per-control cap checks; the section shows if *any* admin cap is present.
- **Hard-delete confirm:** type-the-name in the UI **and** the backend's `workspace.purge` cap +
  confirm token (defense in depth).
- **Live refresh:** refetch-after-mutation (simple, correct); a subscription is a later option.
- **Roles/grants depth:** read + assign/revoke only; the role editor is a follow-up.
- **Demos:** retired into one unified console; coverage ported, not dropped.

## Tests (green)
`cd ui && npx tsc --noEmit` → clean. `npx vitest run`:
```
 Test Files  21 passed (21)
      Tests  59 passed (59)
```
(56 after the first pass; +3 for the upload cases — verified upload appears installed, tampered upload
rejected/nothing installed, malformed file errors locally.)
(was 40 before this slice; +16 net after retiring the two demo suites and adding the console + admin
suites.) New/changed suites:
- `ConfirmDestructive.test.tsx` — blocks until confirmed; reversible vs irreversible shown;
  type-to-confirm for purge; second-gate for uninstall; **cancel performs nothing**.
- `UsersAdmin` / `TeamsAdmin` / `MembersAdmin` / `WorkspacesAdmin` / `GrantsAdmin` — per sub-view on
  the fake (mirror `MembersView.test.tsx`), each through the confirm flow + the accurate consequence
  copy; MembersAdmin keeps the **workspace-isolation** case.
- `ExtensionsView.test.tsx` — both tiers render with live state + the native restart count; stop
  (disable) → not-running; uninstall (second gate) evicts the row; ws-isolation. (Ports the retired
  Registry/Native lifecycle coverage.)
- `AdminView.test.tsx` + `App.test.tsx` — **cap-gated visibility**: toggling caps shows/hides the
  tabs and the nav entries; a plain member sees neither Admin nor Extensions.

Rust side unchanged except the `LoginReply.caps` field — `cargo build -p lb-role-gateway` green;
`cargo test -p lb-role-gateway --test admin_routes_test` → 4 passed (the **server deny on a forged
call** still holds — the boundary is the gateway, not the UI).

Pre-existing unrelated failures (NOT touched): `github_bridge_normalize_test` + a couple native tests
need a prebuilt wasm/sidecar binary absent in this checkout.

## File layout
One component per `.tsx`, one hook per `use<X>.ts`, one verb-group per `*.api.ts`; barrels are
re-export only. Largest new file 127 lines (`UsersAdmin.tsx`). No `admin-utils.ts`/`helpers.ts`. The
shared confirm is the one cross-cutting component, by design.

## Follow-ups
- ~~Signed-artifact upload in the extensions console~~ **SHIPPED this session** (UI + gateway route +
  host verb, verify-before-store, per-workspace install). Install-**from-catalog** (browse a registry
  and install) is still open.
- **Extension UI federation** — mounting an extension's OWN pages in the shell — now **scoped**:
  `docs/scope/extensions/ui-federation-scope.md` (module federation for trusted publishers, iframe
  sandbox for untrusted, host-mediated MCP bridge). The deferral the admin-console Non-goals named.
- A role editor (`roles.define` UI) — the model lives in authz-grants.
- Live multi-admin refresh via a host "admin changed" subscription (refetch-after-mutation ships now).
- Gateway/Tauri wiring already carries these verbs; the Tauri desktop shell's session is the remaining
  transport gap (carryover item 5).

## Related
- scope: ../../scope/frontend/admin-console-scope.md (open questions resolved here)
- backend slices consumed: ../auth-caps/authz-grants-session.md,
  ../auth-caps/admin-crud-session.md, ../extensions/lifecycle-management-session.md
- the collaboration UI this extends: ../frontend/collaboration-session.md
