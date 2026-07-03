# nav — the nav builder (session)

- Date: 2026-07-03
- Scope: ../../scope/nav/nav-builder-scope.md
- Stage: S9+ collaboration UI (builds on the shipped S8 data plane + the dashboard asset/share model)
- Status: done

## Goal

Ship the **nav builder** end to end: a workspace-scoped, slug-identified `nav` asset (an ordered menu
of items linking to core surfaces / dashboards / extension pages / dynamic tag-groups), the full MCP
CRUD + a `nav.resolve` resolver + a per-user pick + a workspace-default pointer, and the UI (NavRail
renders the resolved menu with a SURFACES fallback + a builder tab under the access console). The whole
design is the **lens-vs-grant boundary**: the nav shapes the menu and grants nothing — the exit gate is
the "nav never widens" test.

## What changed

**Backend — a new `nav` host module (`rust/crates/host/src/nav/`), cloned from `dashboard/`:**

- `model.rs` — `Nav` (`id/title/owner/visibility/items[]/schema_version/updated_ts/deleted`), the four
  item kinds + `group` (`NavItem`), `NavPref` (the member-owned pick), `ResolvedNav`/`ResolvedItem`
  (the resolver payload with a `source` tier), item/tag-group caps.
- `store.rs` — raw read/write/scan for `nav`, `nav_pref:[ws,user]`, and the `workspace_nav_default:[ws]`
  pointer (envelope-unwrap mirrors `scan_dashboards`).
- The verbs, one per file: `get`, `list`, `save` (bounded via `bounds.rs`), `delete`, `share` (S4
  `share` edge), `default` (`nav.set_default`), `pref` (`nav.pref.get/set`), and the composite
  `resolve.rs`. `authorize.rs`/`visibility.rs`/`error.rs`/`surfaces.rs` are the gate + surface→cap map.
- `tool.rs` — the `call_nav_tool` MCP bridge (takes `&Node`; `resolve`/`pref` need ext discovery).
- Wired into `lib.rs` (exports), `tool_call.rs` (`is_host_native` + dispatch + gate aliases for
  `nav.pref.*`→`nav.resolve` and `nav.set_default`→`nav.save`), and `system/catalog.rs` (9 host tools).

**Gateway (`rust/role/gateway/`):** `routes/nav.rs` (9 routes: `/navs` CRUD, `/navs/{id}/share`,
`/nav/resolve`, `/nav/default`, `/nav/pref`), registered in `server.rs`; the six `mcp:nav.*:call` caps
added to the dev-login `member_caps()`.

**UI (`ui/`):** `lib/nav/` (types + api client + barrel), the `nav_*` cases in `lib/ipc/http.ts`, the
`CAP.nav*` strings, `features/shell/useResolvedNav.ts` + a `resolvedItems` prop on `NavRail` (renders
the resolved menu, falls back to `SURFACES` — route gates untouched), and `features/admin/nav/`
(`useNavs` + `NavAdmin` builder) added as a **Nav tab** under the access console.

## Decisions & alternatives

Resolved the scope's open questions:

- **Builder home:** a **tab under the access console** (`AdminView`), cap-gated by `nav.save` — it's an
  authz-adjacent authoring tool (the role grants; the nav shapes). Rejected a new top-level surface.
- **Item cap:** `MAX_ITEMS = 100` (incl. one nesting level), `MAX_TAG_GROUP = 50` expanded dashboards
  per group. Enforced in `nav.save` (host is the boundary) + capped at resolve.
- **Workspace-default:** an **explicit `nav.set_default{id}`** → a single `workspace_nav_default:[ws]`
  pointer (determinism), NOT "first visibility:workspace nav wins". Chose the explicit pointer so the
  resolved tier is deterministic. A `visibility:workspace` nav is NOT itself a pick tier.
- **`nav_pref` vs a prefs axis:** a **dedicated `nav_pref` record** — keeps the `lb-prefs` axis set
  closed to formatting. `nav.pref.*` gate on `mcp:nav.resolve:call` (curating your pick is part of
  resolving your own menu); the pick is keyed to the token `sub` (a caller can't set another's pick).
- **Uninstalled ext-page entries:** **stripped silently** at resolve (like a cap-stripped item) — the
  `ext.list` discovery treats the id as opaque (rule 10), no branch.
- **Ordering/grouping shape:** a **flat ordered `items[]`** with `group` children (one source of truth).

**The lens, never a grant (the whole design):** `nav.resolve` is a pure filter over caps the caller
already holds — a `surface` survives iff the caller holds its gate cap (`surfaces.rs`, mirroring the
UI's `allowedSurfaces`); a `dashboard`/tag-group dashboard survives iff the three-gate `dashboard.get`
passes; an `ext` survives iff still installed. It never writes a cap. Rejected embedding caps in nav
items (would duplicate `lb-authz` and violate capability-first).

## Tests

**Backend — `rust/crates/host/tests/nav_test.rs` (real store/node, seeded via the real write path):**
CRUD round-trip, idempotent upsert, over-cap/nesting/unknown-kind rejection, **capability-deny per verb**
(mandatory), **workspace isolation** (mandatory), **gate-3 team-shared member-reads/non-member-denied**,
the **"nav never widens" headline** (strips a surface + dashboard the caller lacks AND a direct read is
still denied server-side), resolution precedence (pick > team > default > fallback, incl. stale-pick
fall-through), tag-group dynamism (tag → appears, untag → gone, unreadable dashboard hidden), member-owned
pref, and group-child independent strip.

```
running 11 tests
test crud_round_trip ... ok
test idempotent_upsert_by_slug ... ok
test over_cap_items_rejected ... ok
test each_verb_is_denied_without_its_cap ... ok
test workspace_isolation ... ok
test team_shared_member_resolves_non_member_denied ... ok
test nav_never_widens_strips_and_direct_read_still_denied ... ok
test resolution_precedence_pick_over_team_over_default_over_fallback ... ok
test tag_group_expands_dynamically_and_respects_reachability ... ok
test member_owns_own_pref_cannot_touch_anothers ... ok
test group_children_are_stripped_independently ... ok
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s
```

**Frontend — unit (`NavRail.test.tsx`, 4 tests) + real-gateway (`NavAdmin.gateway.test.tsx`, 4 tests):**

```
✓ src/features/shell/NavRail.test.tsx (4 tests)   # resolve rendering, fallback, cap-strip shape, empty→fallback
   ... Test Files 67 passed (67) / Tests 430 passed (430)

✓ src/features/admin/nav/NavAdmin.gateway.test.tsx (4 tests)
   # builder round-trip (surface + tag-group → save → reload → getNav), member resolve,
   # cap-strip a surface the caller lacks (nav never widens, over the real gateway), fallback
```

Both mandatory categories (capability-deny per verb, workspace-isolation) are covered on the backend;
the cap-strip lens is proven on both sides (Rust `nav_never_widens_*` + the UI `signInWithCaps` gateway
test). No mock data / no fake backend — every list is a real `*.list`, every write a real `nav.*`.

## Debugging

None — two test-authoring slips (a principal missing a `DELETE`/`tags.remove` cap in a fixture) were
fixed in the same run, not real defects. No `debugging/` entry.

## Public / scope updates

Promoted to `public/nav/nav.md` (the shipped truth) + a `public/SCOPE.md` bullet. The scope doc's open
questions are resolved above and reflected in `scope/nav/nav-builder-scope.md`.

## Skill docs

n/a this session for the SKILL surface: the scope names `skills/nav/SKILL.md` as a follow-up. The
`nav.*` verbs ARE MCP-drivable (host-native, in the catalog) and exercised by the real-gateway test; the
authored skill wrapper is a named follow-up (see below).

## Dead ends / surprises

- The `<Select>` primitive is a native styled `<select>` (single export), not a shadcn compound — the
  builder uses `<option>` children. `@/lib/ext` has no barrel; import from `@/lib/ext/ext.api`.
- Two pre-existing gateway/host test failures are unrelated to nav and fail on a clean tree:
  `SystemView.gateway` (subsystem sheet), `sqlSource.gateway` (render), and `agent_routed_test`
  (mock-model agent-loop timing). Confirmed by stashing the nav changes and re-running.

## Follow-ups

- `skills/nav/SKILL.md` — the drivable skill wrapper the scope names (deferred, named).
- Dashboard **deep-board links**: a resolved `dashboard`/tag-group entry currently navigates to the
  Dashboards surface; a specific-board deep link is a named follow-up.
- Extension-authored navs (v1 non-goal), drag-reorder polish, paged roster beyond `MAX_NAVS`.
- STATUS.md updated (nav slice: shipped).
