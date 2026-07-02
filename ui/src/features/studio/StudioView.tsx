// Built-in Extension Studio. A shell-local view (not an extension), so the chicken-and-egg SDK path
// can scaffold/build/publish the first extension through the real gateway.
//
// The Studio is a strict linear pipeline — create -> build -> publish — so it's shaped as a guided
// three-step wizard rather than one panel of co-equal controls. The stepper shows where you are; the
// footer always holds the single forward action; steps you haven't earned yet aren't reachable. All
// state and side effects live in `useStudioWizard`; everything here is layout.

import { RotateCcw, Wrench } from "lucide-react";

import { AppPageHeader } from "@/components/app/page-header";
import { Button } from "@/components/ui/button";
import { BuildStep } from "./BuildStep";
import { CreateStep } from "./CreateStep";
import { PublishStep } from "./PublishStep";
import { Stepper } from "./Stepper";
import { STUDIO_STEPS } from "./steps";
import { WizardFooter } from "./WizardFooter";
import { useStudioWizard } from "./studio.wizard";

interface Props {
  ws: string;
}

export function StudioView({ ws }: Props) {
  const w = useStudioWizard();
  const active = STUDIO_STEPS[w.step - 1];

  return (
    <div className="flex h-full min-h-0 flex-col bg-bg">
      <AppPageHeader
        icon={Wrench}
        title="Extension Studio"
        description="Scaffold, build, and publish a local extension"
        workspace={ws}
        actions={
          w.step > 1 && (
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={w.reset}
              className="gap-1.5 text-muted hover:text-fg"
            >
              <RotateCcw size={13} />
              Start over
            </Button>
          )
        }
      />

      <main className="flex min-h-0 flex-1 flex-col overflow-hidden px-4">
        <div className="shrink-0 py-6">
          <Stepper step={w.step} onGoBack={w.goTo} />
        </div>

        <div className="mx-auto flex min-h-0 w-full max-w-2xl flex-1 flex-col overflow-hidden">
          <div className="mb-4 shrink-0">
            <h2 className="text-base font-semibold text-fg">{active.label}</h2>
            <p className="mt-0.5 text-xs text-muted">{active.hint}</p>
          </div>

          <div
            key={w.step}
            className="wizard-panel flex min-h-0 flex-1 flex-col overflow-y-auto pb-2"
          >
            {w.step === 1 && <CreateStep w={w} />}
            {w.step === 2 && <BuildStep w={w} />}
            {w.step === 3 && <PublishStep w={w} />}
          </div>

          <WizardFooter w={w} />
        </div>
      </main>
    </div>
  );
}
