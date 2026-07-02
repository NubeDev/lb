// `migrate` — forward-upgrade an OLD persisted `IrSpec.v` to the current `IR_VERSION` on load (the same
// registry-drift class `agent-config` handles). Render-stratum, pure. Today v1 is the only version, so
// this is an identity for well-formed v1 specs and a stamp for a version-less legacy blob; the SWITCH
// exists so the first real migration is a one-line arm, and the golden-file tests pin the shape.
//
// The invariant: a persisted cell must load and render forever unless its GRANT changed — never break
// because a newer node refactored the catalog. `migrate` is where an old shape is reconciled to new.

import type { IrSpec } from "./types";
import { IR_VERSION } from "./types";

/** Upgrade `spec` to the current IR version. Unknown-future versions are returned untouched (validate
 *  will flag them); a missing/zero `v` is treated as the earliest (v1) shape. */
export function migrate(spec: IrSpec): IrSpec {
  let cur: IrSpec = spec.v ? spec : { ...spec, v: 1 };
  // Migration arms run in ascending order; each bumps `cur.v` by one. Add the next as `case 1: ...`.
  while (cur.v < IR_VERSION) {
    switch (cur.v) {
      // case 1: cur = upgradeV1toV2(cur); break;
      default:
        // No arm for this version — cannot advance; hand it back and let validate report.
        return cur;
    }
  }
  return cur;
}
