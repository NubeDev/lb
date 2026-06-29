// The v3 record bounds, mirrored from the host (viz panel-model scope, "Data" caps). The HOST is the
// authority (`dashboard/bounds.rs` rejects an over-cap save); the editor mirrors these so it can warn
// the author BEFORE a save and refuse to add a 65th override. Kept in one named file so the numbers
// have a single owner on the UI side and never drift inline across the tabs.

/** Max client-side transformations per panel. */
export const MAX_TRANSFORMS = 32;
/** Max per-field overrides in a panel's `fieldConfig`. */
export const MAX_OVERRIDES = 64;
/** Max value mappings on one field option set. */
export const MAX_MAPPINGS = 64;
/** Max threshold steps on one field option set. */
export const MAX_THRESHOLD_STEPS = 64;
