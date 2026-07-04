// The Ingest page — the series explorer (data-console scope). A series list/search on the left (with a
// "New series" action); a detail pane showing the selected series' schema, latest value, recent
// samples, and a typed write form GENERATED from the schema. The empty workspace shows a real
// create-series CTA instead of dead-ending on "no series yet". Layout + wiring; data lives in
// `useIngest`, the schema editor in the wizard.

import { useMemo, useState } from "react";
import { Activity, Database, Plus } from "lucide-react";

import { useIngest } from "./useIngest";
import { CreateSeriesWizard } from "./CreateSeriesWizard";
import { SchemaFields } from "./SchemaForm";
import { emptyPayload, type SeriesSchema } from "@/lib/ingest/schema.types";
import type { Sample } from "@/lib/ingest/ingest.types";

interface Props {
  ws: string;
}

/** Render a sample payload by its JSON type — a scalar verbatim, a structure as compact JSON. */
function renderPayload(p: unknown): string {
  if (p === null || p === undefined) return "—";
  if (typeof p === "object") return JSON.stringify(p);
  return String(p);
}

export function IngestView({ ws }: Props) {
  const { series, selected, schema, latest, recent, error, search, select, write, create } =
    useIngest();
  const [query, setQuery] = useState("");
  const [wizardOpen, setWizardOpen] = useState(false);

  const empty = series.length === 0 && query.trim() === "";

  return (
    <section className="flex h-full flex-col bg-bg">
      <header className="page-header">
        <div className="page-header-icon">
          <Activity size={16} />
        </div>
        <div className="min-w-0">
          <h1 className="page-title">Ingest</h1>
          <p className="page-subtitle">Explore series, inspect samples, and write typed payloads.</p>
        </div>
        <span className="scope-pill ml-auto" title={`Workspace ${ws}`}>
          <span className="h-1.5 w-1.5 rounded-full bg-accent" aria-hidden />
          <span className="truncate">{ws}</span>
        </span>
      </header>

      {error && (
        <div role="alert" className="state-alert">
          {error}
        </div>
      )}

      <div className="flex min-h-0 flex-1">
        {/* Series list / search */}
        <aside className="flex w-64 flex-col border-r border-border">
          <div className="flex items-center gap-2 border-b border-border p-2">
            <input
              aria-label="search series"
              placeholder="prefix, or kind:temperature"
              className="min-w-0 flex-1 rounded-md border border-border bg-panel px-2 py-1 text-sm placeholder:text-muted/60 focus-visible:border-accent focus-visible:outline-none"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && void search(query)}
            />
            <button
              type="button"
              aria-label="new series"
              title="New series"
              onClick={() => setWizardOpen(true)}
              className="shrink-0 rounded-md bg-accent/15 p-1.5 text-accent transition-colors hover:bg-accent/25 focus-visible:outline-none"
            >
              <Plus size={15} />
            </button>
          </div>
          <ul className="flex-1 overflow-auto">
            {series.map((s) => (
              <li key={s}>
                <button
                  aria-label={`select ${s}`}
                  className={`w-full px-3 py-1.5 text-left text-sm transition-colors ${
                    selected === s ? "bg-accent/15 text-accent" : "hover:bg-panel"
                  }`}
                  onClick={() => void select(s)}
                >
                  {s}
                </button>
              </li>
            ))}
            {series.length === 0 && query.trim() !== "" && (
              <li className="px-3 py-2 text-xs text-muted">no series match “{query.trim()}”</li>
            )}
          </ul>
        </aside>

        {/* Detail */}
        <div className="flex min-w-0 flex-1 flex-col overflow-auto">
          {selected ? (
            <SeriesDetail
              series={selected}
              schema={schema}
              latest={latest}
              recent={recent}
              onWrite={write}
            />
          ) : empty ? (
            <EmptyState onCreate={() => setWizardOpen(true)} />
          ) : (
            <div className="p-4 text-sm text-muted">Select a series to explore, or create one.</div>
          )}
        </div>
      </div>

      {wizardOpen && (
        <CreateSeriesWizard
          existing={series}
          onCancel={() => setWizardOpen(false)}
          onCreate={async (s) => {
            await create(s);
            setWizardOpen(false);
          }}
        />
      )}
    </section>
  );
}

/** The first-run empty state — a real call to action instead of a dead-end "no series yet". */
function EmptyState({ onCreate }: { onCreate: () => void }) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-3 p-8 text-center">
      <div className="rounded-full border border-border bg-panel p-3">
        <Database size={22} className="text-accent" />
      </div>
      <div>
        <div className="text-sm font-medium">No series yet</div>
        <p className="mx-auto mt-1 max-w-xs text-xs text-muted">
          A series is a named, typed sequence of samples. Create one — name it, define its schema, and
          write the first sample.
        </p>
      </div>
      <button
        type="button"
        onClick={onCreate}
        className="inline-flex items-center gap-1.5 rounded-md bg-accent px-3.5 py-1.5 text-xs font-medium text-bg transition-opacity hover:opacity-90 focus-visible:outline-none"
      >
        <Plus size={14} /> Create series
      </button>
    </div>
  );
}

/** The selected series' detail: schema chips, latest, recent samples, and the typed write form. */
function SeriesDetail({
  series,
  schema,
  latest,
  recent,
  onWrite,
}: {
  series: string;
  schema: SeriesSchema | null;
  latest: Sample | null;
  recent: Sample[];
  onWrite: (s: Sample) => Promise<void>;
}) {
  return (
    <div className="flex flex-col gap-4 p-4">
      <div>
        <div className="flex items-baseline gap-2">
          <h2 className="font-mono text-sm text-fg">{series}</h2>
          {schema?.fields?.length ? (
            <span className="text-[11px] text-muted/70">
              {schema.fields.length} field{schema.fields.length === 1 ? "" : "s"}
            </span>
          ) : (
            <span className="text-[11px] text-muted/60">no schema</span>
          )}
        </div>
        {schema?.description && (
          <p className="mt-0.5 text-xs text-muted">{schema.description}</p>
        )}
      </div>

      <div>
        <div className="text-xs text-muted">latest</div>
        <div className="text-2xl font-semibold" aria-label="latest value">
          {latest ? renderPayload(latest.payload) : "—"}
        </div>
        {latest && (
          <div className="text-xs text-muted">
            seq {latest.seq} · {latest.producer}
          </div>
        )}
      </div>

      <div>
        <div className="mb-1 text-xs text-muted">recent samples</div>
        <table className="w-full text-left text-sm">
          <thead>
            <tr className="text-xs text-muted">
              <th className="py-1 pr-4 font-medium">seq</th>
              <th className="py-1 pr-4 font-medium">payload</th>
              <th className="py-1 font-medium">ts</th>
            </tr>
          </thead>
          <tbody>
            {recent.map((s) => (
              <tr key={s.seq} className="border-t border-border/50">
                <td className="py-1 pr-4 tabular-nums">{s.seq}</td>
                <td className="py-1 pr-4">{renderPayload(s.payload)}</td>
                <td className="py-1 tabular-nums text-muted">{s.ts}</td>
              </tr>
            ))}
            {recent.length === 0 && (
              <tr>
                <td colSpan={3} className="py-2 text-xs text-muted">
                  no samples yet
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <WriteForm series={series} schema={schema} nextSeq={(latest?.seq ?? 0) + 1} onWrite={onWrite} />
    </div>
  );
}

/** The write form. With a schema, renders a generated typed form (`SchemaFields`); without one, a
 *  single freeform payload input. Either way produces one real `ingest.write`. */
function WriteForm({
  series,
  schema,
  nextSeq,
  onWrite,
}: {
  series: string;
  schema: SeriesSchema | null;
  nextSeq: number;
  onWrite: (s: Sample) => Promise<void>;
}) {
  const [seq, setSeq] = useState(String(nextSeq));
  const [label, setLabel] = useState("");
  const initial = useMemo(() => (schema ? emptyPayload(schema) : {}), [schema]);
  const [value, setValue] = useState<Record<string, unknown>>(initial);
  const [raw, setRaw] = useState("");

  // Reset the typed value when the schema (i.e. the series) changes.
  const [schemaKey, setSchemaKey] = useState(series);
  if (schemaKey !== series) {
    setSchemaKey(series);
    setValue(initial);
    setSeq(String(nextSeq));
    setRaw("");
  }

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    let payload: unknown;
    if (schema) {
      payload = value;
    } else {
      try {
        payload = JSON.parse(raw);
      } catch {
        payload = raw;
      }
    }
    const labels: Record<string, unknown> = {};
    if (label.includes(":")) {
      const [k, ...rest] = label.split(":");
      labels[k] = rest.join(":");
    }
    const n = Number(seq) || 0;
    void onWrite({ series, producer: "", ts: n, seq: n, payload, labels });
    setSeq(String(n + 1));
  };

  return (
    <form aria-label="write sample" className="rounded-lg border border-border bg-panel p-3" onSubmit={submit}>
      <div className="mb-2.5 flex items-center gap-2">
        <Plus size={13} className="text-accent" />
        <span className="text-xs font-medium">Write sample</span>
      </div>

      {schema ? (
        <SchemaFields fields={schema.fields} value={value} onChange={setValue} />
      ) : (
        <label className="flex flex-col gap-1">
          <span className="text-xs font-medium text-muted">payload</span>
          <input
            aria-label="payload"
            placeholder="61.4, or a JSON value"
            className="rounded-md border border-border bg-bg px-2 py-1 text-sm placeholder:text-muted/50 focus-visible:border-accent focus-visible:outline-none"
            value={raw}
            onChange={(e) => setRaw(e.target.value)}
          />
        </label>
      )}

      <div className="mt-3 flex flex-wrap items-end gap-2 border-t border-border/60 pt-3">
        <label className="flex flex-col gap-1">
          <span className="text-[11px] text-muted">seq</span>
          <input
            aria-label="seq"
            className="w-20 rounded-md border border-border bg-bg px-2 py-1 text-sm tabular-nums focus-visible:border-accent focus-visible:outline-none"
            value={seq}
            onChange={(e) => setSeq(e.target.value)}
          />
        </label>
        <label className="flex flex-1 flex-col gap-1">
          <span className="text-[11px] text-muted">label (optional)</span>
          <input
            aria-label="label"
            placeholder="host:pi-7"
            className="min-w-0 rounded-md border border-border bg-bg px-2 py-1 text-sm placeholder:text-muted/50 focus-visible:border-accent focus-visible:outline-none"
            value={label}
            onChange={(e) => setLabel(e.target.value)}
          />
        </label>
        <button
          type="submit"
          aria-label="submit sample"
          className="rounded-md bg-accent px-3.5 py-1.5 text-xs font-medium text-bg transition-opacity hover:opacity-90 focus-visible:outline-none"
        >
          Push sample
        </button>
      </div>
    </form>
  );
}
