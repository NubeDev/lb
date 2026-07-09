// The Agent wizard (setup scope) — walks a user through setting up the workspace AI agent, one page
// per key part, with a plain-language intro of what each is for. It is PURE ORCHESTRATION over the
// existing Settings › Agent editors: the definition catalog (`AgentCatalogSection`) and the persona
// section (`PersonaSection`, which itself carries the roster, the Effective-tools view, and the
// Permissions pane). The wizard adds NO new backend and duplicates NO editor — it only sequences the
// real editors and frames each with a title + a "what this is for" blurb. That reuse is the point:
// one editor, two homes (the Settings tab and this wizard). One responsibility per file (FILE-LAYOUT).
//
// The mental model this wizard teaches: an agent run = WHO RUNS (a definition: runtime + model) ×
// WHAT FOR (a persona: its focused tool menu + skills + identity), bounded by TOOLS (the live
// persona ∩ agent ∩ caller reachable set) and supervised by PERMISSIONS (per-tool Allow / Ask / Deny).
// Nothing here widens the wall — the gateway re-checks every write (rule 5); cap-gating only hides
// controls the caller couldn't use anyway.

import { Bot, Cpu, ListChecks, ShieldCheck, UserCog } from "lucide-react";

import { AgentCatalogSection } from "@/features/settings/agent/AgentCatalogSection";
import { PersonaSection } from "@/features/settings/agent/PersonaSection";
import { StepFlow, StepShell, type FlowStep } from "../wizard/StepFlow";

interface Props {
  ws: string;
  caps: string[] | undefined;
  onDone?: () => void;
}

export function AgentWizard({ caps, onDone }: Props) {
  const steps: FlowStep[] = [
    {
      key: "intro",
      label: "Overview",
      hint: "What the agent is",
      render: () => (
        <StepShell
          icon={Bot}
          title="Set up the workspace agent"
          blurb="Your workspace has a built-in AI agent that members can run from the dock — to analyse data, draft dashboards, answer questions, and drive tools. This wizard walks the four parts that define what it is and what it may do."
        >
          <div className="space-y-3 text-sm text-fg">
            <p className="text-muted">
              One mental model runs through every screen:{" "}
              <span className="font-medium text-fg">who runs × what for</span>, bounded by what it can
              reach and how it&apos;s supervised.
            </p>
            <ul className="space-y-2">
              <IntroItem
                icon={Cpu}
                title="Definition — who runs"
                body="A named runtime + model preset (e.g. the in-house loop over Z.AI GLM-4.5). Pick one as the workspace default; add your own or seal a model key on it."
              />
              <IntroItem
                icon={UserCog}
                title="Persona — what for"
                body="A focused job: its own curated tool menu, pinned skills, and identity. The roster picks which personas the dock can suggest; defaults live in your preferences."
              />
              <IntroItem
                icon={ListChecks}
                title="Tools — what it can reach"
                body="The live persona ∩ agent ∩ caller set: exactly which tools a run can actually call. Read-only — it shows the boundary, it doesn't grant it."
              />
              <IntroItem
                icon={ShieldCheck}
                title="Permissions — how it's supervised"
                body="Per-tool Allow / Ask / Deny. This tightens what the agent does unattended; it never grants a capability the caller lacks."
              />
            </ul>
          </div>
        </StepShell>
      ),
    },
    {
      key: "definition",
      label: "Definition",
      hint: "Runtime + model",
      render: () => (
        <StepShell
          icon={Cpu}
          title="Pick who runs — the definition"
          blurb="Choose the runtime + model the agent loop uses. The selected runtime applies to runs now; a per-workspace model endpoint applies once a provider adapter is configured. Built-ins are read-only; add your own with New definition."
        >
          <AgentCatalogSection caps={caps} />
        </StepShell>
      ),
    },
    {
      key: "persona",
      label: "Persona",
      hint: "What for + tools + permissions",
      render: () => (
        <StepShell
          icon={UserCog}
          title="Shape what it's for — persona, tools & supervision"
          blurb="A persona gives the agent a focused tool menu, pinned skills, and an identity. Below the roster you'll see the Effective tools it can actually reach, and the Permissions (Allow / Ask / Deny) that supervise each one. Editing a persona changes what the agent is shown — never what it may reach."
        >
          <PersonaSection caps={caps} />
        </StepShell>
      ),
    },
  ];

  return <StepFlow steps={steps} finishLabel="Finish" onFinish={onDone} />;
}

// A single row in the intro's key-parts list — an icon badge, the part name, and its one-liner.
function IntroItem({
  icon: Icon,
  title,
  body,
}: {
  icon: typeof Cpu;
  title: string;
  body: string;
}) {
  return (
    <li className="flex items-start gap-3 rounded-md border border-border bg-panel px-3 py-2.5">
      <span className="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-primary/10 text-primary">
        <Icon size={15} />
      </span>
      <div className="min-w-0">
        <span className="block text-sm font-medium text-fg">{title}</span>
        <span className="mt-0.5 block text-xs text-muted">{body}</span>
      </div>
    </li>
  );
}
