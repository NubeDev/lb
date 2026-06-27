// The JSX `template` source field (widget-builder Slice B) — ported from rubix-cube's
// `TemplateSourceField.tsx`, with its SWR/`next`/REST data layer swapped for the shipped
// `template.*` MCP verbs over the bridge. The author either writes inline JSX (saved into
// `cell.options.code`, ≤ INLINE_MAX_BYTES) OR picks a saved `render_template:{id}` row (saved into
// `cell.options.templateId`, ≤ TEMPLATE_MAX_BYTES). The saved-template list reads `template.list`
// (the shipped verb) — never REST.
//
// The lazybones `template` engine is the iframe runtime's eval-free JSX/HTML interpreter: `{{path}}`
// interpolation over a `data` object + `[data-call]` write buttons. The default inline snippet matches
// it. Code runs ONLY in the sandboxed iframe (the v2 trust contract is unchanged).

import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import { listTemplates, type RenderTemplateSummary } from "@/lib/dashboard/template.api";
import { CodeEditor } from "./CodeEditor";

/** A working default inline JSX/HTML template — interpolates over `data` (the rows a bridge.call
 *  produced), the convention the shipped iframe `template` engine interprets. */
export const DEFAULT_INLINE_CODE = `<div class="p-2 text-xs">
  <div class="text-muted">{{rows.length}} rows</div>
  <button data-call="store.query" data-args='{"sql":"SELECT seq FROM series LIMIT 1"}'>Refresh</button>
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
        Receives the source rows as <span className="font-mono">data</span>. Inline ≤ 4&nbsp;KB; larger
        snippets save as a render template (≤ 64&nbsp;KB).
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
