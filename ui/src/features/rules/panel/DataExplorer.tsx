// The data explorer — what a rule can query, each click-to-insert (rules-editor-ux scope). Three
// sections over SHIPPED workspace-walled verbs: registered external datasources (`datasource.list` →
// `source("name")`), the local store schema (`store.schema` via the shared SchemaBrowser → a table/
// column name), and the discoverable series (`series.list` → `history("series", name, "24h")`). Each
// section renders an HONEST state: loading, denied (never a fake list), empty (teaches), or ready.
//
// NOTE (honest gap, per scope): there is no per-external-datasource table-introspection verb today —
// `datasource.list` gives kind + endpoint only, and `store.schema` is the LOCAL store. Deep external-
// table browsing is a named follow-up, surfaced here as a hint, not a silent omission.

import { Database, Radio } from "lucide-react";

import { Button } from "@/components/ui/button";
import { SchemaBrowser } from "@/components/schema";
import type { DataExplorerState, SectionState } from "./useDataExplorer";

interface DataExplorerProps {
  state: DataExplorerState;
  /** Insert a snippet at the editor cursor. */
  onInsert: (snippet: string) => void;
}

/** The three click-to-insert data sections. */
export function DataExplorer({ state, onInsert }: DataExplorerProps) {
  return (
    <div aria-label="data explorer" className="flex h-full flex-col gap-3 overflow-auto p-2">
      <Section label="Datasources" hint="Registered external sources — click to query by name.">
        {render(state.datasources, (rows) =>
          rows.length === 0 ? (
            <Empty>No external datasources registered.</Empty>
          ) : (
            <ul className="grid gap-0.5">
              {rows.map((d) => (
                <li key={d.name}>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    aria-label={`insert datasource ${d.name}`}
                    onClick={() => onInsert(`source(${JSON.stringify(d.name)})`)}
                    className="h-auto w-full flex-col items-start gap-0 px-1.5 py-1 text-left hover:bg-muted"
                  >
                    <span className="flex items-center gap-1.5 font-mono text-xs text-fg">
                      <Database size={12} className="text-muted" />
                      {d.name}
                    </span>
                    <span className="pl-[18px] text-[10px] text-muted">
                      {d.kind} · {d.endpoint}
                    </span>
                  </Button>
                </li>
              ))}
            </ul>
          ),
        )}
      </Section>

      <Section label="Local tables" hint="Tables in this workspace's store — click to insert a name.">
        {render(state.schema, (schema) =>
          schema.tables.length === 0 ? (
            <Empty>No local tables yet.</Empty>
          ) : (
            <SchemaBrowser
              schema={schema}
              onPick={(table, column) => onInsert(column ?? table)}
            />
          ),
        )}
      </Section>

      <Section label="Series" hint="Discoverable timeseries — click to read 24h of history.">
        {render(state.series, (names) =>
          names.length === 0 ? (
            <Empty>No series in this workspace.</Empty>
          ) : (
            <ul className="grid gap-0.5">
              {names.map((s) => (
                <li key={s}>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    aria-label={`insert series ${s}`}
                    onClick={() => onInsert(`history("series", ${JSON.stringify(s)}, "24h")`)}
                    className="h-6 w-full justify-start gap-1.5 px-1.5 font-mono text-xs text-fg hover:bg-muted"
                  >
                    <Radio size={12} className="text-muted" />
                    {s}
                  </Button>
                </li>
              ))}
            </ul>
          ),
        )}
      </Section>
    </div>
  );
}

/** Render a section's honest state: loading skeleton, deny, or the ready body. */
function render<T>(
  section: SectionState<T>,
  ready: (data: T) => React.ReactNode,
): React.ReactNode {
  if (section.status === "loading") {
    return <div aria-label="loading" className="h-6 animate-pulse rounded bg-muted" />;
  }
  if (section.status === "denied") {
    return (
      <p aria-label="denied" className="rounded bg-muted px-2 py-1.5 text-[11px] text-muted">
        Not permitted.
      </p>
    );
  }
  return ready(section.data);
}

/** A labelled explorer section with a one-line hint. */
function Section({
  label,
  hint,
  children,
}: {
  label: string;
  hint: string;
  children: React.ReactNode;
}) {
  return (
    <section aria-label={`section ${label}`}>
      <header className="px-1 pb-1">
        <h3 className="text-[11px] font-semibold uppercase tracking-wide text-muted">{label}</h3>
        <p className="text-[10px] text-muted/80">{hint}</p>
      </header>
      {children}
    </section>
  );
}

/** A teaching empty state. */
function Empty({ children }: { children: React.ReactNode }) {
  return <p className="px-2 py-2 text-[11px] text-muted">{children}</p>;
}
