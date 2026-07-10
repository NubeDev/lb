// A rail/top-menu entry's ref in the shared hide/pin grammar (mirrors the resolver's `item_ref`).
// The one home for this pure mapping so `NavRail` and `TopMenuNav` can never drift on what a
// resolved item's pin/hide ref is — both renderers consume the same resolved-nav data, and both
// must agree on refs (a pin toggled in one is the same ref in the other). One concept per file.

import type { ResolvedNavItem } from "./NavRail";

export function itemRef(it: ResolvedNavItem): string {
  if (it.kind === "ext" && it.ext) return `ext:${it.ext}`;
  if (it.kind === "dashboard" && it.dashboard) return it.dashboard;
  return it.surface ?? "";
}
