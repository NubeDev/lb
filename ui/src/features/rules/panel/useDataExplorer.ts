// The data-explorer hook — loads the three things a rule can query, each from a SHIPPED workspace-walled
// verb (rules-editor-ux scope): the registered external datasources (`datasource.list`), the local store
// schema (`store.schema`, via the shared `@/lib/schema` reader), and the discoverable series
// (`series.list`, via the shipped `listRealSeries`). Each section tracks its own loading/deny/ready
// state HONESTLY — a denied `datasource.list` is a deny, never a fabricated roster (CLAUDE §9). One hook
// per file (FILE-LAYOUT).

import { useEffect, useState } from "react";

import { listDatasources, type DatasourceSummary } from "@/lib/datasources";
import { readSchema, type Schema } from "@/lib/schema";
import { listRealSeries } from "@/lib/ingest/schema.api";

/** A section's load state — never a fake "ready with empty data" when the read was denied. */
export type SectionState<T> =
  | { status: "loading" }
  | { status: "ready"; data: T }
  | { status: "denied"; error: string };

export interface DataExplorerState {
  datasources: SectionState<DatasourceSummary[]>;
  schema: SectionState<Schema>;
  series: SectionState<string[]>;
}

function msg(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

/** Load the explorer's three sections once per workspace; expose each section's honest state. */
export function useDataExplorer(ws: string): DataExplorerState {
  const [datasources, setDatasources] = useState<SectionState<DatasourceSummary[]>>({ status: "loading" });
  const [schema, setSchema] = useState<SectionState<Schema>>({ status: "loading" });
  const [series, setSeries] = useState<SectionState<string[]>>({ status: "loading" });

  useEffect(() => {
    let cancelled = false;
    setDatasources({ status: "loading" });
    setSchema({ status: "loading" });
    setSeries({ status: "loading" });

    listDatasources()
      .then((d) => !cancelled && setDatasources({ status: "ready", data: d }))
      .catch((e) => !cancelled && setDatasources({ status: "denied", error: msg(e) }));
    readSchema()
      .then((s) => !cancelled && setSchema({ status: "ready", data: s }))
      .catch((e) => !cancelled && setSchema({ status: "denied", error: msg(e) }));
    listRealSeries()
      .then((s) => !cancelled && setSeries({ status: "ready", data: s }))
      .catch((e) => !cancelled && setSeries({ status: "denied", error: msg(e) }));

    return () => {
      cancelled = true;
    };
  }, [ws]);

  return { datasources, schema, series };
}
