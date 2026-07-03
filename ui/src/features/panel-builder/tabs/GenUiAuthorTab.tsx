// The "AI widget" options tab (genui-scope "Authoring path"). The author types a natural-language
// request; we run it through `agent.invoke` under `skill:core.genui-widget` (caller's principal), stream
// a live preview, and on ACCEPT run parse→normalize→validate→size-check ONCE (loudly) and write the
// typed IR into `options.genui` — the existing PanelEditor Save then persists the cell via
// `dashboard.save`. The live preview renders through the SAME `GenUiView` the dashboard uses (the left
// PreviewPane picks up `state.options.genui.ir` automatically once we patch it).
//
// One responsibility: drive the authoring turn + accept. The render/data path is `GenUiView`'s job.

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import type { IrSpec } from "@nube/genui";
import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import { useGenUiAuthor } from "@/lib/panel-kit/useGenUiAuthor";

interface Props {
  state: EditorState;
  patch: (next: Partial<EditorState>) => void;
  ws: string;
  /** The caller's principal + grant (threaded from the session, mirroring `useAgent`). */
  author?: string;
  caps?: string[];
}

/** Write an accepted IR into `options.genui` so the preview + Save pick it up. `v` mirrors the IR's. */
function patchIr(state: EditorState, patch: Props["patch"], ir: IrSpec) {
  patch({ options: { ...state.options, genui: { v: ir.v, ir } } });
}

export function GenUiAuthorTab({ state, patch, ws, author, caps }: Props) {
  const [prompt, setPrompt] = useState("");
  const [warnings, setWarnings] = useState<string[]>([]);
  const [accepted, setAccepted] = useState(false);
  const { state: authorState, run, accept } = useGenUiAuthor(ws);

  const onGenerate = async () => {
    setAccepted(false);
    setWarnings([]);
    const out = await run(prompt, author, caps);
    // Show the streamed/parsed preview immediately (not yet accepted — normalize/validate run on accept).
    if (out) patchIr(state, patch, out.ir);
  };

  const onAccept = () => {
    const res = accept();
    if (!res.ok) {
      setWarnings([res.error ?? "widget spec was rejected"]);
      setAccepted(false);
      return;
    }
    // Persist the NORMALIZED, validated IR (never the raw preview) — the same spec the host will accept.
    patchIr(state, patch, res.ir!);
    setWarnings(res.findings.filter((f) => f.level === "warning").map((f) => f.message));
    setAccepted(true);
  };

  return (
    <div className="grid gap-3 py-2" aria-label="ai widget author">
      <label className="grid gap-1 text-xs">
        <span className="text-muted">Describe the widget</span>
        <Textarea
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          rows={3}
          placeholder="e.g. a counter from the demo flow next to a 24h chart of series office/temp, red when the counter stalls"
          aria-label="widget prompt"
        />
      </label>
      <div className="flex items-center gap-2">
        <Button size="sm" onClick={onGenerate} disabled={authorState.running || !prompt.trim()} aria-label="generate widget">
          {authorState.running ? "Generating…" : "Generate"}
        </Button>
        <Button
          size="sm"
          variant="outline"
          onClick={onAccept}
          disabled={!authorState.preview || authorState.running}
          aria-label="accept widget"
        >
          Accept
        </Button>
        {accepted && <span className="text-xs text-muted">accepted — Save to persist</span>}
      </div>
      {authorState.error && (
        <div className="text-xs text-destructive" role="alert">
          {authorState.error}
        </div>
      )}
      {warnings.length > 0 && (
        <ul className="grid gap-0.5 text-xs text-muted" aria-label="accept warnings">
          {warnings.map((w, i) => (
            <li key={i}>⚠ {w}</li>
          ))}
        </ul>
      )}
    </div>
  );
}
