// InsightsBasics (panel-wizard scope) — the per-view "basics" for the `insights` view on the wizard's
// Chart type step (step 2), mirroring `StatBasics`/the genui author section. The insights panel is not
// source-bound, so its "basics" are the two choices an author makes most: READ-ONLY vs interactive
// (the headline decision — the user asked for it here on step 2) and the status/severity focus. The
// full option set (limit, refresh, both facets) still lives on step 3 (Options); this is the fast path.
//
// No local state — every control reads/writes through `patch` against the wizard's EditorState nested
// `options.insights.*` block (the same path `readInsightsOptions` + the option registry use), so step 2
// and step 3 can never drift. One responsibility: the insights basics form.

import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import { Checkbox } from "@/components/ui/checkbox";
import { Select } from "@/components/ui/select";
import {
  defaultInsightsOptions,
  readInsightsOptions,
  type InsightsOptions,
} from "@/features/dashboard/views/insights/options";

interface Props {
  state: EditorState;
  patch: (next: Partial<EditorState>) => void;
}

/** Merge a partial insights-options patch into `options.insights`, preserving the rest of `options`. */
function patchInsights(state: EditorState, next: Partial<InsightsOptions>): Partial<EditorState> {
  const current = readInsightsOptions((state.options as Record<string, unknown> | undefined)?.insights);
  return {
    options: {
      ...(state.options as Record<string, unknown> | undefined),
      insights: { ...current, ...next },
    },
  };
}

export function InsightsBasics({ state, patch }: Props) {
  const opts = readInsightsOptions((state.options as Record<string, unknown> | undefined)?.insights) ??
    defaultInsightsOptions();

  return (
    <div className="grid gap-3" aria-label="wizard insights basics">
      {/* Read-only vs interactive — the headline choice. A checked box = end users can act in place. */}
      <label className="flex items-start gap-2.5 rounded-md border border-border p-2.5">
        <Checkbox
          aria-label="allow acknowledge"
          checked={!opts.readOnly}
          onChange={(e) => patch(patchInsights(state, { readOnly: !e.target.checked }))}
          className="mt-0.5"
        />
        <span className="grid gap-0.5">
          <span className="text-xs font-medium text-fg">Let viewers acknowledge &amp; resolve</span>
          <span className="text-[11px] text-muted">
            On, each row gets Ack / Resolve / Dismiss. Off (default), the panel is read-only — a
            glanceable list. Every action is still permission-checked on the server.
          </span>
        </span>
      </label>

      <div className="grid grid-cols-2 gap-2">
        <label className="grid gap-1 text-[11px] text-muted">
          Status
          <Select
            aria-label="insights status filter"
            className="h-8 w-full"
            value={opts.status}
            onChange={(e) => patch(patchInsights(state, { status: e.target.value as InsightsOptions["status"] }))}
          >
            <option value="all">All</option>
            <option value="open">Open</option>
            <option value="acked">Acknowledged</option>
            <option value="resolved">Resolved</option>
          </Select>
        </label>
        <label className="grid gap-1 text-[11px] text-muted">
          Severity
          <Select
            aria-label="insights severity filter"
            className="h-8 w-full"
            value={opts.severity}
            onChange={(e) => patch(patchInsights(state, { severity: e.target.value as InsightsOptions["severity"] }))}
          >
            <option value="all">All</option>
            <option value="info">Info</option>
            <option value="warning">Warning</option>
            <option value="critical">Critical</option>
          </Select>
        </label>
      </div>
    </div>
  );
}
