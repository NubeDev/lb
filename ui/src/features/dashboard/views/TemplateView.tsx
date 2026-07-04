// `TemplateView` — the `view:"template"` dashboard widget, rendered IN-PROCESS (render-template-
// inprocess scope). It is the sibling of `GenUiView`: it reads the panel's rows through the ONE
// `usePanelData` hook (the same path every read view uses), interpolates them into the author HTML via
// the eval-free `interpolateTemplate` (reused verbatim), **sanitizes** the result
// (`sanitizeTemplateHtml` — the security boundary that replaces the sandboxed iframe), and injects it
// into the shell document via `dangerouslySetInnerHTML` inside the standard widget chrome. After commit,
// `[data-call]` buttons are wired to the LEASHED bridge (local `cell.tools` gate + host re-check per
// call; the token never enters this layer) — ported from the deleted iframe `template` engine.
//
// TRUST TIER — IN-PROCESS. The `template` engine runs NO author JavaScript: it is pure `{{path}}`/
// `{{#each}}` interpolation (the eval-free `interpolateTemplate`) over already-gated reads. The iframe
// sandbox bought nothing for `template` and cost a second document + a per-tick postMessage tax + a
// jarring embedded-frame feel. The one real delta the iframe covered — author markup injected into a
// document — is solved by `sanitizeTemplateHtml` (+ the `dashboard.save`/`template.save` authoring cap +
// the `data-*`-only wiring), not by a sandbox. `plot`/`d3` STAY on the iframe tier (they `eval`).
//
// Decisions: 1 (DOMPurify), 4 (iframe `template` branch deleted), 5 (data-*-only wiring + CSP posture).

import { useEffect, useMemo, useRef, useState } from "react";

import type { Cell } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";
import { WidgetHeader, WidgetMessage } from "../widgets/chrome";
import { makeWidgetBridge } from "../builder/widgetBridge";
import { usePanelData } from "../builder/usePanelData";
import { cellTools } from "./WidgetView";
import { interpolateTemplate, type TemplateData } from "../builder/templateInterpolate";
import { sanitizeTemplateHtml } from "../builder/sanitizeTemplateHtml";
import { wireTemplateDataCalls } from "../builder/wireTemplateDataCalls";
import { getTemplate } from "@/lib/dashboard/template.api";

interface Props {
  cell: Cell;
  label?: string;
  scope?: VarScope;
  refreshKey?: number;
}

export function TemplateView({ cell, label, scope = emptyScope(), refreshKey = 0 }: Props) {
  const options = cell.options;
  const tools = cellTools(cell);
  const toolsKey = tools.join("|");
  // eslint-disable-next-line react-hooks/exhaustive-deps -- re-create the bridge only when the tool SET changes
  const bridge = useMemo(() => makeWidgetBridge(tools), [toolsKey]);

  // The panel's rows through the ONE data hook — the SAME path every read view uses, so source-picker,
  // variables, watch/refresh, and the deny/empty states are all inherited (no template-specific data
  // code). A `rules.run` source reaches here exactly like a `series.read`/`store.query` source.
  const state = usePanelData(cell, scope, refreshKey);

  // Inline-vs-Saved code resolution, lifted verbatim from ScriptedView: inline `options.code` (≤4 KB)
  // wins; else `options.templateId` → `template.get` (≤64 KB). `code === null` until it resolves.
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

  // The render pipeline: interpolate (eval-free, escapes DATA values) → sanitize (strips author-markup
  // XSS vectors, keeps `data-call`/`data-args`). Memoized on the inputs so the post-commit wiring effect
  // below re-runs only when the markup actually changes (stable identity for an identical render).
  const html = useMemo(() => {
    if (code == null) return "";
    const data: TemplateData = {
      rows: state.rows,
      latest: state.latest,
      loading: state.loading,
      denied: state.denied,
    };
    return sanitizeTemplateHtml(interpolateTemplate(code, data));
  }, [code, state.rows, state.latest, state.loading, state.denied]);

  // Post-commit: wire `[data-call]` clicks in the committed subtree to the leashed bridge. Re-runs when
  // the markup or the bridge changes; cleans up on unmount/re-wiring (no listener leak, no double-fire).
  const containerRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const node = containerRef.current;
    if (!node || !html) return;
    return wireTemplateDataCalls(node, bridge, tools);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `html` is the content signature; `toolsKey` covers tools
  }, [html, bridge, toolsKey]);

  if (error) return <WidgetMessage tone="denied">{error}</WidgetMessage>;
  if (code === null) return <WidgetMessage tone="muted">no template</WidgetMessage>;
  // Standard denied panel — parity with every other view. The template author cannot render their way
  // around a denied source; the host capability wall is the truth and it shows here, honestly.
  if (state.denied) {
    return (
      <div className="flex h-full flex-col" data-view="template">
        <WidgetHeader label={label ?? ""} />
        <WidgetMessage tone="denied">no access to this source</WidgetMessage>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col" aria-label={`template ${label ?? ""}`} data-view="template">
      <WidgetHeader label={label ?? ""} />
      {/*
        `dangerouslySetInnerHTML` is SAFE here because `html` is the OUTPUT of `sanitizeTemplateHtml`
        (DOMPurify + our config) over `interpolateTemplate`'s already-escaped output. This is the
        sanctioned sink for the template render path; the sanitizer is the load-bearing wall and the
        XSS-vector suite is its definition of done. The post-commit effect wires `[data-call]` clicks
        through the leashed bridge — no inline handler is ever read/executed (Decision 5).
      */}
      <div
        ref={containerRef}
        className="min-h-0 flex-1 overflow-auto text-xs"
        dangerouslySetInnerHTML={{ __html: html }}
      />
    </div>
  );
}
