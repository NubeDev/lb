// The Create-series wizard (data-console scope) — a two-step modal that ends the empty-state dead-end
// ("no series yet" with no way to write the first sample). Step 1: name + describe the series. Step 2:
// build its typed schema (the recursive `SchemaBuilder`). On finish, the schema is persisted (a real
// record via the ingest path) and the caller drops into the new series with a generated typed form.
//
// Modal idiom matches the shared `ConfirmDestructive` (backdrop + bordered panel, amber accent, small
// text scale). A quiet entrance; honors reduced-motion.

import { useState } from "react";
import { Database, X } from "lucide-react";

import { SchemaBuilder } from "./SchemaBuilder";
import { type Field, type SeriesSchema, newField } from "@/lib/ingest/schema.types";

interface Props {
  /** Series names that already exist — to block a duplicate name. */
  existing: string[];
  /** A name pre-seeded by the caller (e.g. the rail's inline "New series…" field). Step 1 still shows
   *  so the author can confirm the name + add a description; the dup check runs against `existing`. */
  initialName?: string;
  onCancel: () => void;
  /** Persist the schema + create the series (the caller writes it + selects it). */
  onCreate: (schema: SeriesSchema) => Promise<void>;
}

export function CreateSeriesWizard({ existing, initialName, onCancel, onCreate }: Props) {
  const [step, setStep] = useState<1 | 2>(1);
  const [series, setSeries] = useState(initialName ?? "");
  const [description, setDescription] = useState("");
  const [fields, setFields] = useState<Field[]>([newField("number")]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const name = series.trim();
  const dup = existing.includes(name);
  const nameValid = name.length > 0 && !dup;
  // A usable schema needs at least one named field.
  const namedFields = fields.filter((f) => f.name.trim());
  const schemaValid = namedFields.length > 0;

  const finish = async () => {
    setBusy(true);
    setError(null);
    try {
      await onCreate({ series: name, description: description.trim() || undefined, fields: namedFields });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setBusy(false);
    }
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Create series"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4 wizard-backdrop"
      onMouseDown={(e) => e.target === e.currentTarget && onCancel()}
    >
      <div className="wizard-panel flex max-h-[85vh] w-[34rem] max-w-full flex-col overflow-hidden rounded-xl border border-border bg-panel shadow-2xl">
        {/* Header + stepper */}
        <header className="flex items-center gap-2.5 border-b border-border px-5 py-3.5">
          <Database size={16} className="text-accent" />
          <h2 className="text-sm font-medium">New series</h2>
          <Stepper step={step} />
          <button
            type="button"
            aria-label="close"
            onClick={onCancel}
            className="rounded-md p-1 text-muted transition-colors hover:text-fg focus-visible:text-fg focus-visible:outline-none"
          >
            <X size={16} />
          </button>
        </header>

        {/* Body */}
        <div className="min-h-0 flex-1 overflow-auto px-5 py-4">
          {step === 1 ? (
            <div className="flex flex-col gap-4">
              <label className="flex flex-col gap-1.5">
                <span className="text-xs font-medium text-muted">Series name</span>
                <input
                  autoFocus
                  aria-label="series name"
                  placeholder="e.g. node.cpu_temp"
                  value={series}
                  onChange={(e) => setSeries(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && nameValid && setStep(2)}
                  className="rounded-md border border-border bg-bg px-2.5 py-1.5 text-sm placeholder:text-muted/50 focus-visible:border-accent focus-visible:outline-none"
                />
                {dup && (
                  <span className="text-xs text-red-400">a series named “{name}” already exists</span>
                )}
                <span className="text-[11px] text-muted/70">
                  A dotted path is conventional (<code className="text-muted">host.metric</code>), but any
                  name works.
                </span>
              </label>

              <label className="flex flex-col gap-1.5">
                <span className="text-xs font-medium text-muted">
                  Description <span className="text-muted/50">(optional)</span>
                </span>
                <textarea
                  aria-label="description"
                  placeholder="What does this series record?"
                  rows={2}
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  className="resize-none rounded-md border border-border bg-bg px-2.5 py-1.5 text-sm placeholder:text-muted/50 focus-visible:border-accent focus-visible:outline-none"
                />
              </label>
            </div>
          ) : (
            <div className="flex flex-col gap-3">
              <div>
                <div className="text-xs font-medium text-fg">Schema</div>
                <p className="mt-0.5 text-[11px] text-muted/80">
                  Define the shape of each sample. Pick <span className="text-accent">Object</span> or{" "}
                  <span className="text-accent">List</span> to nest fields.
                </p>
              </div>
              <SchemaBuilder fields={fields} onChange={setFields} />
            </div>
          )}
        </div>

        {/* Error + footer */}
        {error && (
          <div role="alert" className="border-t border-border bg-red-500/10 px-5 py-2 text-xs text-red-400">
            {error}
          </div>
        )}
        <footer className="flex items-center gap-2 border-t border-border px-5 py-3">
          <div className="text-[11px] text-muted/70">
            {step === 2 && name && (
              <>
                writing <span className="font-mono text-muted">{name}</span>
              </>
            )}
          </div>
          <div className="ml-auto flex gap-2">
            {step === 2 && (
              <button
                type="button"
                onClick={() => setStep(1)}
                className="rounded-md px-3 py-1.5 text-xs text-muted transition-colors hover:text-fg focus-visible:outline-none"
              >
                Back
              </button>
            )}
            <button
              type="button"
              onClick={onCancel}
              className="rounded-md bg-bg px-3 py-1.5 text-xs text-muted transition-colors hover:text-fg focus-visible:outline-none"
            >
              Cancel
            </button>
            {step === 1 ? (
              <button
                type="button"
                disabled={!nameValid}
                onClick={() => setStep(2)}
                className="rounded-md bg-accent px-3.5 py-1.5 text-xs font-medium text-bg transition-opacity hover:opacity-90 disabled:opacity-40"
              >
                Next: schema
              </button>
            ) : (
              <button
                type="button"
                disabled={!schemaValid || busy}
                onClick={finish}
                className="rounded-md bg-accent px-3.5 py-1.5 text-xs font-medium text-bg transition-opacity hover:opacity-90 disabled:opacity-40"
              >
                {busy ? "Creating…" : "Create series"}
              </button>
            )}
          </div>
        </footer>
      </div>
    </div>
  );
}

/** A two-dot stepper, amber for the active/done step. */
function Stepper({ step }: { step: 1 | 2 }) {
  return (
    <div className="ml-2 flex items-center gap-1.5" aria-hidden>
      <Dot active={step >= 1} />
      <span className={`h-px w-4 ${step >= 2 ? "bg-accent/60" : "bg-border"}`} />
      <Dot active={step >= 2} />
    </div>
  );
}

function Dot({ active }: { active: boolean }) {
  return (
    <span
      className={`h-1.5 w-1.5 rounded-full transition-colors ${active ? "bg-accent" : "bg-border"}`}
    />
  );
}
