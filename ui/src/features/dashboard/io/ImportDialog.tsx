// Import a dashboard bundle (dashboard scope, import/export UX). The one surface for "bring a dashboard
// or widget in": paste JSON OR pick a `.lbdash.json` file → a live preview of what the bundle carries
// (counts, per-entry titles, any parse warnings) → a collision choice (rename-safe by default) →
// Confirm, which replays the bundle through the shipped save verbs under the caller's authority. The
// workspace/owner are never taken from the file (rule 6) — this dialog only orchestrates the parse +
// the confirmed replay; the host is the real boundary. One responsibility: the import interaction.

import { useCallback, useRef, useState } from "react";
import {
  AlertTriangle,
  FileUp,
  LayoutDashboard,
  Puzzle,
  Upload,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Textarea } from "@/components/ui/textarea";
import {
  describeBundle,
  parseBundle,
  type DashboardBundle,
} from "@/lib/dashboard";
import type { CollisionMode, ImportOutcome } from "./useDashboardIo";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  busy?: boolean;
  /** Replay the parsed bundle. Returns the outcome (created/renamed/errors) the dialog then summarizes. */
  onImport: (
    bundle: DashboardBundle,
    mode: CollisionMode,
  ) => Promise<ImportOutcome>;
}

export function ImportDialog({
  open,
  onOpenChange,
  busy = false,
  onImport,
}: Props) {
  const [text, setText] = useState("");
  const [mode, setMode] = useState<CollisionMode>("rename");
  const [outcome, setOutcome] = useState<ImportOutcome | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);

  const parsed = text.trim() ? parseBundle(text) : null;
  const bundle = parsed?.ok ? parsed.bundle : null;

  const reset = useCallback(() => {
    setText("");
    setMode("rename");
    setOutcome(null);
  }, []);

  const pickFile = () => fileRef.current?.click();
  const onFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    e.target.value = ""; // let the same file be re-picked
    if (!file) return;
    setOutcome(null);
    setText(await file.text());
  };

  const confirm = async () => {
    if (!bundle) return;
    setOutcome(await onImport(bundle, mode));
  };

  const close = (next: boolean) => {
    if (!next) reset();
    onOpenChange(next);
  };

  return (
    <Dialog open={open} onOpenChange={close}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <FileUp size={16} /> Import dashboard or widgets
          </DialogTitle>
          <DialogDescription>
            Paste a Lazybones bundle (<code>.lbdash.json</code>) or choose a
            file. Imported records are created in{" "}
            <strong>this workspace</strong> under your account — the file never
            carries a workspace or owner.
          </DialogDescription>
        </DialogHeader>

        {outcome ? (
          <ImportSummary outcome={outcome} />
        ) : (
          <div className="space-y-3">
            <div className="flex items-center gap-2">
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={pickFile}
              >
                <Upload size={13} /> Choose file…
              </Button>
              {/* eslint-disable-next-line no-restricted-syntax -- a hidden native file picker; no shadcn equivalent */}
              <input
                ref={fileRef}
                type="file"
                accept=".json,application/json"
                className="hidden"
                aria-label="dashboard bundle file"
                onChange={onFile}
              />
              <span className="text-xs text-muted">or paste below</span>
            </div>

            <Textarea
              aria-label="bundle json"
              className="h-40 font-mono text-xs"
              placeholder='{ "kind": "lazybones.dashboard-bundle", "version": 1, … }'
              value={text}
              onChange={(e) => setText(e.target.value)}
            />

            {parsed && !parsed.ok && (
              <p
                role="alert"
                className="flex items-start gap-1.5 text-xs text-destructive"
              >
                <AlertTriangle size={13} className="mt-0.5 shrink-0" />{" "}
                {parsed.error}
              </p>
            )}

            {bundle && (
              <div className="rounded-md border border-border bg-panel-2/50 p-3 text-xs">
                <p className="mb-2 font-medium">
                  Ready to import {describeBundle(bundle)}:
                </p>
                <ul className="space-y-1">
                  {bundle.dashboards.map((d) => (
                    <li key={`d-${d.id}`} className="flex items-center gap-1.5">
                      <LayoutDashboard size={12} className="text-muted" />
                      <span className="truncate">{d.title}</span>
                      <span className="text-muted">
                        · {d.cells.length} widgets
                      </span>
                    </li>
                  ))}
                  {bundle.panels.map((p) => (
                    <li key={`p-${p.id}`} className="flex items-center gap-1.5">
                      <Puzzle size={12} className="text-muted" />
                      <span className="truncate">{p.title}</span>
                    </li>
                  ))}
                </ul>
                {parsed?.ok && parsed.warnings.length > 0 && (
                  <ul className="mt-2 space-y-0.5 text-warning">
                    {parsed.warnings.map((w, i) => (
                      <li key={i} className="flex items-start gap-1.5">
                        <AlertTriangle size={11} className="mt-0.5 shrink-0" />{" "}
                        {w}
                      </li>
                    ))}
                  </ul>
                )}

                <div
                  className="mt-3 flex items-center gap-2"
                  role="radiogroup"
                  aria-label="on id collision"
                >
                  <span className="text-muted">On collision:</span>
                  <Button
                    type="button"
                    role="radio"
                    aria-checked={mode === "rename"}
                    variant={mode === "rename" ? "default" : "outline"}
                    size="sm"
                    className="h-7"
                    onClick={() => setMode("rename")}
                  >
                    Keep both
                  </Button>
                  <Button
                    type="button"
                    role="radio"
                    aria-checked={mode === "overwrite"}
                    variant={mode === "overwrite" ? "default" : "outline"}
                    size="sm"
                    className="h-7"
                    title="Only your own records can be overwritten (checked server-side)."
                    onClick={() => setMode("overwrite")}
                  >
                    Overwrite
                  </Button>
                </div>
              </div>
            )}
          </div>
        )}

        <DialogFooter>
          {outcome ? (
            <Button onClick={() => close(false)}>Done</Button>
          ) : (
            <>
              <Button variant="ghost" onClick={() => close(false)}>
                Cancel
              </Button>
              <Button disabled={!bundle || busy} onClick={confirm}>
                {busy ? "Importing…" : "Import"}
              </Button>
            </>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function ImportSummary({ outcome }: { outcome: ImportOutcome }) {
  const created = outcome.dashboards.length + outcome.panels.length;
  return (
    <div className="space-y-2 text-sm">
      <p>
        Imported <strong>{created}</strong> record{created === 1 ? "" : "s"}.
      </p>
      {(outcome.dashboards.length > 0 || outcome.panels.length > 0) && (
        <ul className="max-h-40 space-y-1 overflow-y-auto text-xs">
          {outcome.dashboards.map((d) => (
            <li key={`d-${d.id}`} className="flex items-center gap-1.5">
              <LayoutDashboard size={12} className="text-muted" />
              <span className="truncate">{d.title}</span>
              {d.renamedFrom && (
                <span className="text-muted">(was “{d.renamedFrom}”)</span>
              )}
            </li>
          ))}
          {outcome.panels.map((p) => (
            <li key={`p-${p.id}`} className="flex items-center gap-1.5">
              <Puzzle size={12} className="text-muted" />
              <span className="truncate">{p.title}</span>
              {p.renamedFrom && (
                <span className="text-muted">(was “{p.renamedFrom}”)</span>
              )}
            </li>
          ))}
        </ul>
      )}
      {outcome.errors.length > 0 && (
        <ul className="space-y-0.5 text-xs text-destructive">
          {outcome.errors.map((e, i) => (
            <li key={i} className="flex items-start gap-1.5">
              <AlertTriangle size={12} className="mt-0.5 shrink-0" /> {e}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
