// The Setup catalog (setup scope) — the list of guided wizards the Setup tab offers. The tab lands on
// an icon-card grid over THIS list, not straight into one wizard, because more than one wizard lives
// here (onboarding today; more to come). Adding a wizard = one entry + one component branch in
// `SetupHub`; the card grid renders whatever is listed. One responsibility per file (FILE-LAYOUT):
// this file owns only the catalog shape + entries — no markup, no verbs.

import { Bot, Database, LayoutTemplate, Lightbulb, Palette, Rocket } from "lucide-react";
import type { LucideIcon } from "lucide-react";

export type WizardId = "onboard" | "appearance" | "ingest" | "agent" | "datasource" | "template";

export interface WizardEntry {
  id: WizardId;
  /** Card title. */
  title: string;
  /** One-line description shown under the title. */
  blurb: string;
  icon: LucideIcon;
}

export const WIZARDS: WizardEntry[] = [
  {
    id: "onboard",
    title: "Onboard a person",
    blurb: "Get one person from “doesn’t exist” to logging in and seeing the right pages.",
    icon: Rocket,
  },
  {
    id: "appearance",
    title: "Set up appearance",
    blurb: "Walk through theme, layout, sidebar, and branding — how the UI will look.",
    icon: Palette,
  },
  {
    id: "ingest",
    title: "Push in data",
    blurb: "Create a series, mint an API key, and copy a Python example that writes samples.",
    icon: Database,
  },
  {
    id: "agent",
    title: "Set up the agent",
    blurb: "Walk the workspace AI agent — its definition, persona, tools, and permissions.",
    icon: Bot,
  },
  {
    id: "datasource",
    title: "Data to insight",
    blurb: "The full path — connect a datasource, query it, chart it on a dashboard, then watch it with a rule that raises an insight.",
    icon: Lightbulb,
  },
  {
    id: "template",
    title: "Build a render-template widget",
    blurb: "Query the buildings demo, then design a custom HTML widget for the rows — draft it with an AI, preview it live, and save it as a reusable template.",
    icon: LayoutTemplate,
  },
];
