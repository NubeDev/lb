// The nav builder (nav scope) — an admin surface to compose a workspace's navigation menu: pick items
// from the three REAL sources (core surfaces, `dashboard.list`, `ext.list`), add a dynamic tag-group,
// order/group them, set visibility, share to a team, and set the workspace default. The nav is a
// LENS — it grants nothing; it only shapes the menu. Every write is a real `nav.*` verb re-checked
// server-side; the builder can never author a cap. Cap-gated for DISPLAY by `nav.save`; the gateway
// is the boundary. Markup + local edit state; the data + writes live in `useNavs`.

import { useMemo, useState } from "react";
import { List, Plus } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { AppEmptyState } from "@/components/app/empty-state";
import { AdminToolbar } from "../AdminToolbar";
import { CAP, hasCap } from "@/lib/session";
import type { NavItem, Visibility } from "@/lib/nav";
import { useNavs } from "./useNavs";

/** The core surfaces a nav may link to (the static core set — surfaces are core, not extensions).
 *  Kept aligned with the shell's `SURFACES`; a nav entry stores the opaque key as data. */
const CORE_SURFACES = [
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
];

interface Props {
  ws: string;
  caps: string[] | undefined;
}

export function NavAdmin({ ws, caps }: Props) {
  const { navs, sources, shares, reloadShares, error, loading, save, remove, share, unshare, setDefault } =
    useNavs(ws);
  const canWrite = hasCap(caps, CAP.navSave);

  // The nav currently being edited: its id/title/items + a dirty flag. `null` = the roster view.
  const [editId, setEditId] = useState<string | null>(null);
  const [title, setTitle] = useState("");
  const [items, setItems] = useState<NavItem[]>([]);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  // Item-add form state.
  const [addKind, setAddKind] = useState<NavItem["kind"]>("surface");
  const [addSurface, setAddSurface] = useState(CORE_SURFACES[0]);
  const [addDashboard, setAddDashboard] = useState("");
  const [addExt, setAddExt] = useState("");
  const [addFacetKey, setAddFacetKey] = useState("");
  const [addLabel, setAddLabel] = useState("");
  // reusable-pages: template-group binds ONE dashboard's parameter (`addVar`) per facet value; a plain
  // `dashboard` entry can carry a pinned binding (`addPinned`, `name=value, …`) as a named instance.
  const [addVar, setAddVar] = useState("");
  const [addPinned, setAddPinned] = useState("");

  // Share form state.
  const [shareVis, setShareVis] = useState<Visibility>("private");
  const [shareTeam, setShareTeam] = useState("");
  const [filter, setFilter] = useState("");

  const visibleNavs = navs.filter(
    (n) =>
      n.title.toLowerCase().includes(filter.toLowerCase()) ||
      n.id.toLowerCase().includes(filter.toLowerCase()),
  );

  const startNew = () => {
    setEditId("");
    setTitle("");
    setItems([]);
    setMsg(null);
  };

  const startEdit = async (id: string) => {
    setBusy(true);
    setMsg(null);
    try {
      const { getNav } = await import("@/lib/nav");
      const n = await getNav(id);
      setEditId(n.id);
      setTitle(n.title);
      setItems(n.items ?? []);
      setShareVis(n.visibility);
      await reloadShares(n.id);
    } catch (e) {
      setMsg(String(e));
    } finally {
      setBusy(false);
    }
  };

  // Parse a `name=value, name2=value2` pinned-binding string into a `vars` record (reusable-pages).
  const parsePinned = (raw: string): Record<string, string> => {
    const out: Record<string, string> = {};
    for (const pair of raw.split(",")) {
      const [k, ...rest] = pair.split("=");
      const name = k.trim();
      const value = rest.join("=").trim();
      if (name && value) out[name] = value;
    }
    return out;
  };

  const addItem = () => {
    let it: NavItem | null = null;
    if (addKind === "surface") it = { kind: "surface", surface: addSurface, label: addLabel };
    else if (addKind === "dashboard" && addDashboard) {
      // A plain dashboard entry, optionally carrying a pinned binding (a named page instance).
      const vars = parsePinned(addPinned);
      it = { kind: "dashboard", dashboard: addDashboard, label: addLabel };
      if (Object.keys(vars).length) it.vars = vars;
    } else if (addKind === "ext" && addExt) it = { kind: "ext", ext: addExt, label: addLabel };
    else if (addKind === "tag-group" && addFacetKey)
      it = { kind: "tag-group", label: addLabel || addFacetKey, facets: [{ key: addFacetKey }] };
    else if (addKind === "template-group" && addDashboard && addVar && addFacetKey)
      // reusable-pages: ONE dashboard fanned out per distinct value of `addFacetKey`, bound to `addVar`.
      it = {
        kind: "template-group",
        label: addLabel || addFacetKey,
        dashboard: addDashboard,
        var: addVar,
        facets: [{ key: addFacetKey }],
      };
    else if (addKind === "group") it = { kind: "group", label: addLabel || "Group", items: [] };
    if (it) {
      setItems((cur) => [...cur, it!]);
      setAddLabel("");
      setAddFacetKey("");
      setAddVar("");
      setAddPinned("");
    }
  };

  const move = (i: number, delta: number) => {
    setItems((cur) => {
      const next = [...cur];
      const j = i + delta;
      if (j < 0 || j >= next.length) return cur;
      [next[i], next[j]] = [next[j], next[i]];
      return next;
    });
  };

  const removeItem = (i: number) => setItems((cur) => cur.filter((_, k) => k !== i));

  const doSave = async () => {
    if (!editId && !title) {
      setMsg("A nav needs an id/title");
      return;
    }
    setBusy(true);
    setMsg(null);
    try {
      // The id is a slug from the title on create (lowercased, dashed); keep the id on update.
      const id = editId || title.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
      await save(id, title, items);
      setEditId(id);
      setMsg("Saved.");
    } catch (e) {
      setMsg(String(e));
    } finally {
      setBusy(false);
    }
  };

  // Set the nav's overall audience tier (private / workspace) — the "Just me" / "Everyone" toggle.
  // Team sharing is `addTeam` (which force-sets the team tier itself); this handles the two non-team
  // audiences. Self-applying: one click writes it, no separate Save.
  const applyVis = async (vis: Visibility) => {
    if (!editId) return;
    setBusy(true);
    setMsg(null);
    try {
      await share(editId, vis, undefined);
      setShareVis(vis);
      setMsg(vis === "workspace" ? "Shared with everyone." : "Set to private.");
    } catch (e) {
      setMsg(String(e));
    } finally {
      setBusy(false);
    }
  };

  // A team's human name from the loaded roster (falls back to the raw id). The share roster stores
  // ids; the picker + list show names so an admin never reads `team:ops`.
  const teamName = (team: string): string =>
    sources.teams.find((t) => t.team === team)?.name || team;

  const addTeam = async () => {
    if (!editId || !shareTeam) return;
    setBusy(true);
    setMsg(null);
    try {
      // Writes one S4 `nav -[share]-> team` edge; the underlying relate is multi-edge, so each add
      // accumulates. Forces the Team tier (a team share is meaningless at private/workspace).
      if (shareVis !== "team") {
        setShareVis("team");
        await share(editId, "team", shareTeam);
      } else {
        await share(editId, "team", shareTeam);
      }
      setMsg(`Shared to ${shareTeam}.`);
      setShareTeam("");
    } catch (e) {
      setMsg(String(e));
    } finally {
      setBusy(false);
    }
  };

  const doUnshare = async (team: string) => {
    if (!editId) return;
    setBusy(true);
    setMsg(null);
    try {
      await unshare(editId, team);
      setMsg(`Removed share to ${team}.`);
    } catch (e) {
      setMsg(String(e));
    } finally {
      setBusy(false);
    }
  };

  const dashOptions = useMemo(
    () => sources.dashboards.map((d) => ({ id: `dashboard:${d.id}`, label: d.title })),
    [sources.dashboards],
  );
  const extOptions = useMemo(
    () => sources.extensions.map((e) => ({ id: e.ext, label: e.ui?.label ?? e.ext })),
    [sources.extensions],
  );

  if (!canWrite) {
    return (
      <div className="p-4 text-sm text-muted" role="status">
        You need the nav authoring capability (<code>mcp:nav.save:call</code>) to build navs.
      </div>
    );
  }

  // The editor form — rendered inline below the (always-mounted) roster toolbar so the search bar
  // stays put while editing, mirroring the API Keys "New key" flow.
  const navEditor = (
    <>
      {/* The nav's title + items, wrapped in the same bordered panel card the API Keys "New key"
          form uses (labelled fields over inputs) so both create surfaces read as one system. */}
      <div className="space-y-3 rounded-md border border-border bg-panel px-3 py-3">
        <label className="block text-xs text-muted">
          Menu title
          <Input
            aria-label="Nav title"
            className="mt-1"
            placeholder="Menu title (e.g. Operations)"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
          />
        </label>

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
              <Button
                size="sm"
                variant="ghost"
                onClick={() => removeItem(i)}
                aria-label="Remove item"
              >
                ✕
              </Button>
            </div>
          </li>
        ))}
        {items.length === 0 && (
          <li className="text-sm text-muted">No items yet — add one below.</li>
        )}
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
          {/* Two dynamism mechanisms, named side by side (reusable-pages scope, "one mental model"):
              tag-group = MANY dashboards (by tag); template-group = ONE dashboard, many bindings. */}
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
            {/* reusable-pages: an optional pinned binding → a curated named instance in the menu. */}
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
          // reusable-pages: pick the TEMPLATE dashboard, the parameter it binds, and the facet key
          // whose distinct values fan it out (one menu page per value, no dashboard copy).
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

      {/* Save the nav's items/title. Audience (who sees it) is the section below — no separate
          "apply visibility" step, so a nav can't be Saved-but-invisible by accident. */}
      <div className="flex flex-wrap items-center gap-2">
        <Button size="sm" onClick={() => void doSave()} disabled={busy} aria-label="Save nav">
          Save
        </Button>
      </div>
      </div>

      {/* ── Who sees this nav ── the ONE audience control. The old model had three moving parts
          (Save, a visibility <Select>, an "Apply visibility" button, AND a separate team-add), so
          an admin could build a nav, walk away, and leave it `private` = invisible to everyone.
          Now audience is explicit and self-applying: "Just me" (private, default), one or more
          teams (auto-switches to the Team tier + writes the edge in one click), or "Everyone"
          (workspace). No tier dropdown to forget. A member reaches a team-shared nav by being in
          that team (Teams admin). */}
      <section
        className="flex flex-col gap-2 rounded-md border border-border bg-panel px-3 py-3"
        data-testid="nav-shares"
      >
        <h4 className="text-xs font-semibold uppercase text-muted">Who sees this nav</h4>
        <div className="text-sm text-muted">
          {shareVis === "workspace"
            ? "Everyone in the workspace."
            : shares.length > 0
              ? `Members of ${shares.length === 1 ? "this team" : "these teams"}:`
              : "Only you (private). Add a team below, or share with everyone."}
        </div>
        {shareVis !== "workspace" && shares.length > 0 && (
          <ul className="flex flex-col gap-1">
            {shares.map((team) => (
              <li
                key={team}
                className="flex items-center justify-between rounded-md border border-border p-2"
              >
                <code className="text-sm">{teamName(team)}</code>
                <Button
                  size="sm"
                  variant="destructive"
                  onClick={() => void doUnshare(team)}
                  disabled={busy}
                  aria-label={`Remove share to ${team}`}
                >
                  Remove
                </Button>
              </li>
            ))}
          </ul>
        )}

        {/* Add a team — one click both switches the nav to the Team tier AND writes the
            `nav -[share]-> team` edge (see `addTeam`). The dropdown is fed by the real
            `teams.list` (no typing ids); if no teams exist yet, create one in the Teams admin.
            Hidden when the nav is already Workspace-wide (adding a team there is meaningless). */}
        {shareVis !== "workspace" && (
        <div className="flex flex-wrap items-center gap-2">
          <Select
            className="w-48"
            aria-label="Team to add"
            value={shareTeam}
            onChange={(e) => setShareTeam(e.target.value)}
          >
            <option value="">
              {sources.teams.length === 0 ? "No teams — create one in Teams" : "Pick a team"}
            </option>
            {sources.teams.map((t) => (
              <option key={t.team} value={t.team}>
                {t.name || t.team}
              </option>
            ))}
          </Select>
          <Button
            size="sm"
            variant="outline"
            onClick={() => void addTeam()}
            disabled={busy || !editId || !shareTeam}
            aria-label="Add team share"
          >
            Add team
          </Button>
        </div>
        )}

        {/* Workspace-wide toggle — the "everyone" audience, opposite the per-team shares. Setting
            Workspace makes every member see it (no team needed); "Just me" pulls it back to
            private. Both self-apply (`applyVis`), so audience is always one click, never a
            separate Save. */}
        <div className="flex flex-wrap items-center gap-2 pt-1">
          {shareVis === "workspace" ? (
            <Button
              size="sm"
              variant="outline"
              onClick={() => void applyVis("private")}
              disabled={busy || !editId}
              aria-label="Make private"
            >
              Make private (just me)
            </Button>
          ) : (
            <Button
              size="sm"
              variant="outline"
              onClick={() => void applyVis("workspace")}
              disabled={busy || !editId}
              aria-label="Share with everyone"
            >
              Share with everyone in workspace
            </Button>
          )}
        </div>
        <p className="text-xs text-muted">
          To share to a single user, put them in a team and add it here — a nav has no per-user
          audience (manage membership in the Teams admin).
        </p>
      </section>
    </>
  );

  return (
    <div className="flex h-full min-h-0 flex-col" data-testid="nav-admin">
      {error && (
        <div
          role="alert"
          className="border-b border-destructive/20 bg-destructive/10 px-4 py-2 text-xs text-destructive"
        >
          {error}
        </div>
      )}

      <section className="flex min-h-0 flex-1 flex-col">
        <AdminToolbar
          search={navs.length ? filter : undefined}
          onSearch={navs.length ? setFilter : undefined}
          searchPlaceholder="Filter navs…"
          action={
            editId === null ? (
              <Button size="sm" onClick={startNew} aria-label="New nav">
                <Plus size={13} /> New nav
              </Button>
            ) : (
              <>
                {msg && <span className="text-xs text-muted">{msg}</span>}
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => setEditId(null)}
                  aria-label="Back"
                >
                  Cancel
                </Button>
              </>
            )
          }
        />
        <div className="min-h-0 flex-1 overflow-y-auto">
          {editId !== null && (
            // ── Editor (rendered inline below the toolbar, roster stays put) ──
            <div className="flex flex-col gap-3 p-4">
              {navEditor}
            </div>
          )}
          {editId === null && (
          <>
          {loading ? (
            <div className="text-sm text-muted">Loading…</div>
          ) : navs.length === 0 || visibleNavs.length === 0 ? (
            <AppEmptyState
              icon={List}
              title={
                navs.length === 0 ? "No navs yet." : "No navs match."
              }
              description={
                navs.length === 0
                  ? "Members see the built-in sidebar until you build one."
                  : "Clear the filter to see every nav."
              }
            />
          ) : (
            <Table aria-label="navs">
              <TableHeader sticky>
                <TableRow>
                  <TableHead>Title</TableHead>
                  <TableHead>Visibility</TableHead>
                  <TableHead>Id</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {visibleNavs.map((n) => (
                  <TableRow key={n.id}>
                    <TableCell className="font-medium text-fg">{n.title}</TableCell>
                    <TableCell>
                      <Badge variant="outline">{n.visibility}</Badge>
                    </TableCell>
                    <TableCell className="font-mono text-muted">{n.id}</TableCell>
                    <TableCell className="text-right">
                      <span className="flex justify-end gap-1">
                        <Button
                          size="sm"
                          variant="ghost"
                          onClick={() => void startEdit(n.id)}
                          aria-label={`Edit ${n.title}`}
                        >
                          Edit
                        </Button>
                        <Button
                          size="sm"
                          variant="ghost"
                          onClick={() => void setDefault(n.id)}
                          aria-label={`Set ${n.title} as workspace default`}
                        >
                          Make default
                        </Button>
                        <Button
                          size="sm"
                          variant="destructive"
                          onClick={() => void remove(n.id)}
                          aria-label={`Delete ${n.title}`}
                        >
                          Delete
                        </Button>
                      </span>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
          </>
          )}
        </div>
      </section>
    </div>
  );
}

