# Frontend scope — admin console (manage workspaces · teams · users · members · extensions)

Status: scope (the ask). Promotes to `public/frontend/` once shipped. Target stage: **S9+ (collaboration
follow-up)**. The UI counterpart to `auth-caps/admin-crud-scope.md`, `auth-caps/authz-grants-scope.md`,
and `extensions/lifecycle-management-scope.md`. The collaboration slice made the UI a real multi-user app
(login · workspaces · channels · members · inbox · outbox) but every management surface is **create/list
only** — you can add a workspace or a member, never remove one; you can browse the registry but not manage
the full extension lifecycle from a browser. This slice builds the **admin console**: one place to manage
workspaces, teams, users, members, and extensions for real, over the gateway, with the destructive
operations the platform was missing.

It is mostly the **same four-file move the collaboration slice proved**, repeated over the new
destructive/admin verbs — plus the one thing those views lack: **confirm-and-consequence UX** for
operations that delete data or revoke access. The backend verbs are defined in the three sibling scopes;
this scope is the views, the navigation, the session-gated access to them, and the safety UX.

## Goals

- **An admin section, gated by an admin capability** — a `features/admin/` area visible only to a session
  whose token carries an admin cap (the UI checks the cap; the gateway re-checks every verb — UI gating is
  convenience, the server is truth). A non-admin never sees the destructive controls and is denied if they
  forge a call.
- **Workspaces management** — list with status, **rename**, **archive (soft-delete)**, and a guarded
  **hard-delete** behind an explicit confirm. Beside the existing create. The create/list views from
  collaboration are absorbed and extended.
- **Users management** — list users with `active` status; **create** (seeds a dev credential), **disable /
  enable**, **delete** (with grant-revocation consequence shown). The first real user-administration UI.
- **Teams management** — list teams, **create**, **rename**, **delete** (showing member count + the
  cascade consequence). Surfaces `authz-grants`'s team primitive.
- **Members management** — per team: list, **add**, **remove**. Extends the collaboration `MembersView`
  with the missing remove, and shows the live-vs-re-mint revocation note.
- **Roles & grants (read + assign)** — list a workspace's roles and a subject's grants; **assign / revoke**
  a role or cap to a user/team (driving `authz-grants`). Read-first; assignment is the lighter half here,
  the model lives in `authz-grants`.
- **Extensions management** — a real `features/extensions/` console: **list installed** (tier · version ·
  enabled · running · health), **install from catalog**, **upload** a signed artifact, **start / stop /
  enable / disable / restart**, **uninstall** (with confirm). Supersedes the demo `RegistryView` /
  `NativeView`, driven over the new gateway routes so the **browser** can finally manage extensions.
- **Consequence-aware confirm UX for every destructive action** — a shared confirm component that states
  *what is lost* and *what is reversible* (archive vs hard-delete; remove-member's live-vs-re-mint cap
  note; uninstall evicts the binary). Hard-delete requires typing the entity name or a second gate.
- **Drop fakes from the real path; keep them 1:1 for tests** — every new surface follows the channel/
  collaboration contract: `lib/<x>/<x>.api.ts` → gateway route → host verb → `features/<x>/` view+hook,
  with a fake matching the route shape exactly.

## Non-goals

- **No backend verbs.** Every verb is defined in the three sibling backend scopes; this scope **consumes**
  them. If a view needs a verb that doesn't exist, that's a finding for the backend scope, not new work here.
- **No credential/IdP UI.** User create seeds a dev credential; password reset / OIDC / SSO / MFA screens
  are the later pluggable identity scope. No password fields beyond the dev-store minimum.
- **No design-system overhaul.** Reuse the collaboration Tailwind tokens / shadcn primitives; add only the
  controls the admin surface needs (a confirm dialog, a data table, a status badge, a toggle). Visual
  direction stays `frontend-scope.md` / `ui-design-scope.md`.
- **No extension *page* federation.** Mounting an extension's own UI pages is the deferred
  `scope/extensions/` UI-federation scope. This console **manages** extensions; it does not host their pages.
- **No org/tenant-above-workspace admin.** Everything is within the session's workspace (README §7).
- **No analytics/audit dashboard.** A destructive action may show a toast/consequence, but an audit-log
  view is a later scope.

## Intent / approach

**One admin shell, many tables, one safety component.** The console is a navigable section
(`features/admin/`) with a sub-view per entity (workspaces · users · teams · members · roles/grants ·
extensions). Each sub-view is the proven `view + hook + api` triple over the real gateway route. The new
ingredient is a **shared `ConfirmDestructive` component** every delete/disable/remove/uninstall routes
through — it names the consequence, distinguishes reversible (archive/disable) from irreversible
(hard-delete), and escalates the confirm (type-to-confirm or a second gate) for data loss.

```
  features/admin/                         lib/<x>/<x>.api.ts ──► gateway route ──► host verb (sibling scopes)
   ├─ WorkspacesAdmin  (rename/archive/delete)        every action that destroys/revokes
   ├─ UsersAdmin       (create/disable/delete)   ─────────────► <ConfirmDestructive consequence=… reversible=…>
   ├─ TeamsAdmin       (create/rename/delete)                       │ states what is lost / what is reversible
   ├─ MembersAdmin     (add/remove)                                 │ escalates confirm for hard-delete
   ├─ GrantsAdmin      (assign/revoke role|cap)                     ▼
   └─ ExtensionsAdmin  (list/install/upload/start/stop/             proceeds only on explicit confirm
                        enable/disable/restart/uninstall)
            ▲ visible only if session has an admin cap (UI gate); gateway re-checks every verb (truth)
```

- **Cap-gated visibility**: `useSession` already carries the token's caps; the admin section reads them to
  decide what to *show*. The server re-runs the gate on every verb — the UI gate prevents accidental
  display and dead controls, it is never the security boundary. A forged direct call is denied server-side.
- **Each sub-view**: a data table (entities + status), row actions (the verbs), and a create/add control.
  Destructive row actions open `ConfirmDestructive`. State comes from a `use<X>` hook calling the api.
- **`ConfirmDestructive`**: props for `title`, `consequence` (human text: "removes 3 members; team-shared
  docs become unreadable immediately, inherited caps on next sign-in"), `reversible` (archive vs purge),
  and `escalation` (none | type-name | second-gate). One component, every destructive path.
- **Extensions console**: the richest sub-view — a table joining `ext_list` (installed) with the registry
  catalog; row actions map to the lifecycle verbs; an upload control posts a signed artifact to
  `registry_publish`. Replaces the two demo views with one real console.
- **Transport**: `lib/ipc/http.ts` gains the map entries for every new verb (the collaboration slice's
  missing-route lesson); fakes mirror them for Vitest.

**Rejected alternatives:**
- *Add destructive buttons to the existing scattered views.* Rejected — admin operations belong behind a
  cap-gated section with consistent safety UX, not sprinkled across collaboration views where any user
  lands. A dedicated console is discoverable, gateable, and lets the confirm UX be uniform.
- *Trust the UI cap-gate as the boundary.* Rejected outright — the gateway re-checks every verb; the UI
  gate is convenience only. Stating this is load-bearing: a client-trusted admin gate would be a hole
  (mirrors the `authz-grants` "pages are gated callers, never trusted deciders" requirement).
- *A bespoke confirm per action.* Rejected — inconsistent and error-prone; one `ConfirmDestructive` with a
  consequence string and an escalation level keeps every destructive path honest and reviewable.
- *Keep `RegistryView`/`NativeView` and bolt on lifecycle.* Rejected — they're demo-grade and tier-split;
  one unified `ExtensionsAdmin` over `ext_list` (both tiers) is the real console. Retire the demos.

## How it fits the core

- **Tenancy / isolation:** every view operates within the **session's workspace** (the token's hard wall);
  a ws-B admin's console shows only ws-B entities and every verb is ws-scoped server-side. The two-principal
  isolation test extends to the admin surface — ws-B's console can neither see nor mutate ws-A.
- **Capabilities:** the section is **admin-cap-gated in the UI and re-checked per verb on the server**. The
  deny path is real (the gateway returns `Denied`; the view surfaces it). The UI gate is convenience; the
  server is the boundary — explicitly.
- **Placement:** unchanged — gateway role for the browser, Tauri in-process for desktop. Same verbs, two
  transports; the console runs over either.
- **MCP surface:** consumes the sibling scopes' verbs 1:1 (`workspace.delete/rename`, `user.*`, `teams.*`,
  `members.remove`, `grants.assign/revoke`, `ext.*`/`native.*`/`registry.*`). No new verbs minted here.
- **Data (SurrealDB):** none new — the views read/write the records the backend scopes define. The UI holds
  only the session token + transient view state.
- **Bus (Zenoh):** optional live refresh (an "extension started / user disabled" hint) is ordinary motion
  the host already publishes; the console can subscribe to keep tables fresh, but the verbs are state.
- **Stateless extensions:** the console *manages* extensions; their statelessness is what makes
  uninstall/disable safe (all truth in the `Install` record) — the consequence text reflects this.
- **Durability:** read-mostly UI. The one must-deliver action (upload/publish) rides the backend's outbox
  `Target`; the console shows pending→stored, it doesn't own the delivery.
- **One responsibility per file:** one sub-view + hook + api per entity; one shared `ConfirmDestructive`;
  one row-action per handler. No `admin-utils.ts`. Follows `features/<x>/` shape.
- **SDK/WIT impact:** **none** — pure frontend + thin gateway routes over existing host verbs.

## Example flow

1. **Alice (admin)** opens the **Admin** section (visible because her token carries `mcp:workspace.*`/
   `teams.manage`/`user.manage`). **Bob (member)** never sees the section.
2. **Users**: Alice sees `alice (active)`, `bob (active)`, `carol (active)`. She **disables `bob`** — a
   `ConfirmDestructive` (reversible) explains he can't sign in until re-enabled; she confirms. Bob's next
   login is refused.
3. **Members**: Alice **removes `bob` from `facilities`** — the confirm states "team-shared docs become
   unreadable immediately; his inherited caps drop on next sign-in." She confirms; the edge is gone.
4. **Teams**: Alice **deletes `facilities`** — the confirm shows "2 members, removes their inherited caps";
   reversible=false but data-light, single confirm. Done.
5. **Extensions**: Alice opens **Extensions**, sees `hello@v2 (wasm, enabled, running)` and
   `echo-sidecar@v1 (native, enabled, running, restarts 0)`. She **uploads** a signed `hvac@v1`, **installs**
   it, then **disables** it (it stops; reconciler won't auto-start it). She **uninstalls** `echo-sidecar`
   via a hard confirm ("removes the install record and cached binary"); the row disappears.
6. **Workspaces**: Alice **archives** `pilot` (reversible). Later she **hard-deletes** it — typing the
   workspace name to confirm; the second gate (`workspace.purge`) is required; data is destroyed, tombstoned.
7. **Carol (ws-B admin)** opens her console — sees only ws-B entities; cannot touch `acme` anything. The
   wall holds across the entire admin surface.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md`:

- **Capability deny** — a non-admin session does **not** render the admin section, and a **forged** direct
  call to any admin verb is denied server-side (the UI gate is not the boundary — test the server deny).
  Hard-delete requires its escalated gate even for an admin with the soft cap.
- **Workspace isolation** — a ws-B admin console shows only ws-B entities; every destructive verb against a
  ws-A id is denied/empty. Two real sessions, across **gateway + store** — the collaboration two-principal
  test, extended to admin/destructive verbs.
- **Offline / sync** — destructive edits made offline replay idempotently through the real routes; the
  console reflects the synced state (an archived workspace stays archived; a removed member stays removed).

Plus this slice's cases:

- **Confirm UX** — `ConfirmDestructive` blocks the action until confirmed; reversible vs irreversible is
  shown correctly; hard-delete requires type-to-confirm / the second gate; cancel performs nothing.
- **Consequence accuracy** — remove-member shows the live-vs-re-mint cap note; team-delete shows member
  count + cascade; uninstall shows binary eviction. (Copy correctness, tested as content.)
- **Extensions console** — `ext_list` renders both tiers with live state; install/upload/start/stop/enable/
  disable/restart/uninstall each reflect in the table; disable-then-(simulated)-reboot shows not-running.
- **Cap-gated visibility** — toggling the session's caps shows/hides the admin section and individual
  controls (driven by `useSession` caps), while the server gate remains the truth.
- **Vitest per sub-view** — a test per entity view on the fake (mirror `MembersView.test.tsx` /
  `ChannelView.test.tsx`), including the confirm flow; fakes match route contracts 1:1.
- **Rust route parity** — each consumed verb already has a backend route test (sibling scopes); add a
  gateway smoke test that the admin routes are reachable and cap-gated.

## Risks & hard problems

- **The UI cap-gate must never be mistaken for the boundary.** Hiding a button is convenience; the gateway
  re-checking every verb is security. A reviewer (or a future dev) treating the UI gate as the wall is the
  #1 risk — assert it in code comments and test the **server** deny on a forged call, not just the hidden
  control. (This inherits `authz-grants`'s "gated callers, never trusted deciders" rule.)
- **Destructive UX is where users get hurt.** A mis-click that hard-deletes a workspace is unrecoverable.
  The reversible-default + escalated-confirm + accurate-consequence text is the whole safety story; weak
  copy or a missing escalation is a real incident. Treat the confirm component as load-bearing.
- **Consequence text drift.** The live-vs-re-mint cap note, cascade counts, and binary eviction must match
  what the backend actually does; stale copy misleads admins about access state. Keep the strings close to
  the verbs and test them.
- **Surface breadth → fake/route drift.** Many verbs × two transports × fakes; a mismatched fake is a green
  test against a wrong shape (the collaboration lesson, at larger scale). One contract list, names + payloads
  1:1, and a parity check.
- **Retiring `RegistryView`/`NativeView` safely.** They back existing tests; replacing them with
  `ExtensionsAdmin` must port or supersede those tests, not silently drop coverage of install/verify/
  supervise.
- **Build-order coupling.** This consumes verbs from three backend scopes; if a verb isn't built yet, the
  view must fail gracefully (disabled control + "not available") rather than throw `unknown command`. Gate
  views on verb availability, or sequence the backend scopes first.

## Open questions

> **RESOLVED (slice 4, 2026-06-27 — see [admin-console session](../../sessions/frontend/admin-console-session.md)):**
> top-level `features/extensions/` reached from the cap-gated nav; per-control cap checks with the
> section shown if *any* admin cap is present; hard-delete = type-the-name **and** the backend's
> `workspace.purge` cap + confirm token (defense in depth); refetch-after-mutation for liveness;
> read + assign/revoke only (no role editor this slice); `RegistryView`/`NativeView` retired into the
> unified console with coverage ported. The originals are kept below for the record.

> **SUPERSEDED (redesign, 2026-06-27 — see [admin-console-redesign session](../../sessions/frontend/admin-console-redesign-session.md)):**
> the slice-4 console was rebuilt **relationship-first**. The five flat sub-views collapsed to **four
> tabs** (People · Teams · Roles · Workspaces): Members folds inline under a selected Team, and
> grant/role assignment lives in each subject's master-detail panel — so "who belongs to who" is
> browsable, not typed. The **role editor is now BUILT** (resolving the open question below): added a
> real gateway `POST /admin/roles` (`define_role`, no-widening server-side), `roles.list` keeps each
> role's caps, and the UI builds a role by **checking caps** (the admin's own session caps = the
> no-widening set). The chat-style bottom composer was removed everywhere (create = a header action).

- **Where the extensions console lives** — `features/admin/extensions` vs a top-level `features/extensions/`
  reachable from admin. Lean: top-level `features/extensions/` (it's substantial), linked from the admin
  nav, cap-gated the same way.
- **How the admin cap is named/checked in the UI** — a single `is_admin` derived from any of the admin caps,
  vs per-control cap checks. Lean: per-control cap checks (show only what the session can do) with a section
  shown if *any* admin cap is present — finer-grained, matches server gates.
- **Hard-delete confirm mechanism in the UI** — type-the-name vs a re-auth/second-gate prompt vs both. Lean:
  type-the-name **and** rely on the backend's second gate (`workspace.purge`) — defense in depth.
- **Live table refresh** — subscribe to a host "admin changed" motion vs poll-on-action vs refetch-on-focus.
  Lean: refetch after each mutation (simple, correct) + optional subscription later for multi-admin liveness.
- **Roles/grants depth in v1** — full role editor vs read + assign/revoke only. ~~Lean: read +
  assign/revoke only this slice~~ **RESOLVED (redesign):** the **full role editor shipped** — define a
  role by checking caps (no-widening), assign named roles from a dropdown. Remaining: `roles.delete` and
  union-resolved *effective* caps in the People detail (needs a `resolve_caps` gateway verb).
- **Graceful degradation when a backend verb is absent** — hide the control vs show-disabled-with-reason.
  Lean: show-disabled-with-reason during the build-out so the console is honest about what's wired.

## Related

- `scope/auth-caps/admin-crud-scope.md` — the destructive workspace/user/team/member verbs this UI drives.
- `scope/auth-caps/authz-grants-scope.md` — the roles/grants/teams model this UI reads and assigns into;
  the **freshness asymmetry** the consequence text must reflect; the "gated callers, never trusted
  deciders" rule this UI inherits.
- `scope/extensions/lifecycle-management-scope.md` — the full extension lifecycle + gateway routes + upload
  the extensions console consumes.
- `scope/frontend/collaboration-scope.md` — the session/login + the four-file move + the two-principal
  isolation test this extends; the `MembersView` this absorbs and completes.
- `scope/frontend/frontend-scope.md` + `scope/frontend/ui-design-scope.md` — the shell + visual direction
  the console reuses (no overhaul).
- `scope/tenancy/tenancy-scope.md` — the workspace wall the console holds per session.
- README **§6.6** (identity/auth/caps), **§6.13** (frontend / extension UIs), **§7** (tenancy).
</content>
</invoke>
