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

/** A working default inline template — a per-site building summary. Bind a `federation.query` source
 *  over the seeded building data (`docker/postgres/seed.py`: sites / meters / points / readings), e.g.
 *  `SELECT s.name AS site, AVG(pr.value) AS avg_val FROM point_reading pr
 *   JOIN point p ON p.id = pr.point_id JOIN meter m ON m.id = p.meter_id
 *   JOIN site s ON s.id = m.site_id GROUP BY s.name ORDER BY avg_val DESC LIMIT 12` — the template
 *  renders one row per site the query returns. `{{#each rows}}` iterates; `{{site}}`/`{{avg_val}}` read
 *  the row fields; the `[data-call]` button is a host-mediated refresh (add the tool to the cell's tools). */
export const DEFAULT_INLINE_CODE = `<div class="p-3 text-xs">
  <div class="mb-2 text-muted">{{rows.length}} sites</div>
  <ul class="space-y-1">
    {{#each rows}}<li>
      <span class="font-medium">{{site}}</span>
      <span class="ml-2 tabular-nums text-muted">{{avg_val}}</span>
    </li>{{/each}}
  </ul>
  <button class="mt-2 rounded border border-border px-2 py-1" data-call="federation.query" data-args='{}'>Refresh</button>
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
        <span className="font-mono">{"{{#each rows}}…{{/each}}"}</span>; wire writes with{" "}
        <span className="font-mono">data-call</span>. The default body is a per-site building summary —
        bind a <span className="font-mono">federation.query</span> source over the seeded building data
        (<span className="font-mono">docker/postgres/seed.py</span>: site / meter / point / point_reading)
        to see it. Inline ≤ 4&nbsp;KB; larger snippets save as a render template (≤ 64&nbsp;KB).
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
