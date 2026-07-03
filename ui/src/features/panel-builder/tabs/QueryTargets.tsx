// The multi-target Query wrapper (editor-parity scope, step 6) — the A/B/C query rows over the
// `targets[]` model (the backend resolver already dispatches every target). It owns the row bar
// (select/add/duplicate/delete/hide/reorder) + the query-options row (max data points / min interval /
// relative time), and renders the SINGLE-target `QueryTab` against a SCOPED view of the editor state:
// `state.targets` is narrowed to `[activeTarget]` and the scoped `patch` maps a `targets` write back
// into the full array at the active index. So `QueryTab` (and the SQL/Flows sections that read
// `targets[0]`) stay single-target and unchanged — this wrapper multiplexes them. One responsibility:
// the target list + query options around the single-target editor.

import { useState } from "react";
import { Copy, Eye, EyeOff, Plus, Trash2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import type { QueryOptions, Target } from "@/lib/dashboard";
import { QueryTab } from "./QueryTab";

interface Props {
  ws: string;
  state: EditorState;
  patch: (next: Partial<EditorState>) => void;
  onRun?: () => void;
}

/** The next free refId (A, B, C, …) not already used. */
function nextRefId(targets: Target[]): string {
  const used = new Set(targets.map((t) => t.refId));
  for (let i = 0; i < 26; i++) {
    const id = String.fromCharCode(65 + i);
    if (!used.has(id)) return id;
  }
  return `Q${targets.length}`;
}

export function QueryTargets({ ws, state, patch, onRun }: Props) {
  // A flows INPUT control (an inject action, no read target) is inherently single-target — skip the
  // multi-target bar for it so the flow port picker isn't wrapped in an A/B/C row it can't use.
  const isFlowAction = state.carry.action?.tool === "flows.inject";
  const targets = state.targets.length ? state.targets : [{ refId: "A", tool: "", args: {}, datasource: { type: "surreal" as const } }];
  const [active, setActive] = useState(0);
  const idx = Math.min(active, targets.length - 1);

  const writeTargets = (next: Target[]) => patch({ targets: next });
  const setActiveSafe = (i: number) => setActive(Math.max(0, Math.min(i, targets.length - 1)));

  const addTarget = () => {
    const next = [...targets, { refId: nextRefId(targets), tool: "", args: {}, datasource: { type: "surreal" as const } }];
    writeTargets(next);
    setActive(next.length - 1);
  };
  const duplicate = (i: number) => {
    const copy = { ...targets[i], refId: nextRefId(targets) };
    const next = [...targets.slice(0, i + 1), copy, ...targets.slice(i + 1)];
    writeTargets(next);
    setActive(i + 1);
  };
  const remove = (i: number) => {
    if (targets.length <= 1) return; // always keep at least one target row
    writeTargets(targets.filter((_, j) => j !== i));
    setActiveSafe(i > 0 ? i - 1 : 0);
  };
  const toggleHide = (i: number) =>
    writeTargets(targets.map((t, j) => (j === i ? { ...t, hide: !t.hide } : t)));
  const move = (i: number, dir: -1 | 1) => {
    const j = i + dir;
    if (j < 0 || j >= targets.length) return;
    const next = [...targets];
    [next[i], next[j]] = [next[j], next[i]];
    writeTargets(next);
    setActive(j);
  };

  // The SCOPED editor state the single-target QueryTab edits: only the active target is visible, and a
  // `targets` write from QueryTab is spliced back into the full array at `idx`.
  const scopedState: EditorState = { ...state, targets: [targets[idx]] };
  const scopedPatch = (next: Partial<EditorState>) => {
    if (next.targets) {
      const merged = [...targets];
      // QueryTab always writes a one-element `[target]`; splice it in at the active index.
      merged[idx] = next.targets[0] ?? merged[idx];
      patch({ ...next, targets: merged });
    } else {
      patch(next);
    }
  };

  const qo: QueryOptions = state.queryOptions ?? {};
  const setQo = (patchQo: Partial<QueryOptions>) => patch({ queryOptions: { ...qo, ...patchQo } });

  return (
    <div className="grid gap-3" aria-label="query targets">
      {!isFlowAction && (
        <div className="flex flex-wrap items-center gap-1" aria-label="target rows">
          {targets.map((t, i) => (
            <div key={t.refId} className={`flex items-center overflow-hidden rounded-md border ${i === idx ? "border-accent" : "border-border"}`}>
              <Button
                type="button"
                size="sm"
                variant="ghost"
                aria-label={`select query ${t.refId}`}
                aria-pressed={i === idx}
                className={`h-6 rounded-none px-2 text-xs ${i === idx ? "bg-accent/10 text-fg" : "text-muted"} ${t.hide ? "line-through" : ""}`}
                onClick={() => setActive(i)}
              >
                {t.refId}
              </Button>
              <Button type="button" size="sm" variant="ghost" aria-label={`${t.hide ? "show" : "hide"} query ${t.refId}`} className="h-6 px-1 text-muted" onClick={() => toggleHide(i)}>
                {t.hide ? <EyeOff size={12} /> : <Eye size={12} />}
              </Button>
              <Button type="button" size="sm" variant="ghost" aria-label={`duplicate query ${t.refId}`} className="h-6 px-1 text-muted" onClick={() => duplicate(i)}>
                <Copy size={12} />
              </Button>
              <Button type="button" size="sm" variant="ghost" aria-label={`move query ${t.refId} left`} disabled={i === 0} className="h-6 px-1 text-muted" onClick={() => move(i, -1)}>
                ‹
              </Button>
              <Button type="button" size="sm" variant="ghost" aria-label={`move query ${t.refId} right`} disabled={i === targets.length - 1} className="h-6 px-1 text-muted" onClick={() => move(i, 1)}>
                ›
              </Button>
              <Button type="button" size="sm" variant="ghost" aria-label={`delete query ${t.refId}`} disabled={targets.length <= 1} className="h-6 px-1 text-muted hover:text-red-500" onClick={() => remove(i)}>
                <Trash2 size={12} />
              </Button>
            </div>
          ))}
          <Button type="button" size="sm" variant="outline" aria-label="add query" className="h-7 px-2 text-xs" onClick={addTarget}>
            <Plus size={12} /> Query
          </Button>
        </div>
      )}

      <QueryTab ws={ws} state={scopedState} patch={scopedPatch} onRun={onRun} />

      {/* Query options row — forwarded with the whole panel to the resolver. */}
      <details className="rounded-md border border-border bg-bg px-2 py-1.5 text-xs" aria-label="query options">
        <summary className="cursor-pointer text-muted">Query options</summary>
        <div className="mt-2 grid grid-cols-3 gap-2">
          <label className="grid gap-1 text-muted">
            Max data points
            <Input aria-label="max data points" type="number" className="h-7 text-xs" placeholder="auto" value={qo.maxDataPoints ?? ""} onChange={(e) => setQo({ maxDataPoints: e.target.value === "" ? undefined : Number(e.target.value) })} />
          </label>
          <label className="grid gap-1 text-muted">
            Min interval
            <Input aria-label="min interval" className="h-7 text-xs" placeholder="e.g. 1m" value={qo.minInterval ?? ""} onChange={(e) => setQo({ minInterval: e.target.value || undefined })} />
          </label>
          <label className="grid gap-1 text-muted">
            Relative time
            <Input aria-label="relative time" className="h-7 text-xs" placeholder="e.g. now-6h" value={qo.relativeTime ?? ""} onChange={(e) => setQo({ relativeTime: e.target.value || undefined })} />
          </label>
        </div>
      </details>
    </div>
  );
}
