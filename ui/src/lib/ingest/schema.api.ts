// Persist + load a series schema (data-console scope). The backend has no schema store, so a schema
// is written as a **real record** through the existing ingest path: one reserved meta-series per real
// series, `__schema.<name>`, whose latest sample's payload IS the schema JSON. Reading the schema is
// `series.latest("__schema.<name>")`; saving it is one `ingest.write`. No backend change — the schema
// is real, workspace-scoped data behind the same wall as every other sample.

import { latestSample, writeSample, listSeries } from "./ingest.api";
import type { SeriesSchema } from "./schema.types";

/** The reserved prefix for schema meta-series. Hidden from the normal series list (see `realSeries`). */
const SCHEMA_PREFIX = "__schema.";

/** The meta-series name a schema for `series` is stored under. */
function schemaSeries(series: string): string {
  return `${SCHEMA_PREFIX}${series}`;
}

/** Persist `schema` for its series (writes the schema JSON as the latest sample of the meta-series).
 *  Monotonic `seq` by clock so a re-save supersedes (latest wins). */
export async function saveSchema(schema: SeriesSchema): Promise<void> {
  const seq = Date.now();
  await writeSample({
    series: schemaSeries(schema.series),
    producer: "",
    ts: seq,
    seq,
    payload: schema as unknown,
    labels: { kind: "schema" },
  });
}

/** Load the schema for `series`, or `null` if none was ever defined (a series can exist without one —
 *  e.g. seeded by a producer). */
export async function loadSchema(series: string): Promise<SeriesSchema | null> {
  const sample = await latestSample(schemaSeries(series));
  if (!sample) return null;
  const p = sample.payload as Partial<SeriesSchema> | null;
  if (!p || !Array.isArray(p.fields)) return null;
  return { series, description: p.description, fields: p.fields };
}

/** List the workspace's **real** series — every series minus the reserved `__schema.*` meta-series.
 *  This is what the explorer shows. */
export async function listRealSeries(prefix = ""): Promise<string[]> {
  const all = await listSeries(prefix);
  return all.filter((s) => !s.startsWith(SCHEMA_PREFIX));
}

/** True if `series` is a reserved meta-series (never shown in the explorer). */
export function isMetaSeries(series: string): boolean {
  return series.startsWith(SCHEMA_PREFIX);
}
