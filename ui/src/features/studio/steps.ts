// The three stages of the Studio pipeline, in order. Shared by the stepper spine and the step views
// so labels/icons never drift between them.

import { Hammer, Rocket, Sparkles } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import type { StudioStep } from "./studio.wizard";

export interface StepMeta {
  n: StudioStep;
  label: string;
  hint: string;
  icon: LucideIcon;
}

export const STUDIO_STEPS: StepMeta[] = [
  { n: 1, label: "Create", hint: "Scaffold or open a folder", icon: Sparkles },
  { n: 2, label: "Build", hint: "Compile the artifact", icon: Hammer },
  { n: 3, label: "Publish", hint: "Register to the workspace", icon: Rocket },
];
