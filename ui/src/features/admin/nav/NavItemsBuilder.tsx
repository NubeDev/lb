// The nav MENU composer (nav scope) — the shared, reusable core of the nav builder: a title field, the
// ordered items list (reorder / remove), and the add-item form that picks from the three REAL sources
// (core surfaces, `dashboard.list`, `ext.list`) plus the dynamic tag-group / template-group / group
// kinds. It owns NO persistence and NO sharing — it's a controlled editor over `(title, items)` — so it
// drops into BOTH the Nav admin tab (`NavAdmin`) and the onboarding wizard's Access step without
// duplicating the item grammar. Sharing/visibility is the caller's concern (the tab has its
// "Who sees this nav" section; the wizard shares to the team it's onboarding). One responsibility per
// file (FILE-LAYOUT): compose the menu, nothing else.

import { useMemo, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import type { NavItem } from "@/lib/nav";
import type { DashboardSummary } from "@/lib/dashboard";
import type { ExtRow } from "@/lib/ext/ext.api";

/** The core surfaces a nav may link to (the static core set — surfaces are core, not extensions).
 *  Kept aligned with the shell's `SURFACES`; a nav entry stores the opaque key as data. */
export const CORE_SURFACES = [
  "channels",
  "dashboards",
  "rules",
  "flows",
  "datasources",
  "reminders",
  "ingest",
  "data",
  "system",
  "telemetry",
  "inbox",
  "outbox",
  "admin",
  "extensions",
  "studio",
  "settings",
] as const;

/** The pickable sources the add-item form draws from (real lists, workspace-walled). */
export interface NavItemSources {
  dashboards: DashboardSummary[];
  extensions: ExtRow[];
}

interface Props {
  title: string;
  onTitleChange: (title: string) => void;
  items: NavItem[];
  onItemsChange: (items: NavItem[]) => void;
  sources: NavItemSources;
  /** Hide the title field when the caller owns the title elsewhere (e.g. the wizard names the nav). */
  hideTitle?: boolean;
}

/** Parse a `name=value, name2=value2` pinned-binding string into a `vars` record (reusable-pages). */
function parsePinned(raw: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const pair of raw.split(",")) {
    const [k, ...rest] = pair.split("=");
    const name = k.trim();
    const value = rest.join("=").trim();
    if (name && value) out[name] = value;
  }
  return out;
}

export function NavItemsBuilder({
  title,
  onTitleChange,
  items,
  onItemsChange,
  sources,
  hideTitle,
}: Props) {
  // Item-add form state (local to the composer).
  const [addKind, setAddKind] = useState<NavItem["kind"]>("surface");
  const [addSurface, setAddSurface] = useState<string>(CORE_SURFACES[0]);
  const [addDashboard, setAddDashboard] = useState("");
  const [addExt, setAddExt] = useState("");
  const [addFacetKey, setAddFacetKey] = useState("");
  const [addLabel, setAddLabel] = useState("");
  const [addVar, setAddVar] = useState("");
  const [addPinned, setAddPinned] = useState("");

  const dashOptions = useMemo(
    () => sources.dashboards.map((d) => ({ id: `dashboard:${d.id}`, label: d.title })),
    [sources.dashboards],
  );
  const extOptions = useMemo(
    () => sources.extensions.map((e) => ({ id: e.ext, label: e.ui?.label ?? e.ext })),
    [sources.extensions],
  );

  const addItem = () => {
    let it: NavItem | null = null;
    if (addKind === "surface") it = { kind: "surface", surface: addSurface, label: addLabel };
    else if (addKind === "dashboard" && addDashboard) {
      const vars = parsePinned(addPinned);
      it = { kind: "dashboard", dashboard: addDashboard, label: addLabel };
      if (Object.keys(vars).length) it.vars = vars;
    } else if (addKind === "ext" && addExt) it = { kind: "ext", ext: addExt, label: addLabel };
    else if (addKind === "tag-group" && addFacetKey)
      it = { kind: "tag-group", label: addLabel || addFacetKey, facets: [{ key: addFacetKey }] };
    else if (addKind === "template-group" && addDashboard && addVar && addFacetKey)
      it = {
        kind: "template-group",
        label: addLabel || addFacetKey,
        dashboard: addDashboard,
        var: addVar,
        facets: [{ key: addFacetKey }],
      };
    else if (addKind === "group") it = { kind: "group", label: addLabel || "Group", items: [] };
    if (it) {
      onItemsChange([...items, it]);
      setAddLabel("");
      setAddFacetKey("");
      setAddVar("");
      setAddPinned("");
    }
  };

  const move = (i: number, delta: number) => {
    const next = [...items];
    const j = i + delta;
    if (j < 0 || j >= next.length) return;
    [next[i], next[j]] = [next[j], next[i]];
    onItemsChange(next);
  };

  const removeItem = (i: number) => onItemsChange(items.filter((_, k) => k !== i));

  return (
    <div className="space-y-3">
      {!hideTitle && (
        <label className="block text-xs text-muted">
          Menu title
          <Input
            aria-label="Nav title"
            className="mt-1"
            placeholder="Menu title (e.g. Operations)"
            value={title}
            onChange={(e) => onTitleChange(e.target.value)}
          />
        </label>
      )}

      {/* The ordered items list */}
      <ol className="flex flex-col gap-2" data-testid="nav-items">
        {items.map((it, i) => (
          <li
            key={`${it.kind}-${i}`}
            className="flex items-center justify-between rounded-md border border-border p-2"
          >
            <div className="flex items-center gap-2">
              <Badge variant="outline">{it.kind}</Badge>
              <span className="text-sm">
                {it.label ||
                  it.surface ||
                  it.dashboard ||
                  it.ext ||
                  it.facets?.map((f) => f.key).join(",")}
              </span>
            </div>
            <div className="flex gap-1">
              <Button size="sm" variant="ghost" onClick={() => move(i, -1)} aria-label="Move up">
                ↑
              </Button>
              <Button size="sm" variant="ghost" onClick={() => move(i, 1)} aria-label="Move down">
                ↓
              </Button>
              <Button size="sm" variant="ghost" onClick={() => removeItem(i)} aria-label="Remove item">
                ✕
              </Button>
            </div>
          </li>
        ))}
        {items.length === 0 && <li className="text-sm text-muted">No items yet — add one below.</li>}
      </ol>

      {/* Add-item form */}
      <fieldset className="text-xs text-muted">
        <legend className="mb-1">Add item</legend>
        <div className="flex flex-wrap items-center gap-2 rounded-md border border-dashed border-border p-2">
          <Select
            className="w-36"
            aria-label="Item kind"
            value={addKind}
            onChange={(e) => setAddKind(e.target.value as NavItem["kind"])}
          >
            <option value="surface">Surface</option>
            <option value="dashboard">Dashboard</option>
            <option value="ext">Extension</option>
            {/* tag-group = MANY dashboards (by tag); template-group = ONE dashboard, many bindings. */}
            <option value="tag-group">Dashboards by tag</option>
            <option value="template-group">One dashboard per ⟨value⟩</option>
            <option value="group">Group</option>
          </Select>

          {addKind === "surface" && (
            <Select
              className="w-40"
              aria-label="Surface"
              value={addSurface}
              onChange={(e) => setAddSurface(e.target.value)}
            >
              {CORE_SURFACES.map((s) => (
                <option key={s} value={s}>
                  {s}
                </option>
              ))}
            </Select>
          )}
          {addKind === "dashboard" && (
            <>
              <Select
                className="w-48"
                aria-label="Dashboard"
                value={addDashboard}
                onChange={(e) => setAddDashboard(e.target.value)}
              >
                <option value="">Pick a dashboard</option>
                {dashOptions.map((d) => (
                  <option key={d.id} value={d.id}>
                    {d.label}
                  </option>
                ))}
              </Select>
              <Input
                aria-label="Pinned vars"
                placeholder="Pin vars (site=plant-2)"
                className="w-44"
                value={addPinned}
                onChange={(e) => setAddPinned(e.target.value)}
              />
            </>
          )}
          {addKind === "template-group" && (
            <>
              <Select
                className="w-48"
                aria-label="Template dashboard"
                value={addDashboard}
                onChange={(e) => setAddDashboard(e.target.value)}
              >
                <option value="">Pick a template</option>
                {dashOptions.map((d) => (
                  <option key={d.id} value={d.id}>
                    {d.label}
                  </option>
                ))}
              </Select>
              <Input
                aria-label="Template parameter"
                placeholder="Parameter (e.g. site)"
                className="w-36"
                value={addVar}
                onChange={(e) => setAddVar(e.target.value)}
              />
              <Input
                aria-label="Fan-out facet key"
                placeholder="Facet key (e.g. site)"
                className="w-36"
                value={addFacetKey}
                onChange={(e) => setAddFacetKey(e.target.value)}
              />
            </>
          )}
          {addKind === "ext" && (
            <Select
              className="w-48"
              aria-label="Extension"
              value={addExt}
              onChange={(e) => setAddExt(e.target.value)}
            >
              <option value="">Pick an extension</option>
              {extOptions.map((e) => (
                <option key={e.id} value={e.id}>
                  {e.label}
                </option>
              ))}
            </Select>
          )}
          {addKind === "tag-group" && (
            <Input
              aria-label="Facet key"
              placeholder="Facet key (e.g. site)"
              className="w-40"
              value={addFacetKey}
              onChange={(e) => setAddFacetKey(e.target.value)}
            />
          )}
          <Input
            aria-label="Item label"
            placeholder="Label (optional)"
            className="w-40"
            value={addLabel}
            onChange={(e) => setAddLabel(e.target.value)}
          />
          <Button size="sm" variant="outline" onClick={addItem} aria-label="Add item">
            Add item
          </Button>
        </div>
      </fieldset>
    </div>
  );
}
