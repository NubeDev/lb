// The v2 `slider` control view — a numeric slider that CALLS a write tool on release (widget-builder
// scope, "Control views"). The released value fills the action's `argsTemplate` `{{value}}` slot and
// is written through the bridge (granted write, host-re-checked). Gated + ws-scoped + token-less.

import { useMemo, useState } from "react";

import { WidgetHeader, WidgetMessage } from "../widgets/chrome";
import { makeWidgetBridge } from "../builder/widgetBridge";
import { fillArgs } from "./argsTemplate";
import type { Action } from "@/lib/dashboard";

interface Props {
  action?: Action;
  tools: string[];
  options?: Record<string, unknown>;
  label?: string;
}

export function SliderControl({ action, tools, options, label }: Props) {
  const bridge = useMemo(() => makeWidgetBridge(tools), [tools.join("|")]);
  const min = typeof options?.min === "number" ? (options.min as number) : 0;
  const max = typeof options?.max === "number" ? (options.max as number) : 100;
  const [value, setValue] = useState(min);
  const [error, setError] = useState<string | null>(null);

  async function commit(v: number) {
    if (!action?.tool) return;
    setError(null);
    try {
      await bridge.call(action.tool, fillArgs(action.argsTemplate, v));
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
