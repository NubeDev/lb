# auth-caps scope — share the dashboard's closure (panels follow the page, on an explicit offer)

Status: **BUILT** (lb side) — the verb, the cap, the MCP wiring, and the tests are in
(`dashboard/share_closure.rs`, `dashboard/closure.rs`, `tests/dashboard_share_closure_test.rs`).
Promotes to `doc-site/content/public/dashboard/` once the UI ships. **Owning repo: `lb`** (a new host
verb + a `dashboard.share` follow-through). rubix-ai then bumps the `lb-node` tag and builds the guided
UI (see rubix-ai `scope/frontend/dashboard/share-closure-ui-scope.md`).

> **Build notes — what the implementation discovered (read before extending):**
>
> 1. **A pre-existing `access_check` bug fell out of the dual-consistency test.** Preflighting for a
>    `team:` subject reported every team-shared asset RED ("not shared to the subject") — a false red on
>    the correctly-shared case, in the parent scope's headline path. Gate 3 asks "is this principal a
>    `member` of a team the asset is shared to?", and the preflight handed it `sub = "team:ops"`, i.e.
>    "is `team:ops` a member of `team:ops`?" — never true. Fixed via `access_check::gate3_identity`
>    (ask as a representative member). Full writeup:
>    `debugging/auth/access-check-team-subject-false-red.md`. **This is the scope's central bet paying
>    off:** the dual test caught what neither verb's own tests could see.
> 2. **The `not_owned` gap cannot be constructed the obvious way.** `dashboard.save`'s
>    `validate_and_strip_refs` requires every `panel_ref` to resolve **under the saver**, so nobody can
>    embed a panel they cannot read. The gap arises exactly as the Example Flow describes — the panel's
>    and the page's audiences DIVERGE (aidan shares his panel to `design`; ada shares the page to
>    `ops`). The tests build it that way.
> 3. **The team-read probe must skip the panel's OWNER.** `may_read_panel` short-circuits `Ok` for the
>    owner before consulting any share edge, so probing "can this team read it?" as an owner-who-is-
>    also-a-member answers yes for a panel shared with nobody — masking a real gap as `already_shared`.
>    `team_can_read` probes as a non-owner member.
> 4. **Defense in depth is real, and proven.** With the ownership check deliberately removed from the
>    disposition logic, the no-widening test still failed — `panel_share`'s own owner rule refused the
>    write. The report could lie; the wall did not move. That is precisely why the write goes through
>    `panel_share` verbatim instead of `relate()`.
> 5. **A team has TWO identities, and the verb must use the graph's one.** The S4 `member`/`share`
>    edges key on the **bare** id (`member__ops__user:bob`, `share__ops-page__ops`); `team:ops` is the
>    GRANT store's identity (`Subject::as_key()`). `normalize_team` originally canonicalized to the
>    `team:` form, so the verb wrote `share__panel-…__team:ops` — an edge that dead-ends at gate 3's
>    member hop. **It reported `shared` and the user's widget stayed "Panel not accessible."** All 15
>    tests passed, because they seeded membership with `team:ops` too — the fixture agreed with the
>    fixture and disagreed with the live system. Found in a browser in 90 seconds.
>    `debugging/dashboard/share-closure-team-prefix-mismatch.md`. Guarded by
>    `writes_the_share_edge_on_the_same_team_id_the_member_and_dashboard_edges_use`, which asserts the
>    edge is byte-identical to `dashboard.share`'s for the same team. **Do not "normalize" a team id.**

A dashboard is a composite: the page record plus a transitive closure of **library panels** it embeds
(and their sources). Today `dashboard.share` shares only the page record. So a member who can open a
team-shared page still sees **"Panel not accessible"** for every embedded library panel that is still
`private` — the page came through, the widgets did not. `dashboard.access_check` already *detects* this
exact gap (its module doc cites the live `user:bob` → `panel:aidan` 403); what is missing is the
**remediation**: a first-class way to bring the closure's sharing up to the page's audience. We want a
`dashboard.share_closure` verb that, given a dashboard and a target team, shares every **eligible**
embedded panel to that team — as an **explicit, capability-bounded action**, never a silent side effect
of embedding or of sharing the page.

## Goals

- One verb — `dashboard.share_closure(dashboard, team)` — that shares every library panel in a
  dashboard's closure to `team`, so a team handed a page also gets its widgets.
- **Eligible only**: a panel is shared iff the caller is its **owner** (the existing `panel.share`
  owner rule) and holds `mcp:panel.share:call`. A panel the caller does not own is reported as a gap,
  never force-shared — no-widening (rule 6) is absolute here.
- A **preview/report** that lets the caller see exactly what will be shared, what is already shared (by
  team OR already workspace-visible), and what they cannot share (and why) **before** acting. The report
  is panel-centric (a per-panel disposition), NOT the dep-centric `AccessReport` shape — see the shape
  note under "How it fits".
- Idempotent: re-running shares nothing already shared; adding a panel later and re-running shares just
  the new one. A panel already `Visibility::Workspace` (readable by everyone) is a no-op reported as
  `already_visible_workspace`, never a gap — the offer must not nag about it.

## Non-goals

- **No auto-share on embed or on `dashboard.share`.** Adding a panel to a page, or sharing the page,
  never mutates panel visibility on its own. This verb is the deliberate step. (Why: below.)
- **No cross-owner force-share.** This verb never shares a panel the caller doesn't own; that would be
  a grant path around panel ownership. Fixing a not-owned gap is a conversation with the owner, not a
  privilege this verb holds.
- **No new visibility tier and no datasource/query sharing.** The closure's *data* dependencies
  (datasources, saved queries) re-check under the viewer's caps per render (the "sharing never widens"
  thesis) and are a datasource-authority concern, out of scope here. This verb moves **panel share
  edges** only.
- **No deep closure walk (v1).** Panel→panel nesting and query fan-out are reported as `unchecked`
  (mirroring `access_check` v1 depth), never silently shared. This cuts **both** ways and is the rule
  an implementer will get wrong: a nested panel must be reported `unchecked` — never silently
  `would_share` (which would widen a panel the caller never saw in the preview) and never silently
  dropped (which would false-green a closure that is still broken for the team).
- **No `unshare_closure` (v1).** This verb only ever ADDS share edges. Un-sharing stays the per-panel
  `panel.share(visibility=private)` the owner drives deliberately; a bulk un-share is its own ask with
  its own questions (whose audience are you narrowing, and does the page still work for them?) and must
  not ride in on this verb's back. Sharing remains a live relation — revoke an edge and gate-3 denies
  on the next call, unchanged (`visibility.rs`).

## Intent / approach

The platform already decided the shape. `access-model-scope.md` says: *"Assignment surfaces the
closure, never silently widens it. When you share a dashboard the tooling **offers** to share the
closure to the same team and warns on gaps — a guided, explicit step. It never auto-grants a capability
the assigner doesn't hold."* This scope builds the missing half of that sentence: `access_check` is the
"surface the closure" read; `share_closure` is the "offer to share it" write. They are duals over the
**same** closure enumeration and the **same** gate-3 visibility function (`may_read_panel`), so they can
never disagree about what the closure is.

**What "reuse" means concretely (read this before coding — the naive reading is wrong).**
`share_closure` CANNOT simply call `dashboard_access_check` and diff its report. That verb's walk
(`check_cell`/`check_target`, both private) computes verdicts against a **synthetic subject principal**
carrying the *team's* resolved caps — it answers "can the SUBJECT read this?". `share_closure` needs
that answer **and** a second, different one: "does the CALLER own this panel and hold `panel.share`?"
— i.e. is the gap closable. Two different principals, two different questions. So the reuse is precise:

- **Extract, don't duplicate — the enumeration.** The panel-ref enumeration currently inlined in
  `access_check::check_cell` moves into a shared `dashboard/closure.rs` (`closure_panels(&Dashboard)
  -> Vec<PanelRef>`) that **both** verbs call. Two independent walks over `cells[]` would be a parallel
  backend (rule 9) and could drift about what the closure even *is* — the false-green this scope exists
  to prevent. `access_check` keeps its verdict logic; only the enumeration lifts out.
- **Call, don't reimplement — the gate and the write.** Gate-3 is `panel::may_read_panel` (verbatim,
  the live predicate). The write is `panel::panel_share` (verbatim) — it already runs the
  `mcp:panel.share:call` gate and the `panel.owner != principal.owner_sub()` owner rule, so the owner
  wall and the S4 edge write stay in exactly one place. `share_closure` never touches `relate(SHARE)`
  itself.

**Rejected — auto-share on embed (the user's first instinct).** "When a panel is added to a team-shared
page, the team automatically gets the panel" is the intuitive fix and the wrong one. A library panel is
a first-class asset precisely so the *same* panel can sit on many dashboards with **different**
audiences (`library-panels-scope.md`). Auto-granting on embed makes "drop this panel on a team page" a
silent backdoor that widens who can read it — the exact widen-by-reference hole the whole "sharing never
widens" thesis exists to prevent, and a sibling of the wildcard-satisfies-admin-cap class
(`debugging/auth/member-wildcard-satisfies-admin-cap.md`). The panel's owner must *choose* to widen its
audience; the verb makes that choice one click, but a click it stays.

**Rejected — do it in `dashboard.share`.** Folding the cascade into `dashboard.share` would couple two
authorities (page visibility vs panel visibility) and re-privatize or re-share panels as a side effect
of page edits. Keeping it a distinct verb keeps each asset's visibility owned by one call, and lets the
UI show the preview and get a confirmation between "share the page" and "share its panels."

## How it fits

- **Capabilities & the deny path.** New verb gated by a new cap `mcp:dashboard.share_closure:call`.
  Because the verb can only ever *do what per-panel `panel.share` would already allow*, the honest
  minimal gate is: hold `mcp:dashboard.share_closure:call` to call it, and each panel is shared only if
  the per-panel owner+`panel.share` check passes (reused verbatim, not reimplemented). A caller without
  the cap is denied before any read (no existence signal — the S4 ordering rule). **Tier: this is an
  authoring action a member does on their OWN panels, so `mcp:dashboard.share_closure:call` is an
  AUTHOR cap, not admin-only** — verified against the wildcard-span invariant
  (`no_builtin_bundle_may_span_an_admin_only_cap`): it must NOT be reachable via any admin-only path,
  and must be a named member author cap (no broad wildcard grants it — that class of bug is closed).
- **Isolation.** Workspace-first like every dashboard/panel verb; the closure walk and every share edge
  are ws-scoped. A ws-B team can never be a share target for a ws-A panel.
  **Stricter than `panel.share`, deliberately:** `panel_share` today accepts any `team` string and
  `relate()`s to it without checking the team exists in `ws`. For a single call that is the caller's own
  foot-gun (one dangling edge); for a BULK call it would write a dangling edge for **every** owned panel
  in the closure off one typo'd/foreign team name. So `share_closure` **resolves the target team
  in-workspace BEFORE any write** and refuses the whole call (`BadInput`) if it does not exist — no
  partial application. This is a requirement of this verb, not a change to `panel.share`.
- **MCP surface (the right API shape).** Two calls, mirroring `access_check`'s read/act split:
  - `dashboard.share_closure(dashboard, team, dry_run=true)` → returns a `ShareClosureReport` (one
    disposition per closure panel — the six below) and mutates nothing. This is the preview the UI shows.
  - `dashboard.share_closure(dashboard, team, dry_run=false)` → performs the eligible shares and returns
    the same report with each item marked `shared` / `skipped(reason)`. `dry_run` defaults to **true**
    (a plan-only call is safe; the mutation is opt-in), same posture as `federation.migrate`.
- **Report shape — panel-centric, deliberately NOT `AccessReport`.** `access_check` returns
  `{dashboard, subject, ok, dependencies: [{dep, kind, ok, unchecked, cell, reason}]}` — a *dependency*
  verdict list spanning panels, datasources, endpoints, and vars. `share_closure` reports one row **per
  closure panel** with a *disposition*, because "what would this write do?" is a different question from
  "will this render?". Contorting `AccessReport` to carry share dispositions would degrade both. The
  shape:
  `ShareClosureReport { dashboard, team, dry_run, panels: [ShareClosureItem] }` where
  `ShareClosureItem { panel, title, cell, disposition, reason }` and `disposition` is exactly one of:
  - `would_share` (dry_run) / `shared` (applied) — caller owns it, holds `panel.share`, team can't read it yet.
  - `already_shared` — a `share` edge to this team already exists (idempotency).
  - `already_visible_workspace` — the panel is `Visibility::Workspace`; the team can ALREADY read it
    (`may_read_panel` returns `Ok` for any principal). Sharing is a no-op. **Not a gap** — without this
    disposition the UI would nag to "fix" a panel that needs nothing.
  - `not_owned` — a real gap the caller cannot close (needs the owner). Never force-shared.
  - `no_share_cap` — the caller OWNS it but lacks `mcp:panel.share:call`. Distinct from `not_owned`
    because the UI must say a different thing (ask an admin for a cap vs. ask a human for their panel).
  - `unchecked` — a hop v1 does not walk (panel→panel nesting), mirroring `access_check`'s honesty rule.
  The two shapes are bridged by the **dual-consistency test**, not by a shared struct: the set of panels
  `share_closure(dry_run=true)` reports as gate-3 gaps (`would_share ∪ not_owned ∪ no_share_cap`) must
  equal the set of `panel_ref` deps `access_check` reports `ok=false` for that same team. That is the
  anti-drift guarantee, and it holds across the differing shapes.
- **Data / motion.** No new records; reuses the S4 `share` edge (`panel -[share]-> team`) that
  `panel.share` already writes and `may_read_panel` already reads. Sharing is a live relation — revoke
  the edge and the panel is unreadable next call, unchanged.
- **Rule 10 / rule 9.** No extension is named; the verb is generic over any dashboard's panel closure.
  Tests boot the real store + real verbs (`mem://`), no mocks.
- **Skill doc (SCOPE-WRITTING §6) — required, not N/A.** This is an agent-/API-drivable surface: "share
  this page's widgets with the team" is exactly the kind of task an agent drives, and the dry_run→confirm
  two-step is the part a model will get wrong without a written how-to. The implementing session extends
  `skills/dashboard-mcp/SKILL.md` with a `share_closure` section grounded in a live run (the preview
  call, reading the dispositions, the confirm), alongside the existing `dashboard.access_check` material
  — the read and its remediation belong in one place.

## Example flow

1. ada owns `dashboard:ops-page` (embeds `panel:cpu` which she owns, `private`) and shares the page to
   team `ops` (`dashboard.share`). bob (a member of `ops`) opens the page: it renders, but the CPU
   widget shows **"Panel not accessible."**
2. ada (or the UI, on her behalf) calls `dashboard.share_closure("ops-page", "ops", dry_run=true)`.
   Report: `panel:cpu` → `would_share` (she owns it); page already shared; no gaps.
3. ada confirms → `dry_run=false`. The verb writes `panel:cpu -[share]-> ops`. Report: `panel:cpu` →
   `shared`.
4. bob reloads: gate-3 `may_read_panel` now finds him a member of a team `panel:cpu` is shared to → the
   widget renders. The wall did not move for anyone else; a non-`ops` member still gets 403.
5. Counter-case: the page also embeds `panel:aidan`, owned by aidan, `private`. Report:
   `panel:aidan` → `not_owned` (a gap ada cannot close). The verb shares everything else and reports
   `aidan` as the one thing that needs the owner — it never force-shares it.

## Testing plan

- **Capability-deny (mandatory):** no `mcp:dashboard.share_closure:call` → denied before any read.
- **Workspace-isolation (mandatory):** a ws-B team is never a valid target; a ws-B caller cannot reach
  a ws-A dashboard's closure. **Includes the bulk-specific edge:** a target team that does not exist in
  the caller's ws refuses the WHOLE call before any edge is written — asserted by reading the `share`
  edges of every owned panel afterwards and finding none (no partial application, no dangling edges).
- **Workspace-visible panels are not gaps:** a `Visibility::Workspace` panel in the closure reports
  `already_visible_workspace` and is NOT shared (no edge written) — the team could already read it.
- **Nested panels are `unchecked`:** a panel→panel hop is reported `unchecked` and no share edge is
  written for the nested panel — it is neither silently widened nor silently dropped.
- **No-widening (the load-bearing one):** a caller who does NOT own an embedded panel gets that panel
  reported `not_owned` and it is **not** shared — asserted by reading the `share` edges after a
  `dry_run=false` run. This is the test that pins "the verb is not a grant path."
- **Cap-tier + wildcard-span invariant:** the new cap sits in `AUTHOR_CAPS` — present in the member
  bundle **by name**, absent from the viewer bundle, absent from `ADMIN_ONLY_CAPS`. The existing
  `no_builtin_bundle_may_span_an_admin_only_cap` keeps passing unchanged (the cap is a concrete named
  verb matching no wildcard, and `share_closure` is not an admin verb) — but assert the tier placement
  explicitly so a future re-classification fails at the bundle, not in production.
- **Dual consistency:** the set of panels `share_closure(dry_run=true)` reports as gate-3 gaps
  (`would_share ∪ not_owned ∪ no_share_cap`) equals the set of `panel_ref` deps `access_check` reports
  `ok=false` for that same team — the read and the write agree about the closure across their two
  report shapes. This is what makes the extracted shared enumeration load-bearing rather than cosmetic.
- **Idempotency + incremental:** re-run shares nothing; add a panel, re-run, only the new one shares.
- **E2E over the gateway:** ada shares closure → bob (real team-member token) renders the widget;
  every 403 for a non-member stays 403 (the wall is unmoved).

## Risks & hard problems

- **The report must never false-green.** As with `access_check`, an unwalked hop is `unchecked`, never
  silently "ok" — under-reporting the closure would tell an admin "shared" when a nested panel is still
  private. The dual-consistency test guards this.
- **Owner-gap UX is the real edge.** The common frustrating case is a page embedding someone else's
  private panel. The verb reports it crisply; the *product* answer (ask the owner, or fork the panel
  inline) lives in the rubix-ai UI scope, not here.

## Resolved decisions

Both original open questions are **resolved** — no unanswered questions remain in this scope.

1. **One team, explicitly — RESOLVED: explicit `team` is required in v1.** (Was: "should no-team mean
   every team the page is shared to?") An explicit target matches `panel.share`/`dashboard.share`, keeps
   the no-widening story auditable (one named audience per call, one confirmation, one report), and
   keeps the report's rows unambiguous. The "share to the page's whole audience" convenience is a real
   follow-on but a *different* verb-shape: it fans the blast radius across N audiences in one click and
   needs a page→teams enumeration (`list_related_inverse` over the dashboard's `share` edges) with its
   own preview and its own tests. Deferred deliberately, not rejected — ship the single-team path first
   and let the UI loop over teams if it wants N.
2. **`no_share_cap` is a distinct disposition from `not_owned` — RESOLVED: yes, distinct.** They are
   different gaps with different human resolutions: `not_owned` needs *another person* (the owner) and
   is unfixable by the caller at any privilege; `no_share_cap` needs *an admin to grant a cap* and the
   caller already owns the asset. Collapsing them would make the UI say "ask the owner" to someone who
   IS the owner. The check reuses `authorize_tool(caller, ws, "panel.share")` — the same gate
   `panel_share` runs first — so the report can never disagree with what the write would do. Cost is one
   enum variant; the honesty is worth more than the saving.

## Decisions added by review (previously unstated)

3. **`already_visible_workspace` is a fourth non-gap disposition.** `may_read_panel` returns `Ok(())` for
   `Visibility::Workspace` before any team walk — such a panel is readable by the whole workspace, so a
   team share is a no-op. Reporting it as a gap would make the offer nag about panels that need nothing.
4. **Nested panels are `unchecked`, never silently shared and never silently dropped** (see Non-goals).
5. **No `unshare_closure` in v1** (see Non-goals) — this verb only ever adds edges.
6. **The target team must resolve in-workspace before any write** (see Isolation) — a bulk verb must not
   scatter dangling edges off one bad team name.
7. **The report is panel-centric, not `AccessReport`-shaped** (see MCP surface) — bridged to
   `access_check` by the dual-consistency test rather than by a shared struct.

## Related

- `docs/scope/auth-caps/access-model-scope.md` — the parent; this builds the "offer to share the
  closure" write half of its stated model. **Read together.**
- `docs/scope/frontend/dashboard/library-panels-scope.md` — why a panel is its own shareable asset
  (the reason auto-share-on-embed is wrong).
- `rust/crates/host/src/dashboard/access_check.rs` — the detection dual. `share_closure` does NOT call
  it (different principal, different question — see "What reuse means concretely"); the two SHARE the
  extracted `dashboard/closure.rs` enumeration and the `may_read_panel` gate, and are pinned together by
  the dual-consistency test.
- `rust/crates/host/src/panel/share.rs`, `panel/visibility.rs` — the per-panel share write + gate-3
  this verb reuses verbatim (never reimplements).
- `docs/debugging/auth/member-wildcard-satisfies-admin-cap.md` — the widen-by-reference class this scope
  is careful not to reintroduce; the new cap is checked against its invariant.
- rubix-ai `scope/frontend/dashboard/share-closure-ui-scope.md` — the guided UI that drives these verbs.
