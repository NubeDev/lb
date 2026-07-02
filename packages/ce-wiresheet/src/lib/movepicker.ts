import type { Component } from "./engine-types";

// Destination logic for the "Move into…" picker (pure).

export interface MoveCandidate {
  uid: number;
  name: string;
  kind: string;
  path: string;
  tier: number;
}

// Candidate destinations for moving `movingUids`, excluding self and own
// descendants (no cycles), ordered by relationship to the folder being moved
// FROM: up one level (0) → same level (1) → children deeper inside (2) →
// everything else (3), then by path within each tier.
export function moveCandidates(allComponents: Component[], movingUids: Iterable<number>): MoveCandidate[] {
  const movingSet = new Set(movingUids);
  const movingPaths = allComponents.filter((c) => movingSet.has(c.uid)).map((c) => c.path);
  const isMovingOrDescendant = (path: string): boolean =>
    movingPaths.some((mp) => path === mp || path.startsWith(mp + "/"));

  const movingComp = allComponents.find((c) => movingSet.has(c.uid));
  const curFolderUid = movingComp?.parent; // the folder we're moving FROM
  const curFolder = allComponents.find((c) => c.uid === curFolderUid);
  const upUid = curFolder?.parent; // one level up
  const curFolderPath = curFolder?.path;
  const tierOf = (c: Component): number => {
    if (upUid !== undefined && c.uid === upUid) return 0;
    if (curFolderUid !== undefined && c.parent === curFolderUid) return 1;
    if (curFolderPath && c.path.startsWith(curFolderPath + "/")) return 2;
    return 3;
  };

  const candidates: MoveCandidate[] = [];
  for (const c of allComponents) {
    if (movingSet.has(c.uid)) continue;
    if (isMovingOrDescendant(c.path)) continue;
    candidates.push({ uid: c.uid, name: c.name || c.type, kind: c.type, path: c.path, tier: tierOf(c) });
  }
  candidates.sort((a, b) => (a.tier !== b.tier ? a.tier - b.tier : a.path.localeCompare(b.path)));
  return candidates;
}

export function filterMoveCandidates(candidates: MoveCandidate[], filter: string): MoveCandidate[] {
  const f = filter.trim().toLowerCase();
  if (!f) return candidates;
  return candidates.filter(
    (c) =>
      c.name.toLowerCase().includes(f) ||
      c.kind.toLowerCase().includes(f) ||
      c.path.toLowerCase().includes(f),
  );
}
