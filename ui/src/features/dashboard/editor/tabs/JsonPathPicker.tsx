// The visual JSON-path builder for a flow OUTPUT binding (flow-dashboard-binding-ux-scope: "parse out
// the JSON"). It reads the bound node's CURRENT value (`flows.node_state`, the same read the canvas
// uses) and renders it as an interactive, collapsible TREE — objects, arrays, nested, scalars. Clicking
// a row binds THAT path (e.g. `payload`, `payload.cron_ts`, `items[0].name`); the chosen path is stored
// on the source args (`__flowPath`) and the read view shows exactly that value. No hand-typed JSON
// pointers, agnostic to the shape a developer's node emits.

import { useEffect, useMemo, useState } from "react";
import { ChevronRight, ChevronDown, Check } from "lucide-react";

import { getFlowNodeState } from "@/lib/flows/flows.api";
import {
  asPath,
  childrenOf,
  kindOf,
  pathLabel,
  previewOf,
  valueAtPath,
  type JsonKind,
  type PathSeg,
} from "../../views/jsonPaths";

interface Props {
  flowId: string;
  node: string;
  /** The currently-bound path (array of segments) from the source args. */
  path: PathSeg[];
  /** Commit a new path selection (the editor writes it onto the source args). */
  onPick: (path: PathSeg[]) => void;
  /** Re-read tick — bumps to refetch the live value (the canvas-cadence refresh). */
  refreshKey?: number;
}

/** A muted tag for a value's kind. */
function KindTag({ kind }: { kind: JsonKind }) {
  return <span className="ml-1 text-[10px] uppercase tracking-wide text-muted/70">{kind}</span>;
}

export function JsonPathPicker({ flowId, node, path, onPick, refreshKey = 0 }: Props) {
  const [root, setRoot] = useState<unknown>(undefined);
  const [loading, setLoading] = useState(true);
  const [denied, setDenied] = useState(false);
  // Which container paths are expanded (string-encoded for a Set). The bound path's ancestors auto-open.
  const [open, setOpen] = useState<Set<string>>(() => new Set(ancestorKeys(path)));

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    getFlowNodeState(flowId)
      .then((st) => {
        if (cancelled) return;
        const entry = st.nodes.find((n) => n.node === node);
        setRoot(entry?.value ?? null);
        setDenied(false);
        setLoading(false);
      })
      .catch(() => {
        if (cancelled) return;
        setDenied(true);
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [flowId, node, refreshKey]);

  const selectedValue = useMemo(() => valueAtPath(root, path), [root, path]);

  const toggle = (key: string) =>
    setOpen((s) => {
      const next = new Set(s);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });

  // Picking a row binds its path AND expands it (if a container) so its children are immediately
  // visible to drill deeper — the natural "click into the JSON" gesture.
  const handlePick = (p: PathSeg[]) => {
    const v = valueAtPath(root, p);
    if (v && typeof v === "object") setOpen((s) => new Set(s).add(p.join("")));
    onPick(p);
  };

  if (loading) return <p className="text-xs text-muted">reading current value…</p>;
  if (denied) return <p className="text-xs text-danger">no access to this node’s value</p>;
  if (root == null)
    return <p className="text-xs text-muted">this node has no value yet — run the flow once, then pick a field</p>;

  return (
    <div className="grid gap-2" aria-label="json path picker">
      <div className="flex items-center justify-between text-xs text-muted">
        <span>Bind a field</span>
        <span className="font-mono text-fg" aria-label="selected path">
          {pathLabel(path)}
        </span>
      </div>
      <div className="max-h-56 overflow-auto rounded-md border border-border bg-bg p-1">
        <TreeNode
          label="value"
          value={root}
          path={[]}
          depth={0}
          open={open}
          toggle={toggle}
          selectedPath={path}
          onPick={handlePick}
        />
      </div>
      <div className="rounded-md border border-border bg-card px-2 py-1.5">
        <span className="text-[10px] uppercase tracking-wide text-muted">preview</span>
        <pre aria-label="path preview" className="mt-0.5 max-h-24 overflow-auto whitespace-pre-wrap break-words font-mono text-xs text-fg">
          {selectedValue === undefined ? "(field not present)" : pretty(selectedValue)}
        </pre>
      </div>
    </div>
  );
}

interface TreeNodeProps {
  label: string;
  value: unknown;
  path: PathSeg[];
  depth: number;
  open: Set<string>;
  toggle: (key: string) => void;
  selectedPath: PathSeg[];
  onPick: (path: PathSeg[]) => void;
}

/** One row of the value tree — a clickable label that binds its path, plus an expander for containers. */
function TreeNode({ label, value, path, depth, open, toggle, selectedPath, onPick }: TreeNodeProps) {
  const kind = kindOf(value);
  const isContainer = kind === "object" || kind === "array";
  const key = path.join("");
  const expanded = open.has(key) || depth === 0;
  const selected = pathLabel(selectedPath) === pathLabel(path);
  const kids = isContainer ? childrenOf(value) : [];

  return (
    <div>
      <div
        className={`flex items-center gap-1 rounded px-1 py-0.5 ${selected ? "bg-accent/15" : "hover:bg-card"}`}
        style={{ paddingLeft: `${depth * 12 + 4}px` }}
      >
        {isContainer ? (
          // eslint-disable-next-line no-restricted-syntax -- a tiny tree expander, not a Button
          <button
            type="button"
            aria-label={expanded ? `collapse ${label}` : `expand ${label}`}
            onClick={() => toggle(key)}
            className="text-muted hover:text-fg"
          >
            {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
          </button>
        ) : (
          <span className="w-3" />
        )}
        {/* eslint-disable-next-line no-restricted-syntax -- a tree row select, not a Button */}
        <button
          type="button"
          aria-label={`bind ${pathLabel(path)}`}
          onClick={() => onPick(path)}
          className="flex min-w-0 flex-1 items-center gap-1 text-left"
        >
          <span className="truncate font-mono text-xs text-fg">{label}</span>
          <KindTag kind={kind} />
          {!isContainer && <span className="ml-1 truncate text-xs text-muted">{previewOf(value)}</span>}
          {isContainer && <span className="ml-1 text-xs text-muted/60">{previewOf(value)}</span>}
          {selected && <Check size={12} className="ml-auto shrink-0 text-accent" />}
        </button>
      </div>
      {isContainer && expanded && (
        <div>
          {kids.map((c) => (
            <TreeNode
              key={String(c.seg)}
              label={c.label}
              value={c.value}
              path={[...path, c.seg]}
              depth={depth + 1}
              open={open}
              toggle={toggle}
              selectedPath={selectedPath}
              onPick={onPick}
            />
          ))}
          {kids.length === 0 && (
            <p className="text-xs text-muted/60" style={{ paddingLeft: `${(depth + 1) * 12 + 16}px` }}>
              (empty)
            </p>
          )}
        </div>
      )}
    </div>
  );
}

/** The ancestor expand-keys for a path (so opening on a saved deep path shows it). */
function ancestorKeys(path: PathSeg[]): string[] {
  const keys: string[] = [];
  for (let i = 0; i < path.length; i++) keys.push(path.slice(0, i).join(""));
  return keys;
}

function pretty(v: unknown): string {
  try {
    return JSON.stringify(v, null, 2);
  } catch {
    return String(v);
  }
}

export { asPath };
