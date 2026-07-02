// Enumerate every component of a given full "vendor-ext::name" type, globally
// (all folders), via the engine's flat `?type=` scan — structure only, no value
// plane. Powers the empty-state picker's "show all of this type regardless of
// current folder" columns (schedules, jsLogic, …). A registry/index service
// would just duplicate what this scan already returns.

import { getRootNodes } from "../lib/rest";

export interface TypeScanRow {
  uid: number;
  name?: string;
  path?: string;
  type: string;
  parent?: number;
}

export async function loadComponentsByType(fullType: string): Promise<TypeScanRow[]> {
  const r = await getRootNodes({ type: fullType, values: false });
  return r.nodes.map((c) => ({ uid: c.uid, name: c.name, path: c.path, type: c.type, parent: c.parent }));
}
