// The v2 `slider` control view — a numeric slider that CALLS a write tool on release (widget-builder
// scope, "Control views"). The released value fills the action's `argsTemplate` `{{value}}` slot and
// is written through the bridge (granted write, host-re-checked). Gated + ws-scoped + token-less.

import { useEffect, useMemo, useState } from "react";

import { WidgetHeader, WidgetMessage } from "../widgets/chrome";
import { makeWidgetBridge } from "../builder/widgetBridge";
import { useFlowNodeValue } from "./useFlowNodeValue";
import { flowBindingOfAction } from "./flowBinding";
import { asNumber } from "./num";
import type { Action } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { interpolateArgs, emptyScope } from "@/lib/vars";

interface Props {
  action?: Action;
  tools: string[];
  options?: Record<string, unknown>;
  label?: string;
  scope?: VarScope;
  refreshKey?: number;
}

export function SliderControl({
  action,
  tools,
  options,
  label,
  scope = emptyScope(),
  refreshKey = 0,
}: Props) {
  const toolsKey = tools.join("|");
  // eslint-disable-next-line react-hooks/exhaustive-deps -- re-create the bridge only when the tool SET changes
  const bridge = useMemo(() => makeWidgetBridge(tools), [toolsKey]);
  const min = typeof options?.min === "number" ? (options.min as number) : 0;
  const max = typeof options?.max === "number" ? (options.max as number) : 100;
  const step = typeof options?.step === "number" ? (options.step as number) : 1;
  const [value, setValue] = useState(min);
  const [error, setError] = useState<string | null>(null);

  // A flow-bound slider seeds its current value from its OWN retained input on mount (flow-dashboard-
  // binding-ux-scope) — true state after reload/restart, not the min default.
  const flow = flowBindingOfAction(action);
  const seeded = useFlowNodeValue(flow?.flowId, flow?.node, flow?.port ?? "payload", "input", refreshKey);
  useEffect(() => {
    const n = asNumber(seeded.value);
    if (n !== null) setValue(n);
  }, [seeded.value]);

  async function commit(v: number) {
    if (!action?.tool) return;
    setError(null);
    try {
      await bridge.call(
        action.tool,
        interpolateArgs(action.argsTemplate ?? {}, scope, v) as Record<string, unknown>,
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  return (
    <div className="flex h-full flex-col" aria-label={`slider ${action?.tool ?? ""}`}>
      <WidgetHeader label={label ?? action?.tool ?? "slider"} />
      <div className="flex flex-1 flex-col items-center justify-center gap-2 px-3">
        {/* eslint-disable-next-line no-restricted-syntax -- a native range slider; no shadcn equivalent */}
        <input
          type="range"
          min={min}
          max={max}
          step={step}
          value={value}
          aria-label="slider"
          onChange={(e) => setValue(Number(e.target.value))}
          onMouseUp={() => commit(value)}
          onKeyUp={() => commit(value)}
          className="w-full accent-accent"
        />
        <span className="text-sm text-fg" aria-label="slider value">
          {value}
        </span>
      </div>
      {error && <WidgetMessage tone="denied">{error}</WidgetMessage>}
    </div>
  );
}
