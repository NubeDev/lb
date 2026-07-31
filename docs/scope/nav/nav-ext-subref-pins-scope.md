# Nav scope — pinning an extension's declared nav destination (`ext:<id>/<navid>`)

Status: **PROPOSED (2026-07-31)** — a bug-shaped gap in the shipped
[`nav-hide-and-pins-scope.md`](nav-hide-and-pins-scope.md), surfaced by the rubix-ai sidebar.

The hide/pin ref grammar the resolver understands has **three** shapes: a bare surface key, a
`dashboard:<id>` ref, and a whole-extension `ext:<id>` ref. The shell's sidebar, since
`ext-nav-contribution`, renders a pin affordance on a **fourth** shape it invented — an
extension's individual declared nav destination, `ext:<extid>/<navid>` (e.g.
`ext:modbus/networks`). `nav.pref.set` accepts and persists it (refs are opaque, validated only
for non-emptiness and count), but `nav.resolve` silently drops it. The pin fills, the reload
lands, and it un-fills. This scope closes the grammar gap at the resolver.

---

## Goals

- **Teach `pin_to_item`/`resolve_*` the `ext:<extid>/<navid>` shape** — a pin ref with exactly
  one `/` after the `ext:` prefix resolves by (1) finding the install via the generic `ext.list`
  seam, then (2) finding the declared `[[ui.nav]]` item whose `id` matches the second segment.
  Both are opaque-string lookups; no id is ever branched on (rule 10).
- **Honour the destination's own kind.** A declared nav item may carry a `dashboard` ref +
  `vars` (`ext-dashboard-nav` scope). A pin on such an item resolves to a **`dashboard`**-kind
  `ResolvedItem` carrying those vars — so the pinned entry opens the board var-bound, exactly as
  clicking the sidebar row does. A destination with no `dashboard` resolves to an **`ext`**-kind
  item carrying the sub-ref.
- **Round-trip the ref.** `ResolvedItem` gains one optional field, `nav: String` (the `<navid>`
  segment, empty for a whole-extension item), so the client's `itemRef` can reconstruct
  `ext:<ext>/<nav>` and light the pinned state on the right row. Serde-defaulted +
  `skip_serializing_if` ⇒ an old client and a pre-field record both read exactly as today.
- **Symmetric hide.** `item_ref` emits the same sub-ref for a resolved sub-destination, so
  `nav_hidden` can suppress one extension destination — the grammar asymmetry the pin bug
  exposed applies equally to hide, and is fixed in the same place.
- **Strip on every failure, silently.** An uninstalled ext, a `[[ui.nav]]` id that no longer
  exists (the extension shipped a new manifest), an admin-hidden ref, or a cap-stripped
  dashboard all drop the pin from the resolved list without mutating the stored record — the
  existing invariant, extended to the new shape. A later reinstall/regrant restores it free.

## Non-goals

- **No pinning of DYNAMIC children.** A `bridge.setNav` child (`ext:<id>/<navid>/<childid>`,
  three segments) exists only while the extension is mounted and publishing; the server cannot
  resolve it and must not persist a ref it can never honour. The resolver treats any ref with
  ≥2 slashes after `ext:` as unresolvable → stripped. The **shell** drops the pin affordance on
  those rows so nothing advertises what cannot work. A durable pin for a runtime-published child
  would need an extension-owned resolution seam — a real scope, not this one.
- **No manifest, SDK, or storage change.** `ExtUi.nav: Vec<ExtNavItem>` is already persisted on
  the install and already returned by `ext.list`. The resolver simply never looked at it. No
  `lb-ext-sdk`/`lb-ext-ui-sdk` version moves for this.
- **No new caps or verbs.** Pins ride the existing member-owned `nav_pref` read/write path.
- **No authorization change.** A pin grants nothing; a hide blocks nothing. Deep links and
  server-side re-checks are untouched.
- **No validation of an extension's declared ids at pin time.** `nav.pref.set` keeps accepting
  opaque refs; resolution is the only gate. This preserves the "stored record survives a
  temporary strip" property that makes uninstall/reinstall lossless.

## Intent / approach

**The fix belongs at the resolver, not the shell**, because the shell's ref grammar is the one
that matches the product: an extension's declared destinations are stable, manifest-backed pages
— exactly as pinnable as a core surface. Making the rail stop emitting the ref (the other
option) would remove a working-looking feature users can already see, and would leave the same
asymmetry in the hidden-set.

`pin_to_item` currently commits to a kind before any lookup. That is why an `ext:` ref with a
slash falls into the ext branch with a nonsense id `"modbus/networks"` and dies at
`resolve_ext`'s `installed.iter().find(...)`. The change splits the `ext:` branch on the
presence of a `/` and routes the sub-ref case to a new `resolve_ext_nav`, which does the
two-step opaque lookup and then **defers to the declared item's own shape** for the resulting
kind. Everything else — cap-strip, hide-strip, the order-preserving `resolve_pins` loop — is
reused unchanged.

The one wire addition (`ResolvedItem.nav`) is what makes the pinned-state highlight correct
rather than merely present: without it the client reduces a resolved sub-destination back to
`ext:<id>` and would light the pin on the extension's *other* rows too.

## Risks / open questions

- **A destination that changes `dashboard` between installs** silently changes what a pin opens.
  Acceptable: the pin references the destination, and the extension owns what lives there.
- **`MAX_PINNED`** is unchanged; sub-refs consume the same budget as any pin.
- **Ordering** within `pinned` is the member's, preserved as today.

## Test plan

Real store (`mem://`), no mocks — the shipped nav test pattern:

- a pin on a declared ext destination resolves to a rendered entry, in member order;
- a destination declaring `dashboard` + `vars` resolves to a `dashboard`-kind item carrying
  those vars (opens var-bound);
- a destination with no `dashboard` resolves to an `ext`-kind item whose `nav` echoes `<navid>`;
- an unknown `<navid>` (manifest changed) strips silently, record untouched;
- an uninstalled ext strips the sub-ref pin exactly like a whole-ext pin;
- a hidden `ext:<id>/<navid>` ref beats the pin (hide wins), and `item_ref` emits the sub-ref so
  the hidden-set can target one destination;
- a 3-segment (dynamic-child) ref strips silently and never faults the menu;
- a whole-extension `ext:<id>` pin still resolves byte-identically (no regression).
