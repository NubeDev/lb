// The Appearance wizard (setup scope) — walks a user through how the UI will look, in four steps:
// Theme (presets/mode/brand colors), Layout (sidebar variant/collapsible/side), Sidebar (which pages
// show), and Branding (workspace identity — admin only). It is PURE ORCHESTRATION over the existing
// self-contained Settings editors: each step renders the SAME component the Settings tab renders
// (`ThemeTab`, `LayoutTab`, `SidebarTab`, `BrandingTab`), which owns its own load / live-apply /
// persist against the real gateway. The wizard adds no new backend and duplicates no editor logic —
// it only sequences them and frames each with a title. That reuse is the point (one editor, two
// homes: the Settings tab and this wizard). One responsibility per file (FILE-LAYOUT).

import { Building2, LayoutTemplate, Palette, PanelLeft } from "lucide-react";

import { CAP, hasCap } from "@/lib/session";
import { LayoutTab, ThemeTab } from "@/features/theme";
import { BrandingTab } from "../../settings/BrandingTab";
import { SidebarTab } from "../../settings/SidebarTab";
import { StepFlow, StepShell, type FlowStep } from "../wizard/StepFlow";

interface Props {
  ws: string;
  caps: string[] | undefined;
  /** Called when the wizard is finished (e.g. return to the picker). */
  onDone?: () => void;
}

export function AppearanceWizard({ ws, caps, onDone }: Props) {
  // Branding writes the workspace default (admin-only); show that step only when the user could save it.
  const canBrand = hasCap(caps, CAP.prefsSetDefault);

  const steps: FlowStep[] = [
    {
      key: "theme",
      label: "Theme",
      hint: "Colors & mode",
      render: () => (
        <StepShell
          icon={Palette}
          title="Pick your theme"
          blurb="Choose a preset, light or dark mode, corner radius, and brand colors. Changes apply live — you'll see them immediately."
        >
          <ThemeTab />
        </StepShell>
      ),
    },
    {
      key: "layout",
      label: "Layout",
      hint: "Sidebar shape",
      render: () => (
        <StepShell
          icon={LayoutTemplate}
          title="Shape the sidebar"
          blurb="How the sidebar sits: default, floating, or inset; how it collapses; and which side it's on."
        >
          <LayoutTab />
        </StepShell>
      ),
    },
    {
      key: "sidebar",
      label: "Sidebar",
      hint: "Which pages show",
      render: () => (
        <StepShell
          icon={PanelLeft}
          title="Choose what's in the sidebar"
          blurb="Show or hide pages in the workspace sidebar. A hidden page is still reachable by URL — this only curates the menu."
        >
          <SidebarTab ws={ws} caps={caps} />
        </StepShell>
      ),
    },
    ...(canBrand
      ? [
          {
            key: "branding",
            label: "Branding",
            hint: "Workspace identity",
            render: () => (
              <StepShell
                icon={Building2}
                title="Set the workspace brand"
                blurb="The name, abbreviation, tagline, and logo every member sees in the shell and on the login page."
              >
                <BrandingTab ws={ws} caps={caps} />
              </StepShell>
            ),
          } satisfies FlowStep,
        ]
      : []),
  ];

  return <StepFlow steps={steps} finishLabel="Finish" onFinish={onDone} />;
}
