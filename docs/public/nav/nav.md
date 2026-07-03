# Nav — user-/team-authored navigation (public)

Status: **shipped**. Session: [`sessions/nav/nav-builder-session.md`](../../sessions/nav/nav-builder-session.md).
Scope: [`scope/nav/nav-builder-scope.md`](../../scope/nav/nav-builder-scope.md).

The **nav builder** lets an admin (or empowered member) compose a navigation menu — a `nav` asset — whose
ordered entries link to core surfaces, dashboard pages, extension pages, or dynamic tag-groups, assign it
to teams, and have each member see it resolved to *their* effective menu. The menu is a **lens over
existing access, never a grant path**: an entry the caller lacks the capability for is stripped by the
resolver, and the gateway re-checks every verb on click regardless.

## The asset

A `nav` is a workspace-namespaced `nav:{id}` record, modeled exactly like `dashboard:{id}`:

- `id` (slug, unique per ws), `title`, `owner`, `visibility` (`private | team | workspace`),
  `items[]` (a typed nested array), `schema_version`, `updated_ts`, `deleted` (soft-delete tombstone).
- Shared to a team via the **shipped S4 `share` edge** (`nav -[share]-> team`) — the same three-gate
  read check dashboards use (`may_read_nav`), reused verbatim. No new ACL.

**Four item kinds + one grouping level** (`items[]`):
- `surface` — a core system page by its opaque key (`"channels"`, `"rules"`, …).
- `dashboard` — a specific `dashboard:{id}`.
- `ext` — an extension page by its **opaque** id (rule 10 — never branched on; resolved via `ext.list`).
- `tag-group` — a **dynamic** `{ label, facets:[{key,value?}] }`, expanded at resolve time to the tagged
  dashboards (`tags.find`); tag a new dashboard → it appears, no nav edit.
- `group` — `{ label, items:[…] }`, **one** nesting level (a nested `group` is rejected at save).

Bounds (host is the boundary; rejected, not silently stored): `MAX_ITEMS = 100` (incl. nesting),
`MAX_TAG_GROUP = 50` expanded dashboards per group.

Two adjacent records: `nav_pref:[ws, user]` (the member-owned active pick) and
`workspace_nav_default:[ws]` (the admin-set default pointer). Neither is a `lb-prefs` axis — the prefs
axis set stays closed to formatting.

## The verbs (host-native MCP, each its own cap)

| Verb | Cap | Level | Notes |
|---|---|---|---|
| `nav.get{id}` | `mcp:nav.get:call` | member | three-gate read, full `items[]` |
| `nav.list` | `mcp:nav.list:call` | member | membership-filtered `NavSummary` roster (no items) |
| `nav.save{id,title,items,now}` | `mcp:nav.save:call` | admin-ish | idempotent UPSERT (owner-only update), bounded |
| `nav.delete{id,now}` | `mcp:nav.delete:call` | admin-ish | idempotent tombstone (owner-only) |
| `nav.share{id,visibility,team?,now}` | `mcp:nav.share:call` | admin-ish | set visibility / write the S4 `share` edge (owner-only) |
| `nav.set_default{id,now}` | `mcp:nav.save:call` | admin-ish | set the one `workspace_nav_default` pointer |
| `nav.resolve` | `mcp:nav.resolve:call` | member | THE composite read: pick + tag-expand + cap-strip |
| `nav.pref.get` / `nav.pref.set{id,now}` | `mcp:nav.resolve:call` | member | the member's OWN pick (keyed to the token `sub`) |

Wired end to end: store → cap → MCP bridge (`call_nav_tool`) → gateway route (`/navs`, `/nav/resolve`,
`/nav/default`, `/nav/pref`) → `http.ts` (`nav_*`) → UI (`lib/nav`). `nav.pref.*` and `nav.set_default`
gate-alias to `nav.resolve`/`nav.save` at the outer dispatcher (no separate cap for the pick / the pointer).

## The resolver — `nav.resolve`

Returns the caller's **effective** menu as `{ source, nav_id?, title?, items[] }`:

1. **Pick** (4-tier, deterministic): personal `nav_pref` → first team-shared nav for the caller's teams
   → `workspace_nav_default` pointer → **`fallback`** (no nav; the UI renders its built-in `SURFACES`,
   never blank). A pick/default pointing at a deleted/unreadable nav falls through, never errors.
2. **Tag-expand**: each `tag-group` → a `group` of the dashboards matching its facets, filtered to what
   the caller can read.
3. **Cap-strip (the lens)**: a `surface` survives iff the caller holds its gate cap (the backend mirror
   of the UI's `allowedSurfaces`, in `nav/surfaces.rs`); a `dashboard`/tag-group dashboard survives iff
   the three-gate `dashboard.get` passes; an `ext` survives iff still installed (uninstalled → stripped
   silently). Nothing here can be reached that the caller couldn't reach directly.

The nav **grants nothing** — the headline invariant, proven by the `nav_never_widens_*` test (a nav
lists a surface + dashboard the caller lacks; `nav.resolve` strips both AND a direct read is still
denied server-side). The server re-checks every verb on click; route gates (`CoreGate`) are untouched —
the nav *hides*, it does not *block* (a deep link to a permitted-but-unlisted page still works).

## The UI

- **NavRail** (`ui/src/features/shell/NavRail.tsx`) renders `nav.resolve` output (via `useResolvedNav`,
  which re-resolves on ws-change + focus), falling back to today's built-in `SURFACES.filter(allowed)`
  when no nav applies. Route gates unchanged.
- **The builder** (`ui/src/features/admin/nav/NavAdmin.tsx`) — a **Nav tab under the access console**
  (cap-gated by `nav.save`): pick items from the three real sources (`SURFACES`, `dashboard.list`,
  `ext.list`), add a tag-group via a facet key, order/group/remove, set visibility, share to a team, and
  make a nav the workspace default. Every write is a real `nav.*` verb; the builder can never author a cap.

## Guarantees

- **Workspace wall (rule 6):** every `nav`/`nav_pref`/default key is `[ws, …]`; ws-B never reads or
  resolves ws-A's navs or picks. Tested.
- **Capability-first (rule 5):** each verb is `mcp:nav.<verb>:call`-gated at the shared `authorize_tool`
  chokepoint; per-verb deny tested. The nav is a lens, never a grant path.
- **Core knows no extension (rule 10):** an `ext` item stores the opaque id as data, resolved only via
  the generic `ext.list` seam — no core branch on any ext id.
- **Symmetric nodes (rule 1):** store state + cap checks + tag lookups — no `if cloud`; resolves the same
  on an edge node as in the cloud.
