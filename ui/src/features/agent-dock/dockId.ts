// The `dock-` session-channel id convention (agent-dock scope) — mint / parse / filter helpers. A dock
// session is an ordinary channel whose id is `dock-{user-slug}-{ulid}`; the prefix is reserved BY
// CONVENTION IN THE UI ONLY (the host never knows it — the wall is caps, not the name, per scope).
// One responsibility: own the id grammar so the picker, the channel-list filter, and the mint agree.
//
// **Why `-`, not `.`** (scope divergence, recorded there): the capability grammar
// (`crates/caps/src/grammar.rs`) splits a resource on BOTH `/` and `.`, and a member's channel grant is
// `bus:chan/*:pub` where a single `*` matches exactly ONE segment. A dotted id (`dock.ada.01H…`) splits
// into `dock`,`ada`,`01H…` and would NOT match `chan/*` → the create-on-post is DENIED. A dash is not a
// grammar delimiter, so `dock-ada-01H…` stays ONE segment and the existing member grant covers it. The
// scope's `dock.` example is updated to `dock-`; the "reserved-prefix, UI-only" idea is unchanged.
//
// FILE-LAYOUT: pure string helpers, no React, no I/O — trivially unit-testable.

/** The reserved dock session-channel id prefix (a `-`, not a `.`, so the id is one cap segment). */
export const DOCK_PREFIX = "dock-";

/** Slugify a principal (`user:ada@acme` → `user-ada-acme`) into an id-safe segment: lowercase, and
 *  every non-`[a-z0-9]` run collapsed to a single `-`, trimmed. Deterministic (no wall-clock) so the
 *  same user always slugs the same — the picker filters on `dock.{slug}.` and must match the mint. */
export function userSlug(principal: string): string {
  return (
    principal
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "") || "anon"
  );
}

/** A monotonic, sortable, ulid-ish id: 48-bit time in base36 + random suffix. Not a spec ULID (no
 *  Crockford base32 / monotonic counter), but lexicographically time-ordered and collision-safe for a
 *  single user's sessions — all the dock needs (the ulid is opaque; only the `dock.{slug}.` prefix is
 *  matched). Kept tiny + dependency-free (the repo ships no ulid lib), mirroring `newRunId`'s style. */
export function mintUlid(now: () => number = () => Date.now(), rand: () => number = Math.random): string {
  const time = now().toString(36).padStart(10, "0");
  const suffix = Math.floor(rand() * 0xffffffff)
    .toString(36)
    .padStart(7, "0");
  return `${time}${suffix}`;
}

/** Mint a fresh dock session channel id for `principal`: `dock-{user-slug}-{ulid}` (one cap segment). */
export function mintDockId(
  principal: string,
  now?: () => number,
  rand?: () => number,
): string {
  return `${DOCK_PREFIX}${userSlug(principal)}-${mintUlid(now, rand)}`;
}

/** The id prefix that scopes a dock session picker to ONE user's own sessions: `dock-{user-slug}-`. */
export function dockPrefixFor(principal: string): string {
  return `${DOCK_PREFIX}${userSlug(principal)}-`;
}

/** Whether `cid` is ANY dock session channel (used to FILTER dock ids OUT of the channels surface —
 *  the dock's storage is not another room in the channel list, per scope non-goal). */
export function isDockChannel(cid: string): boolean {
  return cid.startsWith(DOCK_PREFIX);
}

/** Whether `cid` is one of `principal`'s OWN dock sessions (the picker shows only these — a member
 *  can technically read any workspace channel, but the picker is scoped to the user's own prefix). */
export function isOwnDockChannel(cid: string, principal: string): boolean {
  return cid.startsWith(dockPrefixFor(principal));
}
