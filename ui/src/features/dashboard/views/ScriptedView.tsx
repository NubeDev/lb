// The v2 scripted view (`plot` / `d3` / `template`) — author code over the source rows, rendered in a
// SANDBOXED IFRAME (widget-builder scope, "Scripted views"). The code is either inline in
// `options.code` (small snippets, ≤ a few KB) or a durable `render_template:{id}` referenced by
// `options.templateId` (loaded via `template.get`). The iframe may WRITE a granted tool — the sandbox +
// the cell's tool set + the host re-check are the three guards. The token never crosses into the frame.

import { useEffect, useMemo, useState } from "react";

import { WidgetMessage } from "../widgets/chrome";
import { makeWidgetBridge } from "../builder/widgetBridge";
import { WidgetIframe } from "../builder/WidgetIframe";
import { getTemplate } from "@/lib/dashboard/template.api";

interface Props {
  engine: "plot" | "d3" | "template";
  tools: string[];
  options?: Record<string, unknown>;
}

export function ScriptedView({ engine, tools, options }: Props) {
  const toolsKey = tools.join("|");
  // eslint-disable-next-line react-hooks/exhaustive-deps -- re-create the bridge only when the tool SET changes
  const bridge = useMemo(() => makeWidgetBridge(tools), [toolsKey]);
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
      <WidgetIframe engine={engine} code={code} tools={tools} bridge={bridge} />
    </div>
  );
}
