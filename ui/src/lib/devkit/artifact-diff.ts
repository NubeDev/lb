// Diff a pre-build artifact snapshot against a fresh post-build inspect, so the Build step can show
// hard proof a build produced fresh output — "remoteEntry.js  412 KB -> 418 KB (+6 KB, rebuilt)" —
// instead of a bare "built" flag that a stale artifact would satisfy forever. Pure functions only;
// the wizard supplies the two snapshots and the view renders the result.

import type { Artifact } from "./devkit.api";

export type ArtifactChange =
  | "new" // absent before, present now
  | "rebuilt" // mtime advanced (size may or may not have changed)
  | "unchanged" // same mtime as before — this build did NOT write it
  | "missing"; // was present before, gone now (a failed/partial build)

export interface ArtifactDelta {
  kind: Artifact["kind"];
  path: string;
  /** Bytes before the build, or null if the artifact didn't exist yet. */
  before: number | null;
  /** Bytes now, or null if it's gone. */
  after: number | null;
  /** after - before, or null when either side is absent. */
  bytesDelta: number | null;
  change: ArtifactChange;
}

/** A build is CONFIRMED only if every compiled binary/component was freshly written (new or
 *  rebuilt) — a UI remote alone, or an unchanged binary, means the build didn't really run. */
export function buildConfirmed(deltas: ArtifactDelta[]): boolean {
  const binaries = deltas.filter(
    (d) => d.kind === "wasm" || d.kind === "native-bin",
  );
  return (
    binaries.length > 0 &&
    binaries.every((d) => d.change === "new" || d.change === "rebuilt")
  );
}

export function diffArtifacts(
  before: Artifact[],
  after: Artifact[],
): ArtifactDelta[] {
  const priorByPath = new Map(before.map((a) => [a.path, a]));
  const seen = new Set<string>();
  const deltas: ArtifactDelta[] = [];

  for (const now of after) {
    seen.add(now.path);
    const prior = priorByPath.get(now.path);
    deltas.push({
      kind: now.kind,
      path: now.path,
      before: prior ? prior.size : null,
      after: now.size,
      bytesDelta: prior ? now.size - prior.size : null,
      change: classify(prior, now),
    });
  }

  // Anything present before but absent now — a build that removed/failed to reproduce an artifact.
  for (const prior of before) {
    if (seen.has(prior.path)) continue;
    deltas.push({
      kind: prior.kind,
      path: prior.path,
      before: prior.size,
      after: null,
      bytesDelta: null,
      change: "missing",
    });
  }

  return deltas;
}

function classify(prior: Artifact | undefined, now: Artifact): ArtifactChange {
  if (!prior) return "new";
  // mtime is the source of truth for "did this build write it". Size can legitimately be identical
  // across two real builds (deterministic output), so we don't rely on a byte change.
  if (prior.mtime && now.mtime && now.mtime > prior.mtime) return "rebuilt";
  if (!prior.mtime && now.mtime) return "rebuilt";
  return "unchanged";
}

/** Human-readable byte size: 0 B, 512 B, 12.4 KB, 1.8 MB. */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const kb = bytes / 1024;
  if (kb < 1024) return `${round(kb)} KB`;
  return `${round(kb / 1024)} MB`;
}

/** Signed delta, e.g. +6.0 KB / -128 B / 0 B. */
export function formatDelta(bytes: number): string {
  const sign = bytes > 0 ? "+" : bytes < 0 ? "-" : "";
  return `${sign}${formatBytes(Math.abs(bytes))}`;
}

function round(n: number): string {
  return (Math.round(n * 10) / 10).toString();
}
