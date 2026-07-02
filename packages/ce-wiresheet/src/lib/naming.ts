// Component-name helpers shared by add / group / paste flows.

// A component `type` looks like "vendor-ext::ComponentName". Derive a name the
// engine's validator accepts: take the local segment and strip anything that
// isn't alphanumeric or underscore (the auto-default can include "::").
export function sanitizeName(type: string): string {
  const idx = type.lastIndexOf("::");
  const local = idx >= 0 ? type.slice(idx + 2) : type;
  const cleaned = local.replace(/[^A-Za-z0-9_]/g, "");
  return cleaned || "node";
}

// First free name under a parent: `base`, then `base2`, `base3`, … skipping any
// already taken by a sibling.
export function uniqueName(base: string, taken: Iterable<string>): string {
  const set = taken instanceof Set ? taken : new Set(taken);
  let name = base;
  let n = 1;
  while (set.has(name)) {
    n += 1;
    name = `${base}${n}`;
  }
  return name;
}
