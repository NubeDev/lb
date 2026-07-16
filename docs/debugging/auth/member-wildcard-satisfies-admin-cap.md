# A member's `mcp:*.list:call` satisfied every admin-only `.list` cap

**Date:** 2026-07-16 · **Area:** authz (role bundles / caps grammar) · **Severity:** privilege escalation
(live) · **Found by:** nav e2e testing (`rubix-ai/docs/testing/nav/README.md` §6)

## Symptom

Two symptoms, one cause. The second is the serious one.

1. **No curated nav EVER applied to ANY member.** `nav.resolve` returned `{"source":"fallback"}` for
   every member, against a team-shared nav they could `nav.get` (200). The feature was inert in
   production while looking healthy — a member still sees *a* rail, so nothing errors.
2. **A plain member could read admin surfaces.** On a live node, `user:bob` (`role:member`):

   ```
   GET /admin/teams        -> 200 + the full team roster
   POST /mcp/call roles.list  -> 200 + every role and its cap bundle
   POST /mcp/call invite.list -> 200
   ```

## Root cause

`AUTHOR_CAPS` / `VIEWER_CAPS` (`crates/host/src/authz/builtin_roles.rs`) carried broad wildcards as a
shorthand for "all the CRUD": `mcp:*.get|list:call` in the viewer floor, and
`mcp:*.write|create|update|delete|post:call` in the author delta.

In the caps grammar (`crates/caps/src/grammar.rs`), `*` matches exactly one segment, and the mcp
surface names its resource `<tool>.<verb>`. So the `*` spans the **`<tool>` half of every tool name** —
`mcp:*.list:call` is not "list-shaped verbs on the member's own stuff", it is *`list` on **anything***,
including `teams`, `roles`, `grants`, `invite`. Enumerated against `ADMIN_ONLY_CAPS`:

| Bundle | admin-only caps it AUTHORIZED |
|---|---|
| `member` | `teams.list`, `roles.list`, `grants.list`, `workspace.create`, `workspace.delete`, `ext.list`, `series.delete`, `nav.delete`, `invite.create`, `invite.list` — **10** |
| `viewer` | `teams.list`, `roles.list`, `grants.list`, `ext.list`, `invite.list` — **5** |
| `apikey-read` | the same 5 (identical hole, `crates/apikey/src/roles.rs`) |
| `apikey-write` | those + `workspace.delete`, `invite.create`, `nav.delete` |

Nav symptom 1 was downstream of the same fact: `nav::admin_lens::is_workspace_admin` probed its admin
marker caps with `holds_cap`, which is wildcard-aware, so `mcp:ext.list:call` matched a member's
`mcp:*.list:call` and every member classified as an admin. `pick_nav` then took the admin no-lockout
short-circuit for them and returned `fallback`. (`mcp:devkit.templates:call` — the member authoring
toolchain — was also literally in the marker list, an independent instance of the same confusion.)

**Why every existing test passed.** This module was written *because* of a prior escalation (the live
`user:bob` incident: a plain member added members, created teams, self-granted `workspace.delete`), and
it carries tests named for it. They all assert `!bundle.contains(admin_cap)` — **literal membership**.
But the wall does not enforce membership, it enforces `holds_cap`. A bundle can literally contain no
admin cap while authorizing ten. The wildcard is invisible to a `contains` check, so the tests guarding
this exact class were structurally unable to see it. `admin_routes_test.rs`'s
`forged_admin_call_by_non_admin_is_denied_server_side` missed it for a second reason: it mints a
hand-picked cap set (`bus:chan/*:pub`), a principal no real login ever issues.

The grammar is not at fault — `holds_cap` documents itself as "would this pass Gate 2?" and is the
right matcher for a gate. The defect was granting a bundle a wildcard whose span nobody had checked.

## Fix

**The bundles name their verbs.** The broad wildcards are gone from `VIEWER_CAPS`, `AUTHOR_CAPS`, and
both apikey role bundles; the legitimate verbs each one supplied are now named concretely (derived by
diffing the wildcard's real span from `GET /mcp/catalog` against what the bundles already named, so the
member's authoring reach is unchanged). The capability grammar is purely additive — there is no deny
form — so a bundle cannot subtract; the only way to bound one is to not over-grant it.

Deliberately **not** restored while naming the span: `secret.get`/`secret.list`/`secret.delete` (the
secret plane is not a viewer's to enumerate or an author's to mutate) and `teams.create`/`roles.delete`
(admin verbs; their tools dispatch on `teams.manage`/`roles.manage`, so the wildcard never actually
reached them — but naming them would grant by the back door what `ADMIN_ONLY_CAPS` denies by the
front). `teams.create` and `roles.delete` were also missing from `ADMIN_ONLY_CAPS` and were added — the
guard test is only as good as that list.

`store:*:read` / `store:*:write` stay: they are store-surface grants whose resource segment is a table,
so they cannot reach an `mcp:` management verb.

### The second fix — the bundle fix did not ship on its own

Removing the wildcards from the code fixed **no existing deployment**. Two mechanisms combined:

- `ensure_builtin_authz_roles` → `ensure_one` is **create-only** ("a present row is left untouched"),
  so a workspace seeded by the older binary keeps its `member` row forever. There is no migration.
- `resolve_caps_with` **unioned** the live bundle on top of that stored row — the fix for a *previous*
  stale-row bug (`builtin-role-row-frozen-stale-on-new-caps.md`). A union is a **floor**: it can add a
  cap to a stale workspace, never remove one.

So the stale row's `mcp:*.list:call` folded straight back into every member's token. Measured on a real
store: a workspace seeded pre-fix and upgraded to the fixed binary still resolved a member authorizing
**9 admin-only caps**. The security fix was inert on exactly the deployments that had the bug — and the
live verification below initially passed only because `make purge-store` had wiped the store first,
which is the kind of false green this class produces.

Fix: for a **built-in** role name the live bundle is now **authoritative and replaces** the stored
record (`crates/authz/src/resolve.rs`); the record is not read at all. Same stale-row workspace after:
**0**. No re-seed, no migration, no version bump.

Replace loses nothing the union bought. The union's stated reason was the extension-install path,
`grant_assign(Subject::Role("member"), cap)` — but that resolves through the role-**subject** recursion,
not the role's stored **record**. The two were conflated;
`live_builtin_caps_keep_direct_role_subject_grants` proves they are separable. Custom roles (no live
bundle) are untouched. One real behaviour change: `roles.define("member", …)` no longer alters what a
member resolves — the intended posture, since a built-in bundle is lb's policy and an admin widening
`member` is the escalation this model exists to prevent (no-widening already blocked adding caps they
lack; the supported path, a grant on the role subject, still works).

The freshness scope's closing invariant — *"Adding a cap … is the only change needed"* — was **true as
written and false in spirit**: silent on removal, because the mechanism only worked one way. Amended in
`scope/auth-caps/builtin-role-freshness-scope.md`. **A freshness mechanism that can only add is a
half-mechanism**: if the live definition is authoritative it must be authoritative in both directions,
or the stored row stays load-bearing for exactly the changes that matter most — the ones that take
power away.

`admin_lens.rs` keeps its exact match (the WIP fix) even though the bundles no longer contain the
colliding wildcards. The agreement between an exact match and a `holds_cap` match is a property of
today's bundles, not of the question — classification asks what a caller **is**, authorization asks what
they **may do**, and the call site should not silently depend on a bundle never growing a wildcard
again. `caps_hold_admin` already matched exactly; `admin_lens` was the deviation, and the module's claim
of ONE definition of admin is now true.

**Audit of every other `holds_cap` call site**: all are genuinely authorization, and wildcard-awareness
is correct — indeed required — in each. The no-widening guards (`apikey/create.rs`, `authz/grants.rs`,
`authz/roles.rs`, `webhook/create.rs`) *must* be wildcard-aware or an admin holding `mcp:*.write:call`
could not grant `mcp:foo.write:call`. The gates (`nav/resolve.rs`, `nav/reach.rs`, `authz/scoped.rs`)
are correct as-is. `admin_lens` was the only classification site.

## Regression tests

Each was proven to go **red on the old code** before being kept.

- `authz::builtin_roles::no_builtin_bundle_may_span_an_admin_only_cap` — **the invariant**. Asserts
  through the *real matcher* that no built-in non-admin bundle (member, viewer, apikey-read,
  apikey-write) authorizes any `ADMIN_ONLY_CAPS` entry. Red on old code listing all 10 for member / 5
  for viewer. This is the class-level guard: any future wildcard added to a bundle, or any admin verb
  added that collides with one, fails here — at the bundle it was added to.
- `nav::admin_lens::every_marker_cap_is_admin_only` — every marker is admin-only by tier AND
  unreachable by member/viewer. Pins the `devkit.templates` half.
- `nav::admin_lens::{member,viewer}_bundle_does_not_read_as_admin`, `workspace_admin_bundle_reads_as_admin`.
- `admin_routes_test::the_real_member_bundle_cannot_reach_an_admin_route` — end-to-end over the real
  router using the **real** `member_role_caps()` bundle (not a hand-picked set). Red on old code:
  `left: 200, right: 403` on `GET /admin/teams`.
- `builtin_role_freshness_test::live_builtin_caps_replace_the_stale_role_row` (lb-authz) — **the
  resolver mechanism**: a cap REMOVED from a built-in bundle must not resolve from a stale stored row.
  Red under the old union (which is what made the whole fix inert on existing deployments), green under
  replace. Its four siblings pass in BOTH modes, proving replace is a strict improvement rather than a
  trade — particularly `live_builtin_caps_keep_direct_role_subject_grants`, which pins the
  extension-install path the union existed to protect.
- `builtin_role_upgrade_test` (lb-host) — **the upgrade path end-to-end**, one layer up: lb's REAL
  `member_role_caps()` resolved through `resolve_caps_live` (the exact function the login mint calls)
  against a store whose `member` row holds the verbatim pre-fix wildcards. Red under the union with all
  seven readmitted; green under replace. Its companion
  (`a_stale_member_row_still_resolves_the_live_authoring_reach`) passes in BOTH modes and asserts the
  member keeps `dashboard.save`/`rules.save`/`flows.save`/`ingest.write`/`nav.resolve` — so the first
  test cannot pass by resolving an empty bundle, which would satisfy "no admin caps" perfectly.
- `rubix-ai/ui/src/lib/session/admin-caps.lockstep.test.ts` — parses lb's `ADMIN_MARKER_CAPS` out of
  the Rust source and pins `ADMIN_SECTION_CAPS` equal to it (skips when the lb checkout is absent).

`undo_exposure_grants_land_at_the_right_tiers` asserted `viewer.contains("mcp:*.list:call")` — it
pinned the *mechanism* (the leaking wildcard) instead of the *contract* (a viewer reaches
`history.list`). Rewritten to assert the reach through `holds_cap`.

## Live verification

Fresh store, fixture per `rubix-ai/docs/testing/nav/README.md` §2. Member `bob@acme.com`, admin
`ada@acme.com`, team `ops`, `ops-nav` sharing `ops-page` (team) + `secret-page` (private) + an `admin`
surface.

```
bob : {"source":"team","nav_id":"ops-nav","items":["Ops Page"]}   # curated nav applies at last
ada : {"source":"fallback","items":[]}                            # correct — no-lockout
GET /admin/teams   ada=200 bob=403                                # was bob=200
```

All 10 previously-leaked caps → 403 as bob. Every §5 wall assertion held, including after the
`__builtin__` escape hatch (menu widens, wall does not move). Member authoring verified intact: all 19
author `.list` reads 200, and a full create→read→delete circle on bob's own dashboard, while
`POST /navs` stays 403. Ada resolves 274 caps, bob 202.

**What the live node did NOT prove — and why that matters here.** This ran on a store created *after*
the fix, so it exercises a fresh seed, not an upgrade. The stale-row half cannot be driven from the
gateway on this node (`store.scan`/`store.tables` aren't registered, and `roles.define` correctly
refuses to re-plant the wildcards — no-widening now denies ada the `mcp:*.list:call` she'd have to hold
to grant it). Its evidence is `builtin_role_upgrade_test`, which drives lb's real bundle through
`resolve_caps_live` — the same function the login mint calls — against a deliberately stale row: 9
admin-only caps under the union, 0 under replace.

That gap is the whole lesson of this entry restated: **the first live verification of the wildcard fix
was a false green.** It passed because `make purge-store` had run, and a purged store is exactly the
one deployment shape the bug could not survive in. Had it not been re-checked against a stale row, a
fix verified live, green across 264 unit tests, would have shipped and protected nobody.

## The lesson

A role bundle is a **policy statement** and must be exhaustive by name. A wildcard in a bundle is an
open-ended promise about verbs that don't exist yet — it grants against tomorrow's tool names, which no
reviewer can audit. `apikey/roles.rs` shows how the reasoning fails halfway: it correctly saw that
`mcp:*.*:call` would reach `apikey.manage` and chose action-named wildcards instead — right about the
mechanism, wrong about the blast radius, because `apikey.manage` was never the only management resource,
just the one we thought of.

If naming every verb feels tedious, that tedium is the feature: it makes the blast radius of a new admin
verb reviewable. And when a test guards a security class, assert the property **through the enforcement
path** (`holds_cap`), never a proxy for it (`contains`) — this bug survived a module explicitly written
to prevent it, guarded by tests explicitly named for it, because every one of them checked the proxy.
