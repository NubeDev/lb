// Load every catalog section from the INJECTED loaders — the PURE async fn (no React). Each loader
// is independent: a deny/throw yields `denied` for that section ONLY (never a fabricated roster —
// CLAUDE §9); an absent loader yields an absent section (the host didn't wire it). The hook
// (`useCatalog`) wraps this; the picker's `loadSourcePicker` PROJECTS off the same per-section
// state — one loader orchestration, two projections (system-catalog scope, "Two state contracts,
// one hook").

import type { SectionState, SourceLoaders } from "./types";
import type { CatalogSectionKind } from "./catalog";

/** The schema of `CatalogSections.data` per section kind. The explorer kinds carry row arrays (or
 *  `Schema` for the local-tables section, which the tree renderer walks); the picker-only kinds
 *  (`extensions`/`rules`/`flowSummaries`/`flowDescriptors`) carry the row shapes `loadSourcePicker`
 *  composes from. */
export interface CatalogSectionData {
  datasources: import("./types").DatasourceRow[];
  schema: import("./types").Schema;
  series: string[];
  channels: import("./types").ChannelRow[];
  insights: import("./types").InsightRow[];
  inbox: import("./types").InboxRow[];
  extensions: import("./types").ExtRow[];
  rules: import("./types").RuleSummary[];
  flowSummaries: import("./types").FlowSummary[];
  flowDescriptors: import("./types").NodeDescriptor[];
}

/** The catalog's per-section honest state. A section is `undefined` when the host supplied no
 *  loader for it (absent ⇒ absent section); `{status:"loading"}` while in flight; `{status:"ready"}`
 *  on success; `{status:"denied"}` on throw (capability wall — never a fake list). */
export type CatalogSections = {
  [K in CatalogSectionKind]?: SectionState<CatalogSectionData[K]>;
};

/** The loader each section kind reads through. Absent loader ⇒ the section is absent. The first
 *  six are the explorer's browseable rosters; the last four are the picker's compose inputs (no
 *  explorer section today, but they share the orchestration so `loadSourcePicker` projects off the
 *  same per-section state). */
interface SectionLoaderMap {
  datasources: "listDatasources";
  schema: "readSchema";
  series: "listSeries";
  channels: "listChannels";
  insights: "listInsights";
  inbox: "listInbox";
  extensions: "listExtensions";
  rules: "listRules";
  flowSummaries: "listFlows";
  flowDescriptors: "listFlowNodes";
}

/** The fixed mapping from a section kind to its loader fn name on `SourceLoaders`. A host wires a
 *  section by adding the matching loader; the renderer never branches on a host's id list. */
const SECTION_LOADERS: SectionLoaderMap = {
  datasources: "listDatasources",
  schema: "readSchema",
  series: "listSeries",
  channels: "listChannels",
  insights: "listInsights",
  inbox: "listInbox",
  extensions: "listExtensions",
  rules: "listRules",
  flowSummaries: "listFlows",
  flowDescriptors: "listFlowNodes",
};

const SECTION_KINDS = Object.keys(SECTION_LOADERS) as CatalogSectionKind[];

function msg(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

/** Run every loader the host wired (deny-tolerant per section). Each present loader resolves to
 *  `ready`/`denied` independently; absent loaders yield an absent (undefined) section. The
 *  orchestration is the single source of truth — the picker's deny→empty collapse and the
 *  explorer's visible tri-state both project off the record this returns.
 *
 *  `publish` (optional) is invoked once per section as it resolves, with the cumulative
 *  `CatalogSections` record — so a caller (the `useCatalog` hook) can surface each section's state
 *  the moment it lands instead of waiting for every loader. Late calls after the caller is
 *  unmounted/cancelled are the caller's concern (it passes a `publish` that no-ops on cancel). */
export async function loadCatalog(
  loaders: SourceLoaders,
  publish?: (merge: (current: CatalogSections) => CatalogSections) => void,
): Promise<CatalogSections> {
  const out: Partial<CatalogSections> = {};
  const commit = (kind: CatalogSectionKind, state: SectionState<unknown>) => {
    (out as Record<string, SectionState<unknown>>)[kind] = state;
    publish?.((current) => ({ ...current, [kind]: state }) as CatalogSections);
  };
  await Promise.all(
    SECTION_KINDS.map(async (kind) => {
      const state = await runSectionLoader(loaders, kind);
      if (state) commit(kind, state);
    }),
  );
  return out as CatalogSections;
}

/** Run ONE section's loader (deny-tolerant). Returns `undefined` for an absent loader (the host wired
 *  no `listX` for this section — the field stays absent from the record). Eager `loadCatalog` is the
 *  fan-out over every kind; this is the per-section shot the lazy explorer fires when a user expands
 *  a section that's still `idle`. Shared with `loadCatalog` so the deny/empty behaviour is identical
 *  between the eager and lazy paths. */
export async function runSectionLoader(
  loaders: SourceLoaders,
  kind: CatalogSectionKind,
): Promise<SectionState<unknown> | undefined> {
  const fn = loaders[SECTION_LOADERS[kind]] as (() => Promise<unknown>) | undefined;
  if (!fn) return undefined; // absent loader ⇒ absent section (the field stays undefined).
  try {
    const data = await fn();
    return { status: "ready", data };
  } catch (e) {
    return { status: "denied", error: msg(e) };
  }
}
