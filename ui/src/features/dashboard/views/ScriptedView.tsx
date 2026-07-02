// The v2 scripted view (`plot` / `d3` / `template`) — author code over the source rows, rendered in a
// SANDBOXED IFRAME (widget-builder scope, "Scripted views"). The code is either inline in
// `options.code` (small snippets, ≤ a few KB) or a durable `render_template:{id}` referenced by
// `options.templateId` (loaded via `template.get`). The iframe may WRITE a granted tool — the sandbox +
// the cell's tool set + the host re-check are the three guards. The token never crosses into the frame.

import { useEffect, useMemo, useState } from "react";

import type { Cell } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";
import { WidgetMessage } from "../widgets/chrome";
import { makeWidgetBridge } from "../builder/widgetBridge";
import { WidgetIframe } from "../builder/WidgetIframe";
import { usePanelData } from "../builder/usePanelData";
import { cellTools } from "./WidgetView";
import { getTemplate } from "@/lib/dashboard/template.api";

interface Props {
  cell: Cell;
  engine: "plot" | "d3" | "template";
  scope?: VarScope;
  refreshKey?: number;
}

export function ScriptedView({ cell, engine, scope = emptyScope(), refreshKey = 0 }: Props) {
  const options = cell.options;
  const tools = cellTools(cell);
  const toolsKey = tools.join("|");
  // eslint-disable-next-line react-hooks/exhaustive-deps -- re-create the bridge only when the tool SET changes
  const bridge = useMemo(() => makeWidgetBridge(tools), [toolsKey]);
  // The panel's rows through the ONE data hook — the SAME path every read view uses, so a template/plot/
  // d3 view is data-driven identically on a dashboard and in a channel response (both build a `Cell`).
  const state = usePanelData(cell, scope, refreshKey);
  const inline = typeof options?.code === "string" ? (options.code as string) : null;
  const templateId = typeof options?.templateId === "string" ? (options.templateId as string) : null;
  const [code, setCode] = useState<string | null>(inline);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (inline !== null) {
      setCode(inline);
      return;
    }
    if (!templateId) {
      setCode(null);
      return;
    }
    let cancelled = false;
    setError(null);
    getTemplate(templateId)
      .then((t) => {
        if (!cancelled) setCode(t.code);
      })
      .catch((e) => {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [inline, templateId]);

  if (error) return <WidgetMessage tone="denied">{error}</WidgetMessage>;
  if (code === null) return <WidgetMessage tone="muted">no template</WidgetMessage>;

  return (
    <div className="h-full w-full" data-scripted={engine}>
      <WidgetIframe
        engine={engine}
        code={code}
        tools={tools}
        bridge={bridge}
        data={{ rows: state.rows, latest: state.latest, loading: state.loading, denied: state.denied }}
      />
    </div>
  );
}
