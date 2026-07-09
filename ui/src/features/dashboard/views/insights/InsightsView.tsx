// `InsightsView` — the `view:"insights"` dashboard tenant of `@nube/insights` (insights-package-scope).
// It mounts the reusable `<InsightsWidget>` over the shell's `insightsClient` (the injected transport
// onto the `insight.*` MCP verbs), folding the cell's `options.insights` into the widget's `filter` +
// `interactive` + `title` props. Read-only by default; `options.insights.readOnly === false` turns on
// inline ack / resolve / dismiss so an end user triages right on the dashboard.
//
// Unlike a viz view this is NOT source-bound: there is no `sources[]`/`usePanelData` — the widget owns
// its own fetch through the client. The host still re-checks `mcp:insight.<verb>:call` + the workspace
// wall on every call the widget makes; the read-only toggle is a UX affordance, never the security gate.
//
// The widget brings its own header/refresh/empty/error/deny surface (self-themed via
// `@nube/insights/style.css`), so this file is a thin frame: the cell title + the widget.

import { useMemo } from "react";
import { InsightsWidget } from "@nube/insights";
import "@nube/insights/style.css";

import type { Cell } from "@/lib/dashboard";
import { insightsClient } from "@/lib/insights/insights.client";
import { insightsFilter, readInsightsOptions } from "./options";

interface Props {
  cell: Cell;
  label?: string;
}

/** Render an insights cell. `label` (the cell title) becomes the widget's panel title; the persisted
 *  `options.insights` drives the filter + read-only vs interactive. */
export function InsightsView({ cell, label }: Props) {
  const opts = useMemo(
    () => readInsightsOptions((cell.options as Record<string, unknown> | undefined)?.insights),
    [cell.options],
  );
  const filter = useMemo(() => insightsFilter(opts), [opts]);

  return (
    <div className="flex h-full min-h-0 flex-col" aria-label={`insights ${label ?? ""}`} data-view="insights">
      <InsightsWidget
        client={insightsClient}
        title={label || "Insights"}
        filter={filter}
        interactive={!opts.readOnly}
        showRefresh={opts.showRefresh}
      />
    </div>
  );
}
