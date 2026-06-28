// The v2 `button` control view — a momentary action button that CALLS a write tool (widget-builder
// scope, "Control views"). Clicking fills the action's `argsTemplate` (the `{{value}}` slot, if any,
// from `options.value`) and writes through the bridge. The button reports the real outcome (sent /
// denied), never a fake success.

import { useMemo, useState } from "react";

import { Button } from "@/components/ui/button";
import { WidgetHeader } from "../widgets/chrome";
import { makeWidgetBridge } from "../builder/widgetBridge";
import type { Action } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { interpolateArgs, emptyScope } from "@/lib/vars";

interface Props {
  action?: Action;
  tools: string[];
  options?: Record<string, unknown>;
  label?: string;
  scope?: VarScope;
}

export function ButtonControl({ action, tools, options, label, scope = emptyScope() }: Props) {
  const toolsKey = tools.join("|");
  // eslint-disable-next-line react-hooks/exhaustive-deps -- re-create the bridge only when the tool SET changes
  const bridge = useMemo(() => makeWidgetBridge(tools), [toolsKey]);
  const [status, setStatus] = useState<"idle" | "sent" | "error">("idle");
  const [error, setError] = useState<string | null>(null);

  async function fire() {
    if (!action?.tool) return;
    setError(null);
    try {
      await bridge.call(
        action.tool,
        interpolateArgs(action.argsTemplate ?? {}, scope, options?.value) as Record<string, unknown>,
      );
      setStatus("sent");
    } catch (e) {
      setStatus("error");
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  return (
    <div className="flex h-full flex-col" aria-label={`button ${action?.tool ?? ""}`}>
      <WidgetHeader label={label ?? action?.tool ?? "button"} />
      <div className="flex flex-1 flex-col items-center justify-center gap-2">
        <Button type="button" variant="solid" size="sm" aria-label="fire" onClick={fire}>
          {(typeof options?.buttonLabel === "string" && options.buttonLabel) || "Run"}
        </Button>
        {status === "sent" && <span className="text-xs text-muted" aria-label="button status">sent</span>}
        {status === "error" && <span className="text-xs text-red-400" aria-label="button status">{error}</span>}
      </div>
    </div>
  );
}
