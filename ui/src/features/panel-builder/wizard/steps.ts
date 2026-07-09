// The wizard step list (panel-wizard scope, step 4) — the ordered, addressable set of wizard steps.
// Kept in its own file so the step nav (PanelWizard) and any future "deep-link to step X" share ONE
// source of truth (the step ids + their labels). One responsibility: the step vocabulary.

export type WizardStepId = "source" | "chartType" | "options" | "transform";

export interface WizardStep {
  id: WizardStepId;
  label: string;
}

/** The sentinel dashboard id the wizard opens under when it is launched WITHOUT a destination dashboard
 *  (the "Create panel" action on the Datasources query page). The wizard resolves it to the same single
 *  deep-linkable `new-panel` route — no duplicate wiring — and, seeing the sentinel, renders a
 *  "Save into which dashboard?" picker on its Save step instead of assuming a target. A real dashboard
 *  id never collides: dashboard ids are slugs and this one is bracketed. */
export const PICK_DASHBOARD = "__pick__";

/** The ordered wizard steps. Source → ChartType → Options → Transform (Save is the trailing action on
 *  Transform, not a separate step). */
export const WIZARD_STEPS: readonly WizardStep[] = [
  { id: "source", label: "Source" },
  { id: "chartType", label: "Chart type" },
  { id: "options", label: "Options" },
  { id: "transform", label: "Transform" },
] as const;
