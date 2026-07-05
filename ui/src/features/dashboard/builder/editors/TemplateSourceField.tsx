// The JSX `template` source field (widget-builder Slice B; render-template-inprocess scope) — ported
// from rubix-cube's `TemplateSourceField.tsx`, with its SWR/`next`/REST data layer swapped for the
// shipped `template.*` MCP verbs over the bridge. The author either writes inline JSX/HTML (saved into
// `cell.options.code`, ≤ INLINE_MAX_BYTES) OR picks a saved `render_template:{id}` row (saved into
// `cell.options.templateId`, ≤ TEMPLATE_MAX_BYTES). The saved-template list reads `template.list`
// (the shipped verb) — never REST.
//
// The lazybones `template` engine is the eval-free HTML interpreter: `{{path}}` +
// `{{#each rows}}…{{/each}}` interpolation over the panel's source rows (loaded by the ONE data hook,
// `usePanelData` — the SAME data every read view gets) + `[data-call]` write buttons. The default inline
// snippet matches it. Code renders IN-PROCESS (`TemplateView`), sanitized by `sanitizeTemplateHtml`
// before it touches the DOM (the iframe sandbox is no longer used for `template`; `plot`/`d3` still do).

import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import { listTemplates, type RenderTemplateSummary } from "@/lib/dashboard/template.api";
import { CodeEditor } from "./CodeEditor";

/** A working default inline template — latest meter readings over the seeded building data
 *  (`docker/postgres/seed.py`, also the SQLite `demo-buildings` datasource: `point_reading` is
 *  `(time, point_id, value)`). Bind a datasource query like the last-N-per-meter window:
 *  `SELECT * FROM (
 *     SELECT *, ROW_NUMBER() OVER (PARTITION BY point_id ORDER BY time DESC) AS rn
 *     FROM point_reading WHERE point_id IN ('meter-001-kwh','meter-002-kwh')
 *   ) t WHERE rn <= 100` — the template sees exactly those rows: `{{rows.length}}` the count,
 *  `{{latest.field}}` the last row, `{{#each rows}}` one block per row with `{{point_id}}`/`{{time}}`/
 *  `{{value}}`/`{{rn}}` reading that row's fields. The `[data-call]` button is a host-mediated write
 *  (add the tool to the cell's tools); remove it if the panel is read-only. */
// Inline STYLES (not exotic Tailwind classes) on purpose: template HTML is injected at runtime, so a
// class the shell never compiled simply doesn't exist — theme tokens via hsl(var(--…)) keep it native
// in both themes. The sanitizer strips style-expression vectors; plain declarations pass.
export const DEFAULT_INLINE_CODE = `<div style="display:flex;flex-direction:column;gap:10px;padding:12px;font-size:11px;color:hsl(var(--fg))">
  <div style="position:relative;overflow:hidden;border-radius:10px;border:1px solid hsl(var(--accent)/0.35);background:linear-gradient(135deg,hsl(var(--accent)/0.18),hsl(var(--panel)) 65%);padding:14px 16px">
    <div style="font-size:9px;letter-spacing:0.14em;text-transform:uppercase;color:hsl(var(--accent))">● Live · latest reading</div>
    <div style="display:flex;align-items:baseline;gap:10px;margin-top:4px">
      <span style="font-size:30px;font-weight:700;font-variant-numeric:tabular-nums;color:hsl(var(--accent))">{{latest.value}}</span>
      <span style="font-family:monospace;font-size:10px;padding:2px 8px;border-radius:999px;border:1px solid hsl(var(--accent)/0.4);color:hsl(var(--accent))">{{latest.point_id}}</span>
    </div>
    <div style="margin-top:6px;color:hsl(var(--muted))">{{rows.length}} readings · epoch {{latest.time}}</div>
  </div>
  <div style="display:flex;flex-direction:column;gap:4px;overflow-y:auto">
    {{#each rows}}<div style="display:flex;align-items:center;gap:10px;border-radius:8px;border:1px solid hsl(var(--border));background:hsl(var(--panel));padding:6px 10px">
      <span style="width:3px;align-self:stretch;border-radius:2px;background:hsl(var(--accent)/0.7)"></span>
      <span style="font-family:monospace;color:hsl(var(--muted))">{{point_id}}</span>
      <span style="margin-left:auto;font-weight:600;font-variant-numeric:tabular-nums">{{value}}</span>
      <span style="font-variant-numeric:tabular-nums;color:hsl(var(--muted))">t={{time}}</span>
      <span style="font-size:9px;padding:1px 6px;border-radius:999px;background:hsl(var(--accent)/0.12);color:hsl(var(--accent))">#{{rn}}</span>
    </div>{{/each}}
  </div>
</div>`;

/** What the field edits: an inline snippet, or a reference to a saved template id. */
export type TemplateValue =
  | { mode: "inline"; code: string }
  | { mode: "saved"; templateId: string };

interface Props {
  value: TemplateValue;
  onChange: (value: TemplateValue) => void;
}

/** The template source field — a Saved/Inline toggle. Saved reads `template.list` over the bridge;
 *  Inline is a `CodeEditor`. */
export function TemplateSourceField({ value, onChange }: Props) {
  const [templates, setTemplates] = useState<RenderTemplateSummary[]>([]);

  // Load the saved-template roster from the shipped `template.list` verb (NOT REST). Tolerates a
  // deny/empty — a workspace with no saved templates simply offers none.
  useEffect(() => {
    let cancelled = false;
    listTemplates()
      .then((rows) => {
        if (!cancelled) setTemplates(rows);
      })
      .catch(() => {
        if (!cancelled) setTemplates([]);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const mode = value.mode;
  const FIELD =
    "h-8 rounded-md border border-border bg-bg px-2.5 text-xs text-fg focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/20";

  return (
    <div className="mt-2 grid gap-1">
      <div className="flex items-center gap-2" role="tablist" aria-label="template source mode">
        <ModeTab
          active={mode === "inline"}
          label="Inline"
          onClick={() =>
            onChange({
              mode: "inline",
              code: value.mode === "inline" ? value.code : DEFAULT_INLINE_CODE,
            })
          }
        />
        <ModeTab
          active={mode === "saved"}
          label="Saved"
          onClick={() =>
            onChange({
              mode: "saved",
              templateId: value.mode === "saved" ? value.templateId : (templates[0]?.id ?? ""),
            })
          }
        />
      </div>

      {mode === "inline" ? (
        <CodeEditor
          value={value.code}
          onChange={(code) => onChange({ mode: "inline", code })}
          ariaLabel="template code"
          height="160px"
        />
      ) : (
        // eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive; matches the builder's FIELD
        <select
          aria-label="saved template"
          className={FIELD}
          value={value.templateId}
          onChange={(e) => onChange({ mode: "saved", templateId: e.target.value })}
        >
          <option value="">— pick a saved template —</option>
          {templates.map((t) => (
            <option key={t.id} value={t.id}>
              {t.title}
            </option>
          ))}
        </select>
      )}
      <p className="text-[10px] text-muted">
        Binds the source rows with <span className="font-mono">{"{{rows.length}}"}</span> /{" "}
        <span className="font-mono">{"{{latest.field}}"}</span> /{" "}
        <span className="font-mono">{"{{#each rows}}…{{/each}}"}</span> (inside{" "}
        <span className="font-mono">each</span>, <span className="font-mono">{"{{field}}"}</span> reads
        that row); wire writes with <span className="font-mono">data-call</span>. The default body is a
        latest-readings table over <span className="font-mono">point_reading</span> — any query returning{" "}
        <span className="font-mono">point_id / time / value</span> rows (e.g. the demo-buildings
        datasource) fills it. Inline ≤ 4&nbsp;KB; larger snippets save as a render template (≤ 64&nbsp;KB).
      </p>
    </div>
  );
}

/** A small tab button for the Inline/Saved toggle, token-bound + keyboard-operable. */
function ModeTab({
  active,
  label,
  onClick,
}: {
  active: boolean;
  label: string;
  onClick: () => void;
}) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      role="tab"
      aria-selected={active}
      onClick={onClick}
      className={`h-6 px-2 ${active ? "bg-accent/15 text-fg" : "text-muted"}`}
    >
      {label}
    </Button>
  );
}
