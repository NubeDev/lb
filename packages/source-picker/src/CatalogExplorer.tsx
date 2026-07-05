// The catalog explorer — the BROWSABLE TREE skin over `useCatalog`'s per-section state (system-catalog
// scope). The shipped `<SourcePicker>`/`<SourceCombobox>` is the combobox skin — pick a source by
// typing. This is the other half: a section panel with click-to-insert rows, an honest loading/denied/
// empty per section, and a table→column tree for the local store.
//
// The renderer IS kind-aware (each kind has its own row shape), but the kind set is the package's
// own vocabulary — never a host's "known subsystem list" (rule 10). A host composes which sections
// its surface shows by passing `sections` (typically just `CATALOG_SECTION_SPECS`, but a host can
// pass a subset). `onSelect` receives a `CatalogEntry`; the HOST decides what the pick MEANS (the
// rule's `source("name")` snippet vs a dashboard cell source vs a SQL table name) — never this
// component.
//
// Self-themed via `--sp-*` tokens scoped to `.sp-root` (the @nube/panel discipline). No preflight,
// no global utilities — same as the combobox skin.

import type { CatalogSections } from "./loadCatalog";
import {
  CATALOG_SECTION_SPECS,
  channelEntries,
  inboxEntries,
  insightEntries,
  type CatalogEntry,
  type CatalogSectionKind,
  type CatalogSectionSpec,
} from "./catalog";
import type { SectionState } from "./types";
import { CatalogSection, CatalogEmpty } from "./CatalogSection";
import { CatalogSchemaTree } from "./CatalogSchemaTree";

export interface CatalogExplorerProps {
  /** The per-section state from `useCatalog`. Sections absent here (the host wired no loader) are
   *  skipped even if `sections` lists them — absent loader ⇒ absent section. */
  sections: CatalogSections;
  /** Called with the picked `CatalogEntry` whenever a row is clicked. The host maps the entry onto
   *  its own snippet/bind (a Rhai `source("name")`, a SQL table name, a dashboard cell source). */
  onSelect: (entry: CatalogEntry) => void;
  /** Which sections to render + their labels/hints, in display order. Defaults to the canonical
   *  `CATALOG_SECTION_SPECS`. A host that wants a subset (e.g. just `datasources` + `series`) passes
   *  its own filtered list. */
  sectionSpecs?: CatalogSectionSpec[];
  /** Extra className on the root. */
  className?: string;
}

/** The system-catalog explorer panel. */
export function CatalogExplorer({
  sections,
  onSelect,
  sectionSpecs = CATALOG_SECTION_SPECS,
  className,
}: CatalogExplorerProps) {
  return (
    <div aria-label="data explorer" className={`sp-root sp-catalog${className ? ` ${className}` : ""}`}>
      {sectionSpecs.map((spec) => {
        const state = sections[spec.kind];
        if (!state) return null; // absent loader ⇒ absent section
        // The state union is per-kind-typed in `CatalogSections`; the renderer is dynamically typed
        // (it switches on `spec.kind` and casts inside `renderRows`), so we widen to `unknown` here.
        return (
          <CatalogSection key={spec.kind} spec={spec} state={state as SectionState<unknown>}>
            {(data) => renderRows(spec.kind, data, onSelect)}
          </CatalogSection>
        );
      })}
    </div>
  );
}

/** Render the ready-body rows for one section kind. Each kind has its own row shape; the row's
 *  click yields a `CatalogEntry` of the matching kind. Returns `null` for an empty ready section so
 *  the section shows no rows (a future host could pass a per-kind empty message via `sectionSpecs`). */
function renderRows(
  kind: CatalogSectionKind,
  data: unknown,
  onSelect: (entry: CatalogEntry) => void,
): React.ReactNode {
  switch (kind) {
    case "datasources": {
      const rows = (data as import("./types").DatasourceRow[]) ?? [];
      if (rows.length === 0) return <CatalogEmpty>No external datasources registered.</CatalogEmpty>;
      return (
        <ul className="sp-catalog-list">
          {rows.map((d) => (
            <li key={d.name}>
              <button
                type="button"
                aria-label={`insert datasource ${d.name}`}
                className="sp-catalog-row sp-catalog-row-datasource"
                onClick={() =>
                  onSelect({
                    kind: "datasource",
                    id: `datasource:${d.name}`,
                    name: d.name,
                    rowKind: d.kind,
                    endpoint: d.endpoint,
                  })
                }
              >
                <span className="sp-catalog-row-label">
                  <span aria-hidden="true" className="sp-catalog-icon">
                    ◳
                  </span>
                  {d.name}
                </span>
                <span className="sp-catalog-row-sub">
                  {d.endpoint ? `${d.kind} · ${d.endpoint}` : d.kind}
                </span>
              </button>
            </li>
          ))}
        </ul>
      );
    }
    case "schema": {
      const schema = data as import("./types").Schema;
      if (schema.tables.length === 0) return <CatalogEmpty>No local tables yet.</CatalogEmpty>;
      return <CatalogSchemaTree schema={schema} onSelect={onSelect} />;
    }
    case "series": {
      const names = (data as string[]) ?? [];
      if (names.length === 0) return <CatalogEmpty>No series in this workspace.</CatalogEmpty>;
      return (
        <ul className="sp-catalog-list">
          {names.map((s) => (
            <li key={s}>
              <button
                type="button"
                aria-label={`insert series ${s}`}
                className="sp-catalog-row sp-catalog-row-series"
                onClick={() => onSelect({ kind: "series", id: `series:${s}`, name: s })}
              >
                <span aria-hidden="true" className="sp-catalog-icon">
                  〜
                </span>
                {s}
              </button>
            </li>
          ))}
        </ul>
      );
    }
    case "channels": {
      const rows = (data as import("./types").ChannelRow[]) ?? [];
      if (rows.length === 0) return <CatalogEmpty>No channels registered.</CatalogEmpty>;
      return (
        <ul className="sp-catalog-list">
          {rows.map((r) => {
            const e = channelEntries([r])[0]!;
            return (
              <li key={e.id}>
                <button
                  type="button"
                  aria-label={`insert channel ${r.id}`}
                  className="sp-catalog-row sp-catalog-row-channel"
                  onClick={() => onSelect(e)}
                >
                  <span aria-hidden="true" className="sp-catalog-icon">
                    #
                  </span>
                  {r.id}
                </button>
              </li>
            );
          })}
        </ul>
      );
    }
    case "insights": {
      const rows = (data as import("./types").InsightRow[]) ?? [];
      if (rows.length === 0) return <CatalogEmpty>No insights in this workspace.</CatalogEmpty>;
      return (
        <ul className="sp-catalog-list">
          {rows.map((r) => {
            const e = insightEntries([r])[0]!;
            return (
              <li key={e.id}>
                <button
                  type="button"
                  aria-label={`insert insight ${r.title}`}
                  className="sp-catalog-row sp-catalog-row-insight"
                  onClick={() => onSelect(e)}
                >
                  <span className="sp-catalog-row-label">
                    <span aria-hidden="true" className="sp-catalog-icon">
                      ◆
                    </span>
                    {r.title}
                  </span>
                  {(r.severity || r.status) && (
                    <span className="sp-catalog-row-sub">
                      {[r.severity, r.status].filter(Boolean).join(" · ")}
                    </span>
                  )}
                </button>
              </li>
            );
          })}
        </ul>
      );
    }
    case "inbox": {
      const rows = (data as import("./types").InboxRow[]) ?? [];
      if (rows.length === 0) return <CatalogEmpty>No items in this inbox.</CatalogEmpty>;
      return (
        <ul className="sp-catalog-list">
          {rows.map((r) => {
            const e = inboxEntries([r])[0]!;
            return (
              <li key={e.id}>
                <button
                  type="button"
                  aria-label={`insert inbox item ${r.id}`}
                  className="sp-catalog-row sp-catalog-row-inbox"
                  onClick={() => onSelect(e)}
                >
                  <span className="sp-catalog-row-label">
                    <span aria-hidden="true" className="sp-catalog-icon">
                      ✉
                    </span>
                    {r.id}
                  </span>
                  <span className="sp-catalog-row-sub">{r.channel}</span>
                </button>
              </li>
            );
          })}
        </ul>
      );
    }
    // The picker-only kinds (`extensions`/`rules`/`flowSummaries`/`flowDescriptors`) are not
    // rendered by the explorer today — they share the orchestration, not the skin. A host that wants
    // one of them as an explorer section adds a `CatalogSectionSpec` for it + a row renderer here.
    default:
      return null;
  }
}

/** Narrow a section state for the explorer — exposes the per-kind `SectionState` typing for hosts
 *  that build their own surface off `CatalogSection` directly. Convenience; the explorer itself uses
 *  the dynamic record above. */
export function sectionOf(
  sections: CatalogSections,
  kind: CatalogSectionKind,
): SectionState<unknown> | undefined {
  return sections[kind];
}
