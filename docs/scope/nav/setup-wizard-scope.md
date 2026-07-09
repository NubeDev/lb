# Setup scope — the onboarding wizard (guided "add a person" flow)

Status: **SHIPPED (frontend, 2026-07-09)** — a new **Setup** tab in the Access console. Pure
frontend orchestration over already-shipped host verbs; **no new backend**. See
[`sessions/nav/setup-wizard-session.md`](../../sessions/nav/setup-wizard-session.md). Builds on the
shipped `user.*`, `teams.*`, `members.*`, `grants.*`/`roles.*`, and `nav.*` verbs.

---

## The ask

An admin needs a **guided, low-ceremony way to onboard one person** without hopping between the
People, Teams, Roles, and Nav tabs and knowing which order to do them in. A stepped wizard that:

1. **Creates (or picks) a user** and, in the same flow,
2. **puts them on a team**,
3. **gives that team a role (access) + a nav (menu)**, and
4. **previews the exact access** the person will get — a live "what will they see" demo.

Emphasis on **nice UX**: a clear step rail, one decision per step, and an honest preview.

## Intent / approach

**The wizard is pure orchestration — it invents nothing.** Every write is one of the *same* real host
verbs the individual admin tabs already call:

| Step | Verb(s) | Same as tab |
| --- | --- | --- |
| Person | `user.create` | People |
| Team | `teams.create`, `members.add` | Teams |
| Access | `grants.assign` (`role:<name>` → **team**), **`nav.save`** (build a nav inline), `nav.share` (team tier), `nav.set-default` | Roles + Nav |

The Access step can **build a brand-new nav in-place** (assign page access without leaving onboarding),
reusing the Nav tab's editor verbatim — see *Shared nav composer* below — or reuse an existing nav, or
keep the built-in sidebar.

### Shared nav composer (reuse, not duplication)

The nav builder's menu editor (title + ordered items + the add-item form over surfaces/dashboards/ext
pages/tag-group/template-group/group) was extracted into a common **`NavItemsBuilder`**
(`features/admin/nav/NavItemsBuilder.tsx`). It's a controlled editor over `(title, items)` with **no**
persistence or sharing of its own. Both `NavAdmin` (the tab) and the wizard's Access step render it, so
the item grammar has **one source of truth** — a new item kind added there appears in both places. The
tab keeps its "Who sees this nav" section; the wizard shares the built nav to the team it's onboarding.
| Preview | `authz.resolve` (read-only) | Access console (effective caps) |

Access is granted to the **team**, not the user — so every current and future member inherits it (the
"give the ops team these pages" shape from the [nav scope](nav-builder-scope.md): *a role grants; a nav
shapes*). The nav is a **lens** — sharing it grants nothing (rule 5); the gateway re-checks every verb.

**The preview is honest, not decorative.** It calls `authz.resolve` for the target user and mirrors the
**two** real rendering paths in `NavRail`:

- **A nav applies** (the wizard built/picked one) → the real rail renders **only that nav's items**,
  each cap-stripped (`resolvedMenu` *replaces* the surface list). So the preview strips the nav's items
  the same way and renders **those** — a 1-item nav previews as 1 item.
- **No nav** (built-in sidebar) → the rail renders `allowedSurfaces(caps)`; the preview shows that
  fallback set.

**Bug fixed (2026-07-09):** the first cut always rendered `allowedSurfaces(caps)` — so a 1-item nav
still previewed the whole cap-allowed set (a `member` role's 161 caps → all 15 pages), which is *not*
what the person would see. The preview now takes the applied nav's `items` and strips them exactly like
the server resolver, so preview and rail agree. Reusing the shell's own gates (`allowedSurfaces`,
`SURFACE_DEF`, `hasCap`) — not a re-implementation — keeps them from drifting. Still a preview of the
display lens; the server re-checks every verb.

## How it fits the core

- **No new authz, no new asset, no new verb** — rule 5/10 hold for free because the wizard only calls
  existing gated seams. It never branches on an extension id (nav `ext` items stay opaque, rendered
  through the shell's existing surface map).
- **Capabilities (rule 5):** the Setup tab shows for a caller holding **any** of `user.manage`,
  `teams.manage`, or `grants.assign` (display gate). Each step's write is re-checked server-side; a
  caller without the cap is refused at the gateway regardless of what the UI shows (tested).
- **Workspace wall (rule 6):** every verb is workspace-scoped by the session token; a person onboarded
  in ws-A is invisible in ws-B (tested).
- **Placement / FILE-LAYOUT:** `ui/src/features/admin/setup/` — one responsibility per file:
  `SetupWizard.tsx` (flow), `useSetup.ts` (verb orchestration hook), `Stepper.tsx` (step rail),
  `PickOrCreate.tsx` (reusable pick-or-create control), `AccessPreview.tsx` (the live preview),
  `previewReach.ts` (the pure caps→surfaces lens, reusing `allowedSurfaces`).

## Non-goals

- **No new backend / verbs / caps.** If a step needs a capability, that capability already exists.
- **No cohort/bulk import** — one person at a time (the "Onboard another" reset keeps the team/role/nav
  so a cohort is *fast*, but each person is a pass).
- **No per-user nav share** — access is a team property (a nav has no per-user audience; the wizard
  routes single-user shares through a team, matching the nav scope).
- **No new nav authoring** — the Access step *picks* an existing nav; building one stays in the Nav tab.

## Testing plan (shipped)

Against the **real** spawned gateway (`pnpm test:gateway`), no mocks (rule 9):

- **End-to-end onboard** — create user → create team + add → grant role to team + share nav → the
  preview resolves the user's **effective** caps and shows the granted page + its provenance chip; the
  resolver confirms the inherited cap. ✅
- **Capability deny (mandatory)** — a caller with only `nav.resolve` gets no wizard (display gate) AND a
  raw `grants.assign` is refused server-side. ✅
- **Workspace isolation (mandatory)** — a user created in ws-A is invisible in ws-B; resolving them in
  ws-B yields no caps. ✅

## Risks

- **Preview honesty** — the one bug that matters is a preview that shows access the person won't have
  (or hides access they will). Mitigated by reusing `allowedSurfaces` verbatim and resolving effective
  caps server-side; the preview is a lens over the real gate, never a second source of truth.
- **Reach caps vs. resolve** — `authz.resolve` returns granted caps, not the login-folded `reach:*`
  caps a curated nav mints at sign-in. The preview therefore shows the **cap-allowed** page set (an
  honest upper bound of "pages this person can open"), which is the onboarding question. Documented in
  `previewReach.ts`.
