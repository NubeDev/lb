// Subscription set diffing for the WS client (pure).

export interface SetDiff {
  added: number[]; // in `desired` but not `current`
  removed: number[]; // in `current` but not `desired`
}

// Compute what to subscribe / unsubscribe to move from `current` to `desired`.
export function diffSets(current: Set<number>, desired: Set<number>): SetDiff {
  const added: number[] = [];
  const removed: number[] = [];
  for (const uid of desired) if (!current.has(uid)) added.push(uid);
  for (const uid of current) if (!desired.has(uid)) removed.push(uid);
  return { added, removed };
}
