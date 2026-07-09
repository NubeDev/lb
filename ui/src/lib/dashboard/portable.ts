// The portable dashboard bundle — OUR-format interchange for whole dashboards and/or standalone
// widgets/panels (dashboard scope, import/export UX). This is the "export from here, import here (or on
// another node)" artifact the manager UI reads/writes. It is NOT Grafana JSON: that is a separate,
// backend-mapped interchange (`viz/import-export-scope.md`, host verbs `dashboard.import`/`.export`).
// This bundle is pure client-side and composes with the SHIPPED `dashboard.save`/`panel.save` verbs, so
// it needs no new host surface — an admin exports a `.lbdash.json`, edits/moves it, imports it back.
//
// Tenancy (rule 6): a bundle carries NO workspace and NO owner. Those are authority the import must
// NEVER take from a file — the workspace comes from the session token, the owner from the caller. We
// carry only the portable shape: title, cells (layout + spec), variables, and per-widget/per-panel
// specs. Ids are advisory: the importer re-slugs to a fresh, non-colliding id so an import never
// silently overwrites an existing record (owner-only overwrite stays a deliberate, separate act).
//
// One responsibility: the bundle type + its serialize / parse / validate. No I/O, no React, no invoke.

import type { Cell, Dashboard, Variable } from "./dashboard.types";
import type { PanelSpec } from "@/lib/panel";

/** Bump when the bundle shape changes incompatibly. Import rejects a MAJOR it can't read; a newer
 *  MINOR from the same major degrades to the fields it knows (additive-forward). */
export const BUNDLE_VERSION = 1;

/** The file marker — an import validates this before trusting any bytes, so a stray `.json` (or a
 *  Grafana export, which belongs to the backend mapper) is rejected with an honest message rather than
 *  half-parsed into a broken record. */
export const BUNDLE_KIND = "lazybones.dashboard-bundle" as const;

/** The recommended file extension (double-extension so a plain JSON viewer still opens it). */
export const BUNDLE_EXT = ".lbdash.json";

/** One exported dashboard — the portable half of a {@link Dashboard} (no workspace/owner/visibility;
 *  those are re-established by the importer under its own authority). `id` is advisory (see file note). */
export interface PortableDashboard {
  id: string;
  title: string;
  cells: Cell[];
  variables?: Variable[];
  /** OUR panel-model doc version, carried for round-trip fidelity (never Grafana's schemaVersion). */
  schemaVersion?: number;
}

/** One exported standalone widget/panel — a {@link PanelSpec} (the non-layout half of a cell) plus an
 *  advisory id/title. This is what "export this widget" and "export a library panel" both produce, so a
 *  single widget travels between dashboards without dragging a whole dashboard along. */
export interface PortablePanel {
  id: string;
  title: string;
  spec: PanelSpec;
  schemaVersion?: number;
}

/** The bundle: one or more dashboards AND/OR one or more standalone widgets/panels. Either array may be
 *  empty, but a valid bundle has at least one entry across the two (a bundle that carries nothing is a
 *  user error, flagged on validate). */
export interface DashboardBundle {
  kind: typeof BUNDLE_KIND;
  version: number;
  /** ISO-ish stamp the exporter writes (informational only; never trusted for authority). Optional so a
   *  hand-authored bundle stays valid. */
  exportedAt?: string;
  dashboards: PortableDashboard[];
  panels: PortablePanel[];
}

/** Project a full {@link Dashboard} record down to its portable shape (drops workspace/owner/visibility/
 *  timestamps — the authority + volatile fields an import must re-establish, not carry). */
export function dashboardToPortable(d: Dashboard): PortableDashboard {
  return {
    id: bareId(d.id),
    title: d.title,
    cells: d.cells,
    variables: d.variables && d.variables.length > 0 ? d.variables : undefined,
    schemaVersion: d.schemaVersion,
  };
}

/** Strip a `dashboard:`/`panel:` table prefix so the portable id is the bare slug (the importer re-adds
 *  the prefix its verb expects). Idempotent on an already-bare id. */
export function bareId(id: string): string {
  const i = id.indexOf(":");
  return i >= 0 ? id.slice(i + 1) : id;
}

/** Build a bundle from any mix of dashboards and standalone panels. `exportedAt` is passed in (the
 *  caller owns the clock — this module stays pure/testable and never calls `Date.now`). */
export function makeBundle(
  dashboards: PortableDashboard[],
  panels: PortablePanel[],
  exportedAt?: string,
): DashboardBundle {
  return {
    kind: BUNDLE_KIND,
    version: BUNDLE_VERSION,
    exportedAt,
    dashboards,
    panels,
  };
}

/** Serialize a bundle to the on-disk string (pretty-printed — a bundle is meant to be diff-able and
 *  hand-editable). */
export function serializeBundle(bundle: DashboardBundle): string {
  return JSON.stringify(bundle, null, 2);
}

/** The outcome of validating parsed JSON as a bundle: the typed bundle, or an honest error. We never
 *  throw across the parse boundary — the dialog renders `error` verbatim. */
export type ParseResult =
  | { ok: true; bundle: DashboardBundle; warnings: string[] }
  | { ok: false; error: string };

/** Parse + validate an untrusted string as a bundle. Rejects (never guesses) on: bad JSON, a missing/
 *  wrong `kind` (so a Grafana export or random JSON is turned away with a pointer, not half-imported), a
 *  MAJOR version we can't read, or a structurally invalid entry. A newer minor within our major is
 *  accepted with a warning (additive-forward). */
export function parseBundle(text: string): ParseResult {
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch (e) {
    return {
      ok: false,
      error: `Not valid JSON: ${e instanceof Error ? e.message : String(e)}`,
    };
  }
  if (!isRecord(raw))
    return { ok: false, error: "Bundle must be a JSON object." };
  if (raw.kind !== BUNDLE_KIND) {
    return {
      ok: false,
      error:
        raw.kind === undefined
          ? `Not a Lazybones dashboard bundle (missing "kind"). A Grafana dashboard JSON imports through the Grafana import path, not here.`
          : `Unrecognized bundle kind "${String(raw.kind)}". Expected "${BUNDLE_KIND}".`,
    };
  }
  const version = typeof raw.version === "number" ? raw.version : NaN;
  if (!Number.isFinite(version))
    return { ok: false, error: "Bundle is missing a numeric version." };
  if (Math.floor(version) > BUNDLE_VERSION) {
    return {
      ok: false,
      error: `Bundle version ${version} is newer than this app understands (${BUNDLE_VERSION}). Update the app or re-export at a compatible version.`,
    };
  }

  const warnings: string[] = [];
  if (version < BUNDLE_VERSION)
    warnings.push(`Bundle is an older version (${version}); imported forward.`);

  const dashboards: PortableDashboard[] = [];
  const rawDash = Array.isArray(raw.dashboards) ? raw.dashboards : [];
  rawDash.forEach((d, i) => {
    const v = validateDashboardEntry(d, i);
    if (typeof v === "string") warnings.push(v);
    else dashboards.push(v);
  });

  const panels: PortablePanel[] = [];
  const rawPanels = Array.isArray(raw.panels) ? raw.panels : [];
  rawPanels.forEach((p, i) => {
    const v = validatePanelEntry(p, i);
    if (typeof v === "string") warnings.push(v);
    else panels.push(v);
  });

  if (dashboards.length === 0 && panels.length === 0) {
    return {
      ok: false,
      error:
        "Bundle contains no importable dashboards or widgets (every entry was invalid or missing).",
    };
  }

  return {
    ok: true,
    bundle: makeBundle(
      dashboards,
      panels,
      typeof raw.exportedAt === "string" ? raw.exportedAt : undefined,
    ),
    warnings,
  };
}

/** A short human summary of what a bundle carries (the import preview headline). */
export function describeBundle(bundle: DashboardBundle): string {
  const parts: string[] = [];
  if (bundle.dashboards.length)
    parts.push(plural(bundle.dashboards.length, "dashboard"));
  if (bundle.panels.length) parts.push(plural(bundle.panels.length, "widget"));
  return parts.join(" and ") || "nothing";
}

// --- internals ---------------------------------------------------------------------------------

function validateDashboardEntry(
  d: unknown,
  i: number,
): PortableDashboard | string {
  if (!isRecord(d)) return `Skipped dashboard #${i + 1}: not an object.`;
  const title = typeof d.title === "string" ? d.title : "";
  const id = typeof d.id === "string" && d.id ? bareId(d.id) : "";
  if (!id && !title)
    return `Skipped dashboard #${i + 1}: missing both id and title.`;
  if (!Array.isArray(d.cells))
    return `Skipped dashboard "${title || id}": cells must be an array.`;
  // Cells are validated structurally on the wire by the host `dashboard.save` (the real boundary); here
  // we only ensure the shape is an array of objects so a corrupt cell can't crash the preview render.
  const cells = (d.cells as unknown[]).filter(isRecord) as unknown as Cell[];
  const variables = Array.isArray(d.variables)
    ? (d.variables.filter(isRecord) as unknown as Variable[])
    : undefined;
  return {
    id: id || slugFromTitle(title),
    title: title || id,
    cells,
    variables: variables && variables.length ? variables : undefined,
    schemaVersion:
      typeof d.schemaVersion === "number" ? d.schemaVersion : undefined,
  };
}

function validatePanelEntry(p: unknown, i: number): PortablePanel | string {
  if (!isRecord(p)) return `Skipped widget #${i + 1}: not an object.`;
  const title = typeof p.title === "string" ? p.title : "";
  const id = typeof p.id === "string" && p.id ? bareId(p.id) : "";
  if (!isRecord(p.spec))
    return `Skipped widget "${title || id}": missing spec.`;
  if (!id && !title)
    return `Skipped widget #${i + 1}: missing both id and title.`;
  return {
    id: id || slugFromTitle(title),
    title: title || id,
    spec: p.spec as unknown as PanelSpec,
    schemaVersion:
      typeof p.schemaVersion === "number" ? p.schemaVersion : undefined,
  };
}

/** Slugify a title into a stable-ish id (mirrors the roster's `slug`). Only used as a fallback when a
 *  portable entry carried no id. */
export function slugFromTitle(title: string): string {
  return (
    title
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "") || "imported"
  );
}

/** Make an id unique against a set of taken ids by appending `-2`, `-3`, … — so an import never
 *  collides with (and silently overwrites) an existing record. */
export function uniqueId(desired: string, taken: Set<string>): string {
  if (!taken.has(desired)) return desired;
  let n = 2;
  while (taken.has(`${desired}-${n}`)) n += 1;
  return `${desired}-${n}`;
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

function plural(n: number, noun: string): string {
  return `${n} ${noun}${n === 1 ? "" : "s"}`;
}
