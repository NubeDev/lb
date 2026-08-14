# Nav scope — workspace sidebar ordering (the arranging lever)

Status: **BUILT (2026-08-14)** — the companion to
[`nav-hide-and-pins-scope.md`](nav-hide-and-pins-scope.md). Where that scope gave the admin one
**subtractive** lever (hide an entry), this adds the one **arranging** lever: put the rail's
sections, and the entries inside them, in the order the workspace wants — without authoring a full
replacement nav.

Both levers live on the SAME record (`nav_hidden:[ws]`) and ride the SAME gates. Hiding and ordering
are independent: each setter carries the sibling field through, so saving visibility never discards
an arrangement and vice versa.

---

## Goals

- **A workspace ordering** — `NavHidden::order`, a list of item refs in the SAME opaque grammar the
  hidden-set uses (bare surface key, `ext:<id>`, `ext:<id>/<navid>`, `dashboard:<id>`, plus
  `group:<Label>` for a section heading). Applied inside `nav.resolve` at every tier and echoed for
  the built-in `SURFACES` fallback the server cannot materialize.
- **One admin verb** — `nav.order.set{order: []}` (full-list LWW; empty clears), riding
  `mcp:nav.save:call` exactly as `nav.hidden.set` does. Read-back is `nav.hidden.get`, which returns
  the whole record. No new cap, no second read verb for one record.
- **Reordering at every depth** — sections order among themselves, and an entry orders within its
  section. A group orders by its `group:<Label>` heading ref, the same string that hides its label.

## The central decision: a PARTIAL order

The ordering is **not** a complete permutation of the rail. A ref named in `order` sorts to its given
position; a ref that is NOT named keeps its natural (authored / caller-side) order **behind** every
named one. The sort is stable, so unnamed siblings never scramble relative to each other.

This is what makes ordering non-destructive, and it is the property every other behaviour falls out
of:

- **A stale ref is inert.** An uninstalled extension, a deleted dashboard, a renamed group heading —
  each names nothing and simply contributes no constraint. It never blanks a row or reorders others.
- **New entries are never lost.** A dashboard created after the ordering was saved, or a freshly
  installed extension, lands at the end of its list instead of vanishing or landing arbitrarily.
- **The UI never has to write a complete list.** Dragging one section to the top can persist a short
  list, not a full snapshot of the rail — so two admins arranging different parts of the rail do not
  clobber each other's untouched regions.
- **Ordering never adds or removes an entry.** It arranges what survived the caps-strip, the
  uninstalled-ext strip, and the hidden-strip. It is applied strictly AFTER all of them.

## Non-goals

- **Per-member ordering.** This is the workspace's arrangement, like the hidden-set. A member's
  personal ordering already exists in a bounded form — `nav_pref.pinned`, which is member-owned and
  renders above the menu.
- **Ordering the Pinned section.** Pins resolve in the member's own `pinned` order; the workspace
  ordering does not reach into a personal favorites list.
- **A second read verb.** `nav.hidden.get` returns the whole record; a `nav.order.get` would be one
  more door onto one record.

## Data

One additive field on the existing record — no migration. `NavHidden` is a plain serde struct in the
generic store (there is no `DEFINE FIELD` schema for `nav_hidden`), and the field is
`#[serde(default)]`, so a pre-ordering record deserializes with an empty `order` meaning "natural
order".

```rust
pub struct NavHidden {
    pub hidden: Vec<String>,
    pub order: Vec<String>,   // NEW — partial order, same ref grammar
    pub updated_ts: u64,
}
```

`ResolvedNav` gains the matching `order` echo, for the same reason it echoes `hidden`: the fallback
menu lives client-side, so the server cannot arrange the one tier it never materializes.

## Bounds

- `MAX_ORDER = 200` refs — `BadInput` over-cap, never silently truncated (sized like `MAX_HIDDEN`;
  an ordering names the same ref population a hidden-set does).
- A blank/whitespace ref is rejected as malformed.
- A **duplicated** ref is rejected — one ref cannot hold two positions. This is the one bound the
  hidden-set does not need, because a set is idempotent under duplication and a list is not.

## Surface

| Layer | Addition |
| --- | --- |
| Store | `NavHidden::order` (additive, `serde(default)`) |
| Verb | `nav_order_set` (`nav/hidden.rs`), rides `nav.save` |
| MCP | `nav.order.set` dispatch arm + catalog entry |
| Gate | `nav.order.set` aliased onto `nav.save` in `tool_call::gate_tool_for` |
| Resolve | `apply_order` — stable partial sort, recursive into groups, after every strip |
| Gateway | `POST /nav/order` (read-back via `GET /nav/hidden`) |
| Packs | `sidebar.order` on the manifest's `Sidebar` block |

## Testing plan

Covered in `crates/host/tests/nav_test.rs`:

- **Bounds** — over-cap, blank ref, duplicate ref all `BadInput`; LWW replace; empty clears.
- **Gate** — a member holding only `nav.resolve` is `Denied`.
- **Independence** — a hidden-set write preserves `order`, an order write preserves `hidden`. (The
  regression this guards: the Settings tab's visibility Save silently wiping the arrangement.)
- **Partial order (headline)** — named refs lead in their given order, unnamed keep authored order
  behind them, a stale ref is inert, and the item count is unchanged.
- **Depth** — a group sorts by its `group:<Label>` ref among its siblings, and its children sort
  within it.
- **Identity** — an empty ordering leaves the authored order exactly as-is.
- **Workspace wall** — one workspace's ordering never reaches another's resolve.
