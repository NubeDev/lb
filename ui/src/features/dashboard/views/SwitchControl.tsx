// The v2 `switch` control view — a boolean toggle that CALLS a write tool (widget-builder scope,
// "Control views"). On toggle it fills the action's `argsTemplate` `{{value}}` slot with the new bool
// and calls `action.tool` through the bridge (a granted write — the host re-checks the cap + ws). It
// MAY optionally read its own current state from `source` (open-Q5 lean: yes, optional). The control
// never holds a token; the write is gated server-side exactly like any other.

import { useEffect, useMemo, useState } from "react";

import { WidgetHeader, WidgetMessage } from "../widgets/chrome";
import { makeWidgetBridge } from "../builder/widgetBridge";
import { useSource } from "../builder/useSource";
import { asNumber } from "./num";
import { fillArgs } from "./argsTemplate";
import type { Action, Source } from "@/lib/dashboard";

interface Props {
  source?: Source;
  action?: Action;
  tools: string[];
  options?: Record<string, unknown>;
  label?: string;
}

export function SwitchControl({ source, action, tools, label }: Props) {
  const toolsKey = tools.join("|");
  // eslint-disable-next-line react-hooks/exhaustive-deps -- re-create the bridge only when the tool SET changes
  const bridge = useMemo(() => makeWidgetBridge(tools), [toolsKey]);
  const { latest } = useSource(source, tools); // optional self-state read
  const [on, setOn] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Reflect the read state into the toggle when the source reports it.
  useEffect(() => {
    const n = asNumber(latest);
    if (n !== null) setOn(n !== 0);
    else if (typeof latest === "boolean") setOn(latest);
  }, [latest]);

  async function toggle() {
    if (!action?.tool) return;
    const next = !on;
    setOn(next);
    setError(null);
    try {
      await bridge.call(action.tool, fillArgs(action.argsTemplate, next));
    } catch (e) {
      setOn(!next); // revert on a denied/failed write — no fake success
      setError(e instanceof Error ? e.message : String(e));
    }
  }

  return (
    <div className="flex h-full flex-col" aria-label={`switch ${action?.tool ?? ""}`}>
      <WidgetHeader label={label ?? action?.tool ?? "switch"} />
      <div className="flex flex-1 items-center justify-center">
        {/* eslint-disable-next-line no-restricted-syntax -- a toggle track, not a shadcn Button shape */}
        <button
          type="button"
          role="switch"
          aria-checked={on}
          aria-label="toggle"
          onClick={toggle}
          className={`relative h-7 w-12 rounded-full transition-colors ${on ? "bg-accent" : "bg-border"}`}
        >
          <span
            className={`absolute top-0.5 h-6 w-6 rounded-full bg-white transition-transform ${on ? "translate-x-5" : "translate-x-0.5"}`}
          />
        </button>
      </div>
      {error && <WidgetMessage tone="denied">{error}</WidgetMessage>}
    </div>
  );
}
