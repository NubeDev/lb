# Nav scope — pinning a curated menu ROW (`nav:<navid>/<rowid>`)

Status: **IN PROGRESS (2026-09-17)** — a gap in the shipped
[`nav-hide-and-pins-scope.md`](nav-hide-and-pins-scope.md), surfaced by the rubix-ai sidebar.

The pin grammar names a TARGET: a surface key, `dashboard:<id>`, `ext:<id>`, `ext:<id>/<navid>`, or a
host-authored ext board row. A curated nav, though, is mostly rows that are not a bare target:

- a `dashboard` row with a pinned **variable binding** (reusable-pages scope) — "Chullora › Energy" is
  the Energy board bound to `site=chullora`. Pinning `dashboard:energy` opens the shared board with no
  site, so the shell draws no pin on such a row at all;
- a `group` (folder) carrying its **own board** (nav-folder-target) — a site folder that opens its
  overview. There is no ref for a folder.

A site menu is almost entirely those two shapes, so a member browsing one sees no pin affordance
anywhere. This scope makes a curated row pinnable **as the row**.

---

## Goals

- **Stable row ids.** `NavItem` gains an optional `id` (opaque, `[A-Za-z0-9_-]`, ≤ 64). `nav.save`
  keeps every valid id it is given, mints one for any row without one, and re-mints a DUPLICATE (the
  first occurrence keeps it — the builder's "duplicate row" copies an id). A malformed id is
  `BadInput`. Ids are what make a row addressable across reorders: a positional path breaks the moment
  an author drags a site above another.
- **Echo the id.** `ResolvedItem` gains `id` (the row id, on every row resolved from a stored item —
  never on a tag-/template-group's generated children, which have no stored row).
- **A fourth pin grammar: `nav:<navid>/<rowid>`.** `resolve_pins` resolves it by reading nav `<navid>`
  through the SAME readability gate the pick tiers use (gate 3), finding the row by id at any depth,
  and running it through the ordinary `resolve_item` pipeline — so a cap-stripped board, a deleted
  row, a deleted/unshared nav all strip silently, never mutating `nav_pref`.
- **The pinned row keeps the row's identity.** It carries the row's own label, icon, colour and
  `vars`, plus `nav_id` (so the client reconstructs the ref and lights the right row) and `trail`
  (the ancestor folder labels, so two sites' "Energy" pins read apart).
- **A pinned folder is the folder.** It resolves exactly as it does in the menu — a `group` carrying
  everything inside it (cap-stripped, empty subfolders pruned, row ids echoed) plus its own board when
  it has a readable one. A member who pins a site wants the site, not just its overview; and a folder
  with no board of its own is pinnable too. (Decided 2026-09-17 after first flattening a folder to its
  board — the pages under it silently went missing from Pinned.)
- **Hide still beats pin.** Through the menu's own `strip_hidden`: a hidden page strips, a pinned
  folder loses its hidden descendants, and a folder left empty strips.

## Non-goals

- **No pinning of generated rows.** A tag-group / template-group expands at resolve time; its children
  have no stored row and so no id, so they cannot be pinned on their own. The group row itself IS a
  stored row and pins like any folder — with its expansion, re-resolved on every load.
- **No migration.** A nav saved before this field has no ids; its rows gain them the next time the nav
  is saved. Until then the client draws no pin on an id-less row — exactly today's behaviour.
- **No new caps or verbs; no change to hide/order grammar.** Pins still ride `nav.pref.set`.
- **No authorization change.** A pin grants nothing — the row resolves under the caller's own caps and
  only if the caller can read the nav it lives in.

## Risks / open questions

- **The author edits the row.** The pin follows it (label, board, vars) — that is the point of pinning
  the row rather than a snapshot. Deleting the row strips the pin.
- **The member loses access to the nav** (unshared, deleted). The pin strips silently and returns if
  access returns, like every other stale pin.
