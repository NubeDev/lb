// The wizard step list (panel-wizard scope, step 4) — the ordered, addressable set of wizard steps.
// Kept in its own file so the step nav (PanelWizard) and any future "deep-link to step X" share ONE
// source of truth (the step ids + their labels). One responsibility: the step vocabulary.

export type WizardStepId = "source" | "chartType" | "options" | "transform";

export interface WizardStep {
  id: WizardStepId;
  label: string;
}

/** The ordered wizard steps. Source → ChartType → Options → Transform (Save is the trailing action on
 *  Transform, not a separate step). */
export const WIZARD_STEPS: readonly WizardStep[] = [
  { id: "source", label: "Source" },
  { id: "chartType", label: "Chart type" },
  { id: "options", label: "Options" },
  { id: "transform", label: "Transform" },
] as const;
