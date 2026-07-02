// THE one declared fake in this package (thecrew-scope.md §reuse #2): a deterministic
// plant-value simulator behind the ValueSource seam. Allowed because there is no node
// here at all; the framework replaces this file with the bridge. Do not add others.

import type { ValueSource } from "./value-source";

// TODO(phase 1): channels like "ahu1.sf1.running|speed|fault", "zone.101.temp" —
// smooth plausible waveforms + occasional fault flips, seeded (deterministic for tests).
export function createSimulator(): ValueSource {
  throw new Error("TODO(phase 1): createSimulator");
}
