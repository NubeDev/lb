// The NavRail data model — what a host passes to describe its sidebar. Deliberately
// app-agnostic: no lb `CoreSurface` union, no capabilities. A host that gates entries by
// permission filters this list itself before handing it in.

import type * as React from "react";

/** One entry in the rail. `icon` is a component (e.g. a lucide-react icon). */
export interface NavItem {
  /** Stable id echoed back through `onSelect`; also the active-match key. */
  id: string;
  label: string;
  /** Rendered at 16px; shown alone (with a tooltip) when the rail is collapsed. */
  icon?: React.ComponentType;
  /** Optional group heading. Items sharing a `group` render under one label, in array
   *  order; ungrouped items render in the default (unlabeled) group. */
  group?: string;
}

/** Internal: items bucketed by `group`, preserving first-seen group order. */
export interface NavGroup {
  /** undefined = the default, unlabeled group. */
  label?: string;
  items: NavItem[];
}

/** Bucket `items` into ordered groups. Group order = order of first appearance; the
 *  default (ungrouped) bucket keeps its slot wherever its first item appears. */
export function groupItems(items: NavItem[]): NavGroup[] {
  const order: (string | undefined)[] = [];
  const byGroup = new Map<string | undefined, NavItem[]>();
  for (const item of items) {
    if (!byGroup.has(item.group)) {
      byGroup.set(item.group, []);
      order.push(item.group);
    }
    byGroup.get(item.group)!.push(item);
  }
  return order.map((label) => ({ label, items: byGroup.get(label)! }));
}
